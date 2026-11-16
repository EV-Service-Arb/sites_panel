//! A place's live settings over HTTP (`panel::place`): the editor's routes under `/api/v1`
//! behind the conductor's assertion, and the sites' read under `/api/internal`, which has none.
//!
//! ```text
//! GET  /api/internal/brands/{brand}/locations/{slug}?locale    PlaceLive, `{}` when none or
//!                                                              unknown; 404 only when withdrawn
//!                                                              (a brand's pricing beside it:
//!                                                              `crate::pricing`)
//! ```

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use axum::{
	Extension, Json, Router,
	error_handling::HandleErrorLayer,
	extract::{Path, State},
	http::{Method, StatusCode},
	response::{IntoResponse, Response},
	routing::get,
};
use ev_lib::api::{
	Created, GenContext, JsonSchema, OperationOutput,
	openapi::{Operation, Response as Answer, StatusCode as Status},
};
use jiff::Timestamp;
use panel::{
	Panel,
	place::{Expected, Live, PlaceError, PlaceView},
};
use panel_core::{
	ids::{BrandId, LocationId},
	place::{Editor, FieldErrors, PlaceLive, PlaceSettings},
};
use sa_auth::Places;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tower::{BoxError, ServiceBuilder, limit::GlobalConcurrencyLimitLayer};
use tower_http::timeout::TimeoutLayer;
use uuid::Uuid;

use crate::{
	api::{ApiError, ApiResult, Body, ErrorDto, Ids, present},
	conductor::{InternalEndpoint, InternalReads},
	gate::Caller,
	http::{ApiRoute, Gate, Section},
};

