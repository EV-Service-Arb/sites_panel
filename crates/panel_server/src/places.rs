//! A place's live settings over HTTP (`panel::place`): the editor's routes under `/api/v1`
//! behind the conductor's assertion, and the sites' read under `/api/internal`, which has none.
//!
//! ```text
//! GET  /api/v1/places/{brand}/{slug}/settings                 {brand, slug, withdrawn, settings,
//!                                                              updated_at, updated_by, can_edit}
//! PUT  /api/v1/places/{brand}/{slug}/settings        admin    {settings, expected_updated_at}
//!                                                              → 200 as GET; 409 stale; 422 fields
//! GET  /api/v1/places/{brand}/{slug}/settings/history         {changes: [{id, at, by, kind, before, after}]}
//! POST /api/v1/places/{brand}/{slug}/settings/revert/{id}  admin  [{expected_updated_at}]
//!                                                              → 200 as GET; 404; 409 stale
//! POST /api/v1/places/{brand}/{slug}/withdraw | /restore   admin  → 200 as GET
//! PUT  /api/v1/places/{brand}/{slug}/gmb          admin    {gmb_id: n | null}
//!                                                              → 200 as GET; 404; 409 {"error": "gmb_taken"}
//! GET  /api/v1/gmbs                                            {"<gmb_id>": {brand, location}}
//! POST /api/v1/places                                admin    {brand, slug} → 201 as GET;
//!                                                              409 {"error": "exists"}
//! GET  /api/internal/brands/{brand}/locations/{slug}?locale    PlaceLive, `{}` when none or
//!                                                              unknown; 404 only when withdrawn
//!                                                              (a brand's pricing beside it:
//!                                                              `crate::pricing`)
//! ```

use std::{sync::Arc, time::Duration};

use axum::{
	Extension, Json, Router,
	body::Bytes,
	error_handling::HandleErrorLayer,
	extract::{Path, State, rejection::JsonRejection},
	http::{Method, StatusCode},
	response::{IntoResponse, Response},
	routing::get,
};
use jiff::Timestamp;
use panel::{
	Panel,
	place::{Expected, Live, PlaceError, PlaceView},
};
use panel_core::{
	ids::{BrandId, LocationId},
	place::{Editor, PlaceSettings},
};
use sa_auth::Places;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tower::{BoxError, ServiceBuilder, limit::GlobalConcurrencyLimitLayer};
use tower_http::timeout::TimeoutLayer;
use uuid::Uuid;

use crate::{
	api::ApiError,
	conductor::{InternalEndpoint, InternalReads},
	gate::Caller,
	http::{ApiRoute, Section},
};

pub(crate) fn routes() -> Vec<ApiRoute> {
	use Method as M;
	let work = Some(Section::Work);
	vec![
		ApiRoute::new(M::GET, "/places/{brand}/{slug}/settings", work, settings),
		ApiRoute::new(M::GET, "/places/{brand}/{slug}/settings/history", work, history),
		ApiRoute::new(M::POST, "/places", work, register),
		ApiRoute::new(M::PUT, "/places/{brand}/{slug}/settings", work, set),
		ApiRoute::new(M::POST, "/places/{brand}/{slug}/settings/revert/{id}", work, revert),
		ApiRoute::new(M::POST, "/places/{brand}/{slug}/withdraw", work, withdraw),
		ApiRoute::new(M::POST, "/places/{brand}/{slug}/restore", work, restore),
		ApiRoute::new(M::PUT, "/places/{brand}/{slug}/gmb", work, link_gmb),
		ApiRoute::new(M::GET, "/gmbs", work, gmbs),
	]
}

