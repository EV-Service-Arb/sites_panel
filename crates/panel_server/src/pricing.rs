//! A brand's pricing over HTTP (`panel::pricing`): the editor's routes under `/api/v1` behind
//! the conductor's assertion, and the sites' read under `/api/internal`, which has none.
//!
//! ```text
//! GET    /api/internal/brands/{brand}/pricing       the model, or {} — never a 404 or a 5xx
//! ```

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use axum::{
	Extension, Json,
	extract::{Path, State},
	http::{Method, StatusCode},
	response::{IntoResponse, Response},
};
use ev_lib::api::{
	GenContext, JsonSchema, OperationOutput,
	openapi::{Operation, Response as Answer, StatusCode as Status},
};
use jiff::Timestamp;
use panel::{
	Panel,
	place::Expected,
	pricing::{PricingError, PricingView},
};
use panel_core::pricing::{Code, PricingModel, Problem};
use sa_auth::Pricing;
use sa_core::ids::BrandId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
	api::{ApiError, ApiResult, Body, ErrorDto, Ids},
	conductor::{InternalEndpoint, InternalReads},
	gate::Caller,
	http::{ApiRoute, Gate, Section},
	places::editor,
};

/// How long the sites' read may take before they are answered `{}`: a site gives up at 3 s.
const LIVE_WITHIN: Duration = Duration::from_millis(2500);

pub(crate) fn routes() -> Vec<ApiRoute> {
	use Method as M;
	let work = Gate::section(Section::Work);
	let edit = work.may(Pricing::Edit);
	vec![
		ApiRoute::new(M::GET, "/pricing", work, list_pricing),
		ApiRoute::new(M::GET, "/pricing/{brand}", work, get_pricing),
		ApiRoute::new(M::GET, "/pricing/{brand}/changes", work, pricing_changes),
		ApiRoute::new(M::POST, "/pricing/{brand}/preview", work, preview_price),
		ApiRoute::new(M::PUT, "/pricing/{brand}", edit, set_pricing),
		ApiRoute::new(M::DELETE, "/pricing/{brand}", edit, remove_pricing),
	]
}

/// Why a request about pricing failed, in the contract's shapes.
enum PricingApiError {
	Api(ApiError),
	Stale(Box<PricingView>),
	Invalid(Problem),
}

/// `expected_updated_at` is not what the brand's pricing stands at.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PricingStale")]
struct StaleDto {
	/// `stale`.
	error: &'static str,
	current: ItemDto,
}

/// A model refused: its first problem.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PricingInvalid")]
struct InvalidDto {
	/// In English, for a log.
	error: String,
	code: Code,
	/// Relative to the model: `needs.standard.inputs[2]`, `inputs[0].options[1].labels.en`;
	/// `""` when the model is not an object at all.
	path: String,
}

impl IntoResponse for PricingApiError {
	fn into_response(self) -> Response {
		match self {
			Self::Api(e) => e.into_response(),
			Self::Stale(current) => (
				StatusCode::CONFLICT,
				Json(StaleDto {
					error: "stale",
					current: (*current).into(),
				}),
			)
				.into_response(),
			Self::Invalid(p) => (
				StatusCode::UNPROCESSABLE_ENTITY,
				Json(InvalidDto {
					error: p.why,
					code: p.code,
					path: p.path,
				}),
			)
				.into_response(),
		}
	}
}

impl OperationOutput for PricingApiError {
	type Inner = ErrorDto;