pub(crate) fn routes() -> Vec<ApiRoute> {
	use Method as M;
	let work = Gate::section(Section::Work);
	let edit = work.may(Places::Edit);
	vec![
		ApiRoute::new(M::GET, "/places/{brand}/{slug}/settings", work, place_settings),
		ApiRoute::new(M::GET, "/places/{brand}/{slug}/settings/history", work, place_history),
		ApiRoute::new(M::POST, "/places", edit, register_place),
		ApiRoute::new(M::PUT, "/places/{brand}/{slug}/settings", edit, set_place_settings),
		ApiRoute::new(M::POST, "/places/{brand}/{slug}/settings/revert/{id}", edit, revert_place_settings),
		ApiRoute::new(M::POST, "/places/{brand}/{slug}/withdraw", edit, withdraw_place),
		ApiRoute::new(M::POST, "/places/{brand}/{slug}/restore", edit, restore_place),
		ApiRoute::new(M::PUT, "/places/{brand}/{slug}/gmb", edit, link_gmb),
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
/// a site gives up after 3 s.
const INTERNAL_TIMEOUT: Duration = Duration::from_secs(3);

/// Why a change to a place failed, in the contract's shapes.
enum PlaceApiError {
	Api(ApiError),
	/// `expected_updated_at` is not what the place stands at.
	Conflict,
	GmbTaken,
	Exists,
	Invalid(FieldErrors),
}

/// Settings refused field by field.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PlaceInvalid")]
struct InvalidDto {
	/// `invalid`.
	error: &'static str,
	/// Why each field was refused, by its path.
	fields: FieldErrors,
}

impl IntoResponse for PlaceApiError {
	fn into_response(self) -> Response {
		let conflict = |error: &str| (StatusCode::CONFLICT, Json(ErrorDto { error: error.to_owned() })).into_response();
		match self {
			Self::Api(e) => e.into_response(),
			Self::Conflict => conflict("conflict"),
			Self::GmbTaken => conflict("gmb_taken"),
			Self::Exists => conflict("exists"),
			Self::Invalid(fields) => (StatusCode::UNPROCESSABLE_ENTITY, Json(InvalidDto { error: "invalid", fields })).into_response(),
		}
	}
}

impl OperationOutput for PlaceApiError {
	type Inner = ErrorDto;

	fn inferred_responses(ctx: &mut GenContext, op: &mut Operation) -> Vec<(Option<Status>, Answer)> {
		let mut answers = ApiError::inferred_responses(ctx, op);
		answers.extend(Json::<InvalidDto>::operation_response(ctx, op).map(|r| (Some(Status::Code(422)), r)));
		answers
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

impl From<PlaceError> for PlaceApiError {
	fn from(e: PlaceError) -> Self {
		match e {
			PlaceError::NotFound => Self::Api(ApiError::NotFound),
			PlaceError::Conflict => Self::Conflict,
			PlaceError::GmbTaken => Self::GmbTaken,
			PlaceError::Invalid(f) => Self::Invalid(f),
			PlaceError::Internal(e) => Self::Api(ApiError::Internal(e)),
		}
	}
}

type PlaceResult<T> = Result<T, PlaceApiError>;

/// Who a history names: the user's email, their id when concierge gave none. One rule for a
/// place's, a price list's and an experiment's changes.
pub(crate) fn editor(caller: &Caller) -> Editor {
	let label = if caller.email.trim().is_empty() { caller.user_id.to_string() } else { caller.email.clone() };
	Editor::User { id: caller.user_id, label }
}

/// `{brand}/{slug}`.
#[derive(Deserialize, JsonSchema)]
struct PlacePath {
	brand: BrandId,
	slug: LocationId,
}

/// A place: its live settings, who set them last, and where it stands.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Place")]
struct PlaceDto {
	brand: BrandId,
	slug: LocationId,
	/// Off the sites: they answer it as gone.
	withdrawn: bool,
	/// The conductor's GMB this place is the landing of.
	gmb_id: Option<i64>,
	/// Only the fields set; `{}` when none (the site serves its baked config).
	#[schemars(with = "PlaceLive")]
	settings: Value,
	/// Null while the settings were never set: what a first edit must expect.
	updated_at: Option<Timestamp>,
	updated_by: Option<String>,
	/// Whether the caller may change it.
	can_edit: bool,
}

fn place_dto(v: PlaceView, caller: &Caller) -> PlaceDto {
	PlaceDto {
		brand: v.brand,
		slug: v.slug,
		withdrawn: v.withdrawn,
		gmb_id: v.gmb_id,
		settings: v.settings.as_json(),
		updated_at: v.updated_at,
		updated_by: v.updated_by,
		can_edit: caller.permissions.may(Places::Edit),
	}
}

/// A place's live settings, as the sites are served them.
async fn place_settings(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Ids(PlacePath { brand, slug }): Ids<PlacePath>) -> ApiResult<Json<PlaceDto>> {
	Ok(Json(place_dto(panel.place(&brand, &slug).await?, &caller)))
}

/// One change of a place: its settings before and after.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PlaceChange")]
struct ChangeDto {
	id: Uuid,
	at: Timestamp,
	/// The user's email, else their id; `cli` from the pod.
	by: String,
	/// register | set | revert | withdraw | restore
	kind: &'static str,
	#[schemars(with = "PlaceLive")]
	before: Value,
	#[schemars(with = "PlaceLive")]
	after: Value,
	/// The change a revert went back before.
	reverts: Option<Uuid>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PlaceHistory")]
struct HistoryDto {
	changes: Vec<ChangeDto>,
}

/// A place's changes, the latest first: its settings before and after each.
async fn place_history(State(panel): State<Panel>, Ids(PlacePath { brand, slug }): Ids<PlacePath>) -> ApiResult<Json<HistoryDto>> {
	let changes = panel
		.place_history(&brand, &slug)
		.await?
		.into_iter()
		.map(|c| ChangeDto {
			id: c.id,
			at: c.at,
			by: c.by,
			kind: c.kind.as_str(),
			before: c.before.as_json(),
			after: c.after.as_json(),
			reverts: c.reverts,
		})
		.collect();
	Ok(Json(HistoryDto { changes }))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SetPlaceSettings")]
struct SetBody {
	/// The whole of the settings, not a patch: a field left out is cleared.
	#[schemars(with = "PlaceLive")]
	settings: Value,
	/// The `updated_at` the editor read; null (or absent) for settings never set.
	#[serde(default)]
	expected_updated_at: Option<Timestamp>,
}

/// Replaces a place's live settings, whole; the sites take them on their next read.
///
/// 409 `conflict` when `expected_updated_at` is not what the place stands at; 422 with every
/// refused field, keyed by the field or its path in a list (`hours[0].opens`, `serviceArea[2]`).
async fn set_place_settings(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Ids(PlacePath { brand, slug }): Ids<PlacePath>,
	Body(b): Body<SetBody>,
) -> PlaceResult<Json<PlaceDto>> {
	let settings = PlaceSettings::parse(&b.settings).map_err(PlaceApiError::Invalid)?;
	let v = panel
		.set_place(&editor(&caller), &brand, &slug, settings, Expected::At(b.expected_updated_at), Timestamp::now())
		.await?;
	Ok(Json(place_dto(v, &caller)))
}

/// `{brand}/{slug}/settings/revert/{id}`.
#[derive(Deserialize, JsonSchema)]
struct RevertPath {
	brand: BrandId,
	slug: LocationId,
	/// The change to go back before.
	id: Uuid,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "RevertPlaceSettings")]
struct RevertBody {
	/// Checked as a PUT checks it: null for settings never set. Absent: over whatever is there.
	#[serde(default, deserialize_with = "present")]
	#[schemars(with = "Option<Timestamp>")]
	expected_updated_at: Option<Option<Timestamp>>,
}

/// Puts back the settings a change found (its `before`), journaled as a revert.
///
/// The body is optional: none, `null` or `{}` reverts over whatever is there. 404 for a change
/// not of this place; 409 `conflict` as for a PUT.
async fn revert_place_settings(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Ids(RevertPath { brand, slug, id }): Ids<RevertPath>,
	b: Option<Body<RevertBody>>,
) -> PlaceResult<Json<PlaceDto>> {
	let expected = match b.and_then(|Body(b)| b.expected_updated_at) {
		None => Expected::Any,
		Some(at) => Expected::At(at),
	};
	let v = panel.revert_place(&editor(&caller), &brand, &slug, id, expected, Timestamp::now()).await?;
	Ok(Json(place_dto(v, &caller)))
}

/// Takes a place off the sites: they answer it as gone. Its settings are kept.
async fn withdraw_place(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Ids(PlacePath { brand, slug }): Ids<PlacePath>) -> PlaceResult<Json<PlaceDto>> {
	let v = panel.withdraw_place(&editor(&caller), &brand, &slug, true, Timestamp::now()).await?;
	Ok(Json(place_dto(v, &caller)))
}

/// Puts a withdrawn place back on the sites.
async fn restore_place(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Ids(PlacePath { brand, slug }): Ids<PlacePath>) -> PlaceResult<Json<PlaceDto>> {
	let v = panel.withdraw_place(&editor(&caller), &brand, &slug, false, Timestamp::now()).await?;
	Ok(Json(place_dto(v, &caller)))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "GmbLink")]
struct GmbBody {
	/// Required: `null` unlinks.
	#[serde(deserialize_with = "Option::deserialize")]
	#[schemars(required)]
	gmb_id: Option<i64>,
}

/// Links a place to the conductor's GMB it is the landing of, or unlinks it.
///
/// 404 for a place not known; 409 `gmb_taken` when another place has that GMB.
async fn link_gmb(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Ids(PlacePath { brand, slug }): Ids<PlacePath>, Body(b): Body<GmbBody>) -> PlaceResult<Json<PlaceDto>> {
	let v = panel.link_gmb(&brand, &slug, b.gmb_id, Timestamp::now()).await?;
	Ok(Json(place_dto(v, &caller)))
}

/// A GMB's place and how its leads stand.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "GmbPlace")]
struct GmbPlaceDto {
	brand: String,
	location: String,
	/// Leads created in the last 7 days.
	leads_7d: u32,
	/// Leads overdue for their first contact.
	overdue: u32,
}

/// The places linked to a GMB, by the GMB's id, with their leads of the last 7 days and those
/// overdue for a first contact.
async fn gmbs(State(panel): State<Panel>) -> ApiResult<Json<BTreeMap<i64, GmbPlaceDto>>> {
	let rows = panel.gmbs(Timestamp::now()).await?;
	Ok(Json(
		rows.into_iter()
			.map(|(id, brand, location, leads_7d, overdue, ..)| (id, GmbPlaceDto { brand, location, leads_7d, overdue }))
			.collect(),
	))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "RegisterPlace")]
struct RegisterBody {
	brand: BrandId,
	slug: LocationId,
}

/// Registers a place by hand, before it has a lead.
///
/// 409 `exists` for a place the panel knows.
async fn register_place(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Body(b): Body<RegisterBody>) -> PlaceResult<Created<PlaceDto>> {
	let (v, added) = panel.register_place(&editor(&caller), &b.brand, &b.slug, Timestamp::now()).await?;
	if !added {
		return Err(PlaceApiError::Exists);
	}
	Ok(Created(place_dto(v, &caller)))
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