/// The sites' reads — a place's settings, a brand's pricing and experiments. Bounded like ingest, with one budget for both (the same sites call both);
/// a site answered 503 serves its baked place, so shedding costs a site nothing. The pricing
/// route is answered `{}` instead of a 503: its contract has no 5xx.
pub fn internal() -> Router<Panel> {
	let permits = Arc::new(Semaphore::new(INTERNAL_CONCURRENT));
	let places = ServiceBuilder::new()
		.layer(HandleErrorLayer::new(|_: BoxError| async { (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "busy" }))) }))
		.load_shed()
		.layer(GlobalConcurrencyLimitLayer::with_semaphore(permits.clone()))
		.layer(TimeoutLayer::with_status_code(StatusCode::SERVICE_UNAVAILABLE, INTERNAL_TIMEOUT));
	// Shed only: the handler bounds its own time, and answers `{}` past it.
	let pricing = ServiceBuilder::new()
		.layer(HandleErrorLayer::new(|_: BoxError| async { Json(json!({})) }))
		.load_shed()
		.layer(GlobalConcurrencyLimitLayer::with_semaphore(permits.clone()));
	// The same, answering no overrides.
	let experiments = ServiceBuilder::new()
		.layer(HandleErrorLayer::new(|_: BoxError| async { Json(json!({ "experiments": {} })) }))
		.load_shed()
		.layer(GlobalConcurrencyLimitLayer::with_semaphore(permits));
	Router::new()
		.route("/brands/{brand}/locations/{slug}", get(live).layer(places))
		.route("/brands/{brand}/pricing", get(crate::pricing::live).layer(pricing.clone()))
		.route("/brands/{brand}/experiments", get(crate::experiments::live).layer(experiments))
}

/// The sites' reads at once, places, pricing and experiments together.
const INTERNAL_CONCURRENT: usize = 32;
/// kitstart gives up after 3 s.
const INTERNAL_TIMEOUT: Duration = Duration::from_secs(3);

/// Why a change to a place failed, in the contract's shapes.
enum PlaceApiError {
	Api(ApiError),
	Conflict,
	GmbTaken,
	Invalid(panel_core::place::FieldErrors),
}

impl IntoResponse for PlaceApiError {
	fn into_response(self) -> Response {
		match self {
			Self::Api(e) => e.into_response(),
			Self::Conflict => (StatusCode::CONFLICT, Json(json!({ "error": "conflict" }))).into_response(),
			Self::GmbTaken => (StatusCode::CONFLICT, Json(json!({ "error": "gmb_taken" }))).into_response(),
			Self::Invalid(fields) => (StatusCode::UNPROCESSABLE_ENTITY, Json(json!({ "error": "invalid", "fields": fields }))).into_response(),
		}
	}
}

impl From<ApiError> for PlaceApiError {
	fn from(e: ApiError) -> Self {
		Self::Api(e)
	}
}

impl From<eyre::Report> for PlaceApiError {
	fn from(e: eyre::Report) -> Self {
		Self::Api(ApiError::Internal(e))
	}
}

fn place_error(e: PlaceError) -> PlaceApiError {
	match e {
		PlaceError::NotFound => PlaceApiError::Api(ApiError::NotFound),
		PlaceError::Conflict => PlaceApiError::Conflict,
		PlaceError::GmbTaken => PlaceApiError::GmbTaken,
		PlaceError::Invalid(f) => PlaceApiError::Invalid(f),
		PlaceError::Internal(e) => PlaceApiError::Api(ApiError::Internal(e)),
	}
}

type PlaceResult<T> = Result<T, PlaceApiError>;

fn ids(brand: &str, slug: &str) -> Result<(BrandId, LocationId), ApiError> {
	Ok((BrandId::parse(brand)?, LocationId::parse(slug)?))
}

fn admin(caller: &Caller) -> Result<(), ApiError> {
	if caller.permissions.may(Places::Edit) { Ok(()) } else { Err(ApiError::Forbidden) }
}

/// Who a history names: the user's email, their id when concierge gave none. One rule for a
/// place's, a price list's and an experiment's changes.
pub(crate) fn editor(caller: &Caller) -> Editor {
	let label = if caller.email.trim().is_empty() { caller.user_id.to_string() } else { caller.email.clone() };
	Editor::User { id: caller.user_id, label }
}

