//! A brand's pricing over HTTP (`panel::pricing`): the editor's routes under `/api/v1` behind
//! the conductor's assertion, and the sites' read under `/api/internal`, which has none.
//!
//! ```text
//! GET    /api/v1/pricing                            {items: [Item]}: every brand the panel knows
//! GET    /api/v1/pricing/{brand}                    Item
//! PUT    /api/v1/pricing/{brand}           admin    {model, expected_updated_at} → 200 Item;
//!                                                   409 {error: "stale", current: Item};
//!                                                   422 {error, path}
//! DELETE /api/v1/pricing/{brand}           admin    {expected_updated_at} → 200 Item; 409 as PUT
//! POST   /api/v1/pricing/{brand}/preview            {model, need, inputs} → 200 {cents | null};
//!                                                   422 as PUT
//! GET    /api/v1/pricing/{brand}/changes            {changes: [{id, at, by, kind, valid_from, needs,
//!                                                   model | null}]}
//! GET    /api/internal/brands/{brand}/pricing       the model, or {} — never a 404 or a 5xx
//!
//! Item: {brand_id, locales, model | null, updated_at | null, updated_by | null}
//! ```
//!
//! `path` is relative to the model, `code` one of `PRICING_CODES`: `needs.standard.inputs[2]`,
//! `inputs[0].options[1].labels.en` (`""` when the model is not an object at all).

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use axum::{
	Extension, Json,
	body::Bytes,
	extract::{Path, State, rejection::JsonRejection},
	http::{Method, StatusCode},
	response::{IntoResponse, Response},
};
use jiff::Timestamp;
use panel::{
	Panel,
	place::Expected,
	pricing::{PricingError, PricingView},
};
use panel_core::{ids::BrandId, pricing::Problem};
use sa_auth::Pricing;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
	api::ApiError,
	conductor::{InternalEndpoint, InternalReads},
	gate::Caller,
	http::{ApiRoute, Section},
	places::editor,
};

/// How long the sites' read may take before they are answered `{}`: a site gives up at 3 s.
const LIVE_WITHIN: Duration = Duration::from_millis(2500);

pub(crate) fn routes() -> Vec<ApiRoute> {
	use Method as M;
	let work = Some(Section::Work);
	vec![
		ApiRoute::new(M::GET, "/pricing", work, list),
		ApiRoute::new(M::GET, "/pricing/{brand}", work, item),
		ApiRoute::new(M::GET, "/pricing/{brand}/changes", work, changes),
		ApiRoute::new(M::POST, "/pricing/{brand}/preview", work, preview),
		ApiRoute::new(M::PUT, "/pricing/{brand}", work, set),
		ApiRoute::new(M::DELETE, "/pricing/{brand}", work, remove),
	]
}

/// Why a request about pricing failed, in the contract's shapes.
enum PricingApiError {
	Api(ApiError),
	Stale(Box<PricingView>),
	Invalid(Problem),
}

impl IntoResponse for PricingApiError {
	fn into_response(self) -> Response {
		match self {
			Self::Api(e) => e.into_response(),
			Self::Stale(current) => (StatusCode::CONFLICT, Json(json!({ "error": "stale", "current": item_json(&current) }))).into_response(),
			Self::Invalid(p) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": p.why, "code": p.code.as_str(), "path": p.path }))).into_response(),
		}
	}
}

impl From<ApiError> for PricingApiError {
	fn from(e: ApiError) -> Self {
		Self::Api(e)
	}
}

impl From<eyre::Report> for PricingApiError {
	fn from(e: eyre::Report) -> Self {
		Self::Api(ApiError::Internal(e))
	}
}

impl From<PricingError> for PricingApiError {
	fn from(e: PricingError) -> Self {
		match e {
			PricingError::Stale(current) => Self::Stale(current),
			PricingError::Invalid(problems) => match problems.into_iter().next() {
				Some(first) => Self::Invalid(first),
				None => Self::Api(ApiError::Internal(eyre::eyre!("a model refused with no problem named"))),
			},
			PricingError::Internal(e) => Self::Api(ApiError::Internal(e)),
		}
	}
}

type PricingResult<T> = Result<T, PricingApiError>;

fn item_json(v: &PricingView) -> Value {
	json!({
		"brand_id": v.brand.as_str(),
		"locales": v.locales,
		"model": v.model,
		"updated_at": v.updated_at.map(|t| t.to_string()),
		"updated_by": v.updated_by,
	})
}

fn admin(caller: &Caller) -> Result<(), ApiError> {
	if caller.permissions.may(Pricing::Edit) { Ok(()) } else { Err(ApiError::Forbidden) }
}

fn json_body<T: serde::de::DeserializeOwned>(b: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
	b.map(|Json(v)| v).map_err(|e| ApiError::BadRequest(e.body_text()))
}

/// `expected_updated_at` as the editor sent it: null for pricing never set.
fn expected_at(raw: Option<&str>) -> Result<Expected, ApiError> {
	let at = raw
		.map(|raw| {
			raw.parse::<Timestamp>()
				.map_err(|_| ApiError::BadRequest("expected_updated_at is not an RFC 3339 instant".into()))
		})
		.transpose()?;
	Ok(Expected::At(at))
}