	fn inferred_responses(ctx: &mut GenContext, op: &mut Operation) -> Vec<(Option<Status>, Answer)> {
		let mut answers = ApiError::inferred_responses(ctx, op);
		answers.extend(Json::<StaleDto>::operation_response(ctx, op).map(|r| (Some(Status::Code(409)), r)));
		answers.extend(Json::<InvalidDto>::operation_response(ctx, op).map(|r| (Some(Status::Code(422)), r)));
		answers
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

/// A brand's pricing as saved.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Pricing")]
struct ItemDto {
	brand_id: BrandId,
	/// The languages every label must be in; fr and en unless set.
	locales: Vec<String>,
	/// Null when never set or removed: the sites keep their baked one.
	#[schemars(with = "Option<PricingModel>")]
	model: Option<Value>,
	/// Null while never set: what a first save must expect. A removal has a time too.
	updated_at: Option<Timestamp>,
	updated_by: Option<String>,
}

impl From<PricingView> for ItemDto {
	fn from(v: PricingView) -> Self {
		Self {
			brand_id: v.brand,
			locales: v.locales,
			model: v.model,
			updated_at: v.updated_at,
			updated_by: v.updated_by,
		}
	}
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Pricings")]
struct ItemsDto {
	items: Vec<ItemDto>,
}

/// The pricing of every brand the panel knows (a lead's, a place's, a landing's experiments')
/// or has pricing or locales for.
async fn list_pricing(State(panel): State<Panel>) -> ApiResult<Json<ItemsDto>> {
	Ok(Json(ItemsDto {
		items: panel.all_pricing().await?.into_iter().map(ItemDto::from).collect(),
	}))
}

/// `{brand}`.
#[derive(Deserialize, JsonSchema)]
struct BrandPath {
	brand: BrandId,
}

/// A brand's pricing; one never set has its model, `updated_at` and `updated_by` null.
async fn get_pricing(State(panel): State<Panel>, Ids(BrandPath { brand }): Ids<BrandPath>) -> ApiResult<Json<ItemDto>> {
	Ok(Json(panel.pricing(&brand).await?.into()))
}

/// One change of a brand's pricing.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PricingChange")]
struct ChangeDto {
	id: Uuid,
	at: Timestamp,
	by: String,
	/// set | remove
	kind: &'static str,
	/// The saved model's `validFrom`; null for a removal.
	valid_from: Option<String>,
	/// How many needs the saved model prices; null for a removal.
	needs: Option<usize>,
	/// The model the change left; null for a removal.
	#[schemars(with = "Option<PricingModel>")]
	model: Option<Value>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PricingChanges")]
struct ChangesDto {
	changes: Vec<ChangeDto>,
}

/// A brand's last 50 pricing changes, the latest first; a removal's model, `valid_from` and
/// `needs` null.
async fn pricing_changes(State(panel): State<Panel>, Ids(BrandPath { brand }): Ids<BrandPath>) -> ApiResult<Json<ChangesDto>> {
	let changes = panel
		.pricing_history(&brand)
		.await?
		.into_iter()
		.map(|c| ChangeDto {
			id: c.id,
			at: c.at,
			by: c.by,
			kind: c.kind.as_str(),
			valid_from: c.valid_from,
			needs: c.needs,
			model: c.model,
		})
		.collect();
	Ok(Json(ChangesDto { changes }))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SetPricing")]
struct SetBody {
	#[schemars(with = "PricingModel")]
	model: Value,
	/// The `updated_at` the editor read; null (or absent) for pricing never set.
	#[serde(default)]
	expected_updated_at: Option<Timestamp>,
}

/// Saves a brand's price list; its sites take it on their next read.
///
/// 409 `stale` with the current pricing when `expected_updated_at` is not what it stands at;
/// 422 with the model's first problem (`code` one of `PRICING_CODES`).
async fn set_pricing(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Ids(BrandPath { brand }): Ids<BrandPath>, Body(b): Body<SetBody>) -> PricingResult<Json<ItemDto>> {
	let v = panel
		.set_pricing(&editor(&caller), &brand, &b.model, Expected::At(b.expected_updated_at), Timestamp::now())
		.await?;
	Ok(Json(v.into()))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "RemovePricing")]
struct RemoveBody {
	/// Required: taking a brand's prices off its sites over a version the editor did not see is
	/// not something to do by default. Null for pricing never set.
	#[serde(deserialize_with = "Option::deserialize")]
	#[schemars(required)]
	expected_updated_at: Option<Timestamp>,
}

/// Takes a brand's price list off its sites: they keep their baked one.
///
/// 409 as for a PUT.
async fn remove_pricing(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Ids(BrandPath { brand }): Ids<BrandPath>,
	Body(b): Body<RemoveBody>,
) -> PricingResult<Json<ItemDto>> {
	let v = panel.remove_pricing(&editor(&caller), &brand, Expected::At(b.expected_updated_at), Timestamp::now()).await?;
	Ok(Json(v.into()))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PricePreviewRequest")]
struct PreviewBody {
	#[schemars(with = "PricingModel")]
	model: Value,
	need: String,
	/// Input id → option id.
	#[serde(default)]
	inputs: BTreeMap<String, String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PricePreview")]
struct PreviewDto {
	/// Integer cents EUR TTC; null when the need is a quote.
	cents: Option<i64>,
}

/// What a model, saved or not, prices a need at with these answers; `cents` null is no price.
///
/// 422 as for a PUT: the draft is held to the brand's locales, as a site would hold it.
async fn preview_price(State(panel): State<Panel>, Ids(BrandPath { brand }): Ids<BrandPath>, Body(b): Body<PreviewBody>) -> PricingResult<Json<PreviewDto>> {
	let cents = panel.preview_price(&brand, &b.model, &b.need, &b.inputs).await?;
	Ok(Json(PreviewDto { cents }))
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