fn body(v: PlaceView, caller: &Caller) -> Value {
	json!({
		"brand": v.brand.as_str(),
		"slug": v.slug.as_str(),
		"withdrawn": v.withdrawn,
		"gmb_id": v.gmb_id,
		"settings": v.settings.as_json(),
		"updated_at": v.updated_at.map(|t| t.to_string()),
		"updated_by": v.updated_by,
		"can_edit": caller.permissions.may(Places::Edit),
	})
}

async fn settings(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, slug)): Path<(String, String)>) -> PlaceResult<Json<Value>> {
	let (brand, slug) = ids(&brand, &slug)?;
	Ok(Json(body(panel.place(&brand, &slug).await?, &caller)))
}

async fn history(State(panel): State<Panel>, Path((brand, slug)): Path<(String, String)>) -> PlaceResult<Json<Value>> {
	let (brand, slug) = ids(&brand, &slug)?;
	let changes: Vec<Value> = panel
		.place_history(&brand, &slug)
		.await?
		.into_iter()
		.map(|c| {
			json!({
				"id": c.id.to_string(),
				"at": c.at.to_string(),
				"by": c.by,
				"kind": c.kind.as_str(),
				"before": c.before.as_json(),
				"after": c.after.as_json(),
				"reverts": c.reverts.map(|r| r.to_string()),
			})
		})
		.collect();
	Ok(Json(json!({ "changes": changes })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetBody {
	settings: Value,
	/// The `updated_at` the editor read; null (or absent) for settings never set.
	#[serde(default)]
	expected_updated_at: Option<String>,
}

fn json_body<T: serde::de::DeserializeOwned>(b: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
	b.map(|Json(v)| v).map_err(|e| ApiError::BadRequest(e.body_text()))
}

async fn set(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Path((brand, slug)): Path<(String, String)>,
	b: Result<Json<SetBody>, JsonRejection>,
) -> PlaceResult<Json<Value>> {
	admin(&caller)?;
	let (brand, slug) = ids(&brand, &slug)?;
	let b = json_body(b)?;
	let expected = expected_at(b.expected_updated_at.as_deref())?;
	let settings = PlaceSettings::parse(&b.settings).map_err(PlaceApiError::Invalid)?;
	let v = panel
		.set_place(&editor(&caller), &brand, &slug, settings, Expected::At(expected), Timestamp::now())
		.await
		.map_err(place_error)?;
	Ok(Json(body(v, &caller)))
}

/// `expected_updated_at` as the editor sent it: null for settings never set.
fn expected_at(raw: Option<&str>) -> Result<Option<Timestamp>, ApiError> {
	raw.map(|raw| {
		raw.parse::<Timestamp>()
			.map_err(|_| ApiError::BadRequest("expected_updated_at is not an RFC 3339 instant".into()))
	})
	.transpose()
}

/// A revert's body, optional: `{"expected_updated_at": "…" | null}` checks it as a PUT does;
/// no body, `null` or `{}` reverts over whatever is there.
fn revert_expected(raw: &[u8]) -> Result<Expected, ApiError> {
	if raw.iter().all(u8::is_ascii_whitespace) {
		return Ok(Expected::Any);
	}
	let v: Value = serde_json::from_slice(raw).map_err(|e| ApiError::BadRequest(format!("the body is not JSON: {e}")))?;
	match v {
		Value::Null => Ok(Expected::Any),
		Value::Object(m) => {
			if let Some(k) = m.keys().find(|k| *k != "expected_updated_at") {
				return Err(ApiError::BadRequest(format!("unknown field `{k}`, expected `expected_updated_at`")));
			}
			match m.get("expected_updated_at") {
				None => Ok(Expected::Any),
				Some(Value::Null) => Ok(Expected::At(None)),
				Some(Value::String(t)) => Ok(Expected::At(expected_at(Some(t))?)),
				Some(_) => Err(ApiError::BadRequest("expected_updated_at is a string or null".into())),
			}
		}
		_ => Err(ApiError::BadRequest("the body is {\"expected_updated_at\": …}".into())),
	}
}

async fn revert(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, slug, id)): Path<(String, String, String)>, raw: Bytes) -> PlaceResult<Json<Value>> {
	admin(&caller)?;
	let (brand, slug) = ids(&brand, &slug)?;
	let id = Uuid::parse_str(&id).map_err(|_| ApiError::NotFound)?;
	let expected = revert_expected(&raw)?;
	let v = panel.revert_place(&editor(&caller), &brand, &slug, id, expected, Timestamp::now()).await.map_err(place_error)?;
	Ok(Json(body(v, &caller)))
}

async fn withdrawal(panel: &Panel, caller: &Caller, brand: &str, slug: &str, withdrawn: bool) -> PlaceResult<Json<Value>> {
	admin(caller)?;
	let (brand, slug) = ids(brand, slug)?;
	let v = panel.withdraw_place(&editor(caller), &brand, &slug, withdrawn, Timestamp::now()).await.map_err(place_error)?;
	Ok(Json(body(v, caller)))
}

async fn withdraw(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, slug)): Path<(String, String)>) -> PlaceResult<Json<Value>> {
	withdrawal(&panel, &caller, &brand, &slug, true).await
}