async fn list(State(panel): State<Panel>) -> PricingResult<Json<Value>> {
	let items: Vec<Value> = panel.all_pricing().await?.iter().map(item_json).collect();
	Ok(Json(json!({ "items": items })))
}

async fn item(State(panel): State<Panel>, Path(brand): Path<String>) -> PricingResult<Json<Value>> {
	let brand = BrandId::parse(&brand).map_err(ApiError::from)?;
	Ok(Json(item_json(&panel.pricing(&brand).await?)))
}

async fn changes(State(panel): State<Panel>, Path(brand): Path<String>) -> PricingResult<Json<Value>> {
	let brand = BrandId::parse(&brand).map_err(ApiError::from)?;
	let changes: Vec<Value> = panel
		.pricing_history(&brand)
		.await?
		.into_iter()
		.map(|c| {
			json!({
				"id": c.id.to_string(),
				"at": c.at.to_string(),
				"by": c.by,
				"kind": c.kind.as_str(),
				"valid_from": c.valid_from,
				"needs": c.needs,
				"model": c.model,
			})
		})
		.collect();
	Ok(Json(json!({ "changes": changes })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetBody {
	model: Value,
	/// The `updated_at` the editor read; null (or absent) for pricing never set.
	#[serde(default)]
	expected_updated_at: Option<String>,
}

async fn set(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path(brand): Path<String>, b: Result<Json<SetBody>, JsonRejection>) -> PricingResult<Json<Value>> {
	admin(&caller)?;
	let brand = BrandId::parse(&brand).map_err(ApiError::from)?;
	let b = json_body(b)?;
	let expected = expected_at(b.expected_updated_at.as_deref())?;
	let v = panel.set_pricing(&editor(&caller), &brand, &b.model, expected, Timestamp::now()).await?;
	Ok(Json(item_json(&v)))
}

/// A removal's body: `{"expected_updated_at": "…" | null}`, required — taking a brand's prices
/// off its sites over a version the editor did not see is not something to do by default.
fn remove_expected(raw: &[u8]) -> Result<Expected, ApiError> {
	let shape = || ApiError::BadRequest("the body is {\"expected_updated_at\": \"…\" | null}".into());
	let v: Value = serde_json::from_slice(raw).map_err(|_| shape())?;
	let Value::Object(m) = v else { return Err(shape()) };
	if m.len() != 1 {
		return Err(shape());
	}
	match m.get("expected_updated_at").ok_or_else(shape)? {
		Value::Null => expected_at(None),
		Value::String(t) => expected_at(Some(t)),
		_ => Err(shape()),
	}
}

async fn remove(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path(brand): Path<String>, raw: Bytes) -> PricingResult<Json<Value>> {
	admin(&caller)?;
	let brand = BrandId::parse(&brand).map_err(ApiError::from)?;
	let expected = remove_expected(&raw)?;
	let v = panel.remove_pricing(&editor(&caller), &brand, expected, Timestamp::now()).await?;
	Ok(Json(item_json(&v)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewBody {
	model: Value,
	need: String,
	/// Input id → option id.
	#[serde(default)]
	inputs: BTreeMap<String, String>,
}

async fn preview(State(panel): State<Panel>, Path(brand): Path<String>, b: Result<Json<PreviewBody>, JsonRejection>) -> PricingResult<Json<Value>> {
	let brand = BrandId::parse(&brand).map_err(ApiError::from)?;
	let b = json_body(b)?;
	let cents = panel.preview_price(&brand, &b.model, &b.need, &b.inputs).await?;
	Ok(Json(json!({ "cents": cents })))
}

/// What a site gets: the model, or `{}` for none — and `{}` too for a brand id that cannot be
/// one, a store that fails or does not answer in time. The site then keeps its baked model,
/// which is what it would do on an error; a 404 or a 5xx here would only be noise.
pub(crate) async fn live(State(panel): State<Panel>, Extension(reads): Extension<Arc<InternalReads>>, Path(brand): Path<String>) -> Json<Value> {
	let Ok(brand) = BrandId::parse(&brand) else {
		return Json(json!({}));
	};
	reads.record(&brand, InternalEndpoint::Pricing);
	match tokio::time::timeout(LIVE_WITHIN, panel.live_pricing(&brand)).await {
		Ok(Ok(Some(model))) => Json(model),
		Ok(Ok(None)) => Json(json!({})),
		Ok(Err(e)) => {
			crate::report(&e, "serving a brand's pricing to a site");
			Json(json!({}))
		}
		Err(_) => {
			tracing::warn!(%brand, "serving a brand's pricing to a site took too long: answered {{}}");
			Json(json!({}))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_removal_names_what_it_removes() {
		assert!(matches!(remove_expected(br#"{"expected_updated_at": null}"#), Ok(Expected::At(None))));
		assert!(matches!(remove_expected(br#"{"expected_updated_at": "2026-10-03T12:00:00Z"}"#), Ok(Expected::At(Some(_)))));
		for bad in [
			&b""[..],
			b"null",
			b"{}",
			br#"{"expected_updated_at": 1}"#,
			br#"{"expected_updated_at": null, "x": 1}"#,
			br#"{"expected_updated_at": "yesterday"}"#,
		] {
			assert!(remove_expected(bad).is_err(), "{}", String::from_utf8_lossy(bad));
		}
	}
}