async fn restore(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, slug)): Path<(String, String)>) -> PlaceResult<Json<Value>> {
	withdrawal(&panel, &caller, &brand, &slug, false).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GmbBody {
	/// Required: `null` unlinks, an absent field is refused.
	#[serde(deserialize_with = "Option::deserialize")]
	gmb_id: Option<i64>,
}

async fn link_gmb(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Path((brand, slug)): Path<(String, String)>,
	b: Result<Json<GmbBody>, JsonRejection>,
) -> PlaceResult<Json<Value>> {
	admin(&caller)?;
	let (brand, slug) = ids(&brand, &slug)?;
	let b = json_body(b)?;
	let v = panel.link_gmb(&brand, &slug, b.gmb_id, Timestamp::now()).await.map_err(place_error)?;
	Ok(Json(body(v, &caller)))
}

async fn gmbs(State(panel): State<Panel>) -> PlaceResult<Json<Value>> {
	let rows = panel.gmbs().await?;
	let by_id: serde_json::Map<String, Value> = rows
		.into_iter()
		.map(|(id, brand, location)| (id.to_string(), json!({ "brand": brand, "location": location })))
		.collect();
	Ok(Json(Value::Object(by_id)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterBody {
	brand: String,
	slug: String,
}

async fn register(State(panel): State<Panel>, Extension(caller): Extension<Caller>, b: Result<Json<RegisterBody>, JsonRejection>) -> PlaceResult<Response> {
	admin(&caller)?;
	let b = json_body(b)?;
	let (brand, slug) = ids(&b.brand, &b.slug)?;
	let (v, added) = panel.register_place(&editor(&caller), &brand, &slug, Timestamp::now()).await?;
	if !added {
		return Ok((StatusCode::CONFLICT, Json(json!({ "error": "exists" }))).into_response());
	}
	Ok((StatusCode::CREATED, Json(body(v, &caller))).into_response())
}

/// What a site gets. `locale` is accepted and not needed: a landmark is stored with every
/// locale's text, and the site picks its own.
async fn live(State(panel): State<Panel>, Extension(reads): Extension<Arc<InternalReads>>, Path((brand, slug)): Path<(String, String)>) -> Response {
	// A brand or slug that could not name a place is a place the panel does not know: `{}`,
	// never the 404 that would take a live page down.
	let (Ok(brand), Ok(slug)) = (BrandId::parse(&brand), LocationId::parse(&slug)) else {
		return Json(json!({})).into_response();
	};
	reads.record(&brand, InternalEndpoint::Locations);
	match panel.live_place(&brand, &slug).await {
		Ok(Live::Settings(s)) => Json(s.as_json()).into_response(),
		Ok(Live::Withdrawn) => (StatusCode::NOT_FOUND, Json(json!({ "error": "not_found" }))).into_response(),
		Err(e) => {
			// The site keeps its baked place on a 5xx: an outage here delays a change, no more.
			crate::report(&e, "serving a place to a site");
			(StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "internal error" }))).into_response()
		}
	}
}
