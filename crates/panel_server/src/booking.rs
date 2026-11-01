//! Booking over HTTP (`panel::booking`): an operator's routes under `/api/v1`, and the
//! providers' webhooks under `/api/hooks/booking`, which take no assertion — a provider's
//! signature, checked by its adapter, is what lets one in.
//!
//! ```text
//! POST /api/v1/leads/{brand}/{lead}/booking          {action: "set", start_at, end_at?} | {action: "clear"}
//!                                                    → 201 {event_id}; 409 not from where it stands
//! POST /api/v1/leads/{brand}/{lead}/booking/status   {status: done | no_show | canceled} → 201; 409
//! GET  /api/v1/bookings/unmatched?brand&limit         {bookings: [Booking]}, the next slot first
//! POST /api/v1/bookings/{id}/attach                   {lead} → 201 {event_id}; 404; 409
//! POST /api/hooks/booking/{provider}/{brand}          a push provider's webhook → 200 {written, …};
//!                                                    404 for a provider not registered (none is, yet)
//! ```

use std::{sync::Arc, time::Duration};

use axum::{
	Extension, Json, Router,
	body::{Body, to_bytes},
	error_handling::HandleErrorLayer,
	extract::{Path, Query, State, rejection::JsonRejection},
	http::{HeaderMap, Method, StatusCode},
	response::{IntoResponse, Response},
	routing::post,
};
use jiff::Timestamp;
use panel::{
	Panel,
	booking::{BookingView, Provider, PushError, PushRequest, PushSources, SlotAction},
	operator::Actor,
};
use panel_core::ids::{BrandId, LeadId};
use sa_auth::Leads;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tower::{BoxError, ServiceBuilder, limit::GlobalConcurrencyLimitLayer};
use tower_http::timeout::TimeoutLayer;
use uuid::Uuid;

use crate::{
	api::{ApiError, ApiResult, body},
	gate::Caller,
	http::{ApiRoute, Section},
};

/// The operator's booking routes.
pub(crate) fn routes() -> Vec<ApiRoute> {
	use Method as M;
	let work = Some(Section::Work);
	vec![
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/booking", work, slot),
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/booking/status", work, close),
		ApiRoute::new(M::GET, "/bookings/unmatched", work, unmatched),
		ApiRoute::new(M::POST, "/bookings/{id}/attach", work, attach),
	]
}

/// A webhook's body at most: a booking is a few KiB.
pub const MAX_HOOK_BODY: usize = 256 * 1024;
/// Webhooks served at once; past it, 503 (the provider retries).
const HOOKS_CONCURRENT: usize = 8;
const HOOK_TIMEOUT: Duration = Duration::from_secs(10);

/// What a webhook needs: the engine and the push providers registered.
#[derive(Clone, Debug)]
pub struct Hooks {
	pub panel: Panel,
	pub sources: PushSources,
}

/// `POST /api/hooks/booking/{provider}/{brand}`: mounted without an assertion. Bounded on its own
/// budget; the edge rate-limits `/api/hooks` per client IP.
pub fn hooks(panel: Panel, sources: PushSources) -> Router {
	let layers = ServiceBuilder::new()
		.layer(HandleErrorLayer::new(|_: BoxError| async { (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": "busy" }))) }))
		.load_shed()
		.layer(GlobalConcurrencyLimitLayer::with_semaphore(Arc::new(Semaphore::new(HOOKS_CONCURRENT))))
		.layer(TimeoutLayer::with_status_code(StatusCode::SERVICE_UNAVAILABLE, HOOK_TIMEOUT));
	Router::new()
		.route("/api/hooks/booking/{provider}/{brand}", post(hook).layer(layers))
		.with_state(Hooks { panel, sources })
}

fn not_found() -> Response {
	(StatusCode::NOT_FOUND, Json(json!({ "error": "not_found" }))).into_response()
}

async fn hook(State(hooks): State<Hooks>, Path((provider, brand)): Path<(String, String)>, headers: HeaderMap, body: Body) -> Response {
	// A provider that is not one, or not registered, and a brand that cannot be one, answer
	// alike: nothing here to post to.
	let (Ok(provider), Ok(brand)) = (Provider::parse(&provider), BrandId::parse(&brand)) else {
		return not_found();
	};
	let Some(source) = hooks.sources.get(provider) else { return not_found() };
	let body = match to_bytes(body, MAX_HOOK_BODY).await {
		Ok(b) => b,
		Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, Json(json!({ "error": "the body is too large or did not arrive" }))).into_response(),
	};
	let headers: Vec<(String, String)> = headers
		.iter()
		.filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_ascii_lowercase(), v.to_owned())))
		.collect();
	let now = Timestamp::now();
	let request = PushRequest {
		brand: &brand,
		headers: &headers,
		body: &body,
		now,
	};
	let events = match source.verify(&request) {
		Ok(events) => events,
		Err(PushError::Unauthorized) => {
			tracing::warn!(%provider, %brand, "booking webhook: refused");
			return (StatusCode::UNAUTHORIZED, Json(json!({ "error": "unauthorized" }))).into_response();
		}
		Err(PushError::BadRequest(why)) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": why }))).into_response(),
	};
	match hooks.panel.ingest_bookings(&brand, events, now).await {
		Ok(done) => Json(json!({
			"written": done.written,
			"duplicate": done.duplicate,
			"unchanged": done.unchanged,
			"ignored": done.ignored,
			"refused": done.refused,
		}))
		.into_response(),
		Err(e) => {
			// A 5xx: the provider retries, and the event's id makes the retry a duplicate.
			crate::report(&e, "a booking webhook");
			(StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": "internal error" }))).into_response()
		}
	}
}

fn ids(brand: &str, lead: &str) -> ApiResult<(BrandId, LeadId)> {
	Ok((BrandId::parse(brand)?, LeadId::parse(lead)?))
}

fn created(event: Uuid, replayed: bool) -> Response {
	let status = if replayed { StatusCode::OK } else { StatusCode::CREATED };
	(status, Json(json!({ "event_id": event.to_string() }))).into_response()
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum SlotBody {
	Set { start_at: String, end_at: Option<String> },
	Clear,
}

async fn slot(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Path((brand, lead)): Path<(String, String)>,
	headers: HeaderMap,
	b: Result<Json<SlotBody>, JsonRejection>,
) -> ApiResult<Response> {
	if !caller.permissions.may(Leads::Edit) {
		return Err(ApiError::Forbidden);
	}
	let key = crate::api::idempotency_key(&headers)?;
	let (brand, lead) = ids(&brand, &lead)?;
	let action = match body(b)? {
		SlotBody::Set { start_at, end_at } => SlotAction::Set { start_at, end_at },
		SlotBody::Clear => SlotAction::Clear,
	};
	let done = panel.book_once(Actor(caller.user_id), &brand, &lead, action, Timestamp::now(), key.as_deref()).await?;
	Ok(created(done.value.raw(), done.replayed))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CloseBody {
	status: String,
}

async fn close(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Path((brand, lead)): Path<(String, String)>,
	headers: HeaderMap,
	b: Result<Json<CloseBody>, JsonRejection>,
) -> ApiResult<Response> {
	if !caller.permissions.may(Leads::Edit) {
		return Err(ApiError::Forbidden);
	}
	let key = crate::api::idempotency_key(&headers)?;
	let (brand, lead) = ids(&brand, &lead)?;
	let status = body(b)?.status;
	let done = panel.close_booking_once(Actor(caller.user_id), &brand, &lead, &status, Timestamp::now(), key.as_deref()).await?;
	Ok(created(done.value.raw(), done.replayed))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnmatchedQuery {
	brand: Option<String>,
	limit: Option<u32>,
}

/// A provider's booking as the front end reads it.
pub(crate) fn booking_body(v: BookingView) -> Value {
	let r = v.row;
	let mut b = json!({
		"id": r.id.to_string(),
		"brand": r.brand_id.as_str(),
		"provider": r.provider.as_str(),
		"external_ref": r.external_ref,
		"status": if r.booked { "booked" } else { "canceled" },
		"start_at": r.start_at.to_string(),
		"end_at": r.end_at.map(|t| t.to_string()),
		"booked_at": r.booked_at.map(|t| t.to_string()),
		"last_event_at": r.last_event_at.to_string(),
		"lead_id": r.lead.as_ref().map(|(l, _)| l.as_str()),
		"match": r.lead.as_ref().map(|(_, m)| m.as_str()),
	});
	if let Some(contact) = v.contact {
		b["contact"] = contact;
	}
	b
}

async fn unmatched(State(panel): State<Panel>, Extension(caller): Extension<Caller>, q: Result<Query<UnmatchedQuery>, axum::extract::rejection::QueryRejection>) -> ApiResult<Json<Value>> {
	let Query(q) = q.map_err(|e| ApiError::BadRequest(e.body_text()))?;
	let brand = q.brand.as_deref().map(BrandId::parse).transpose()?;
	let pii = crate::api::pii(&caller);
	let rows = panel.unmatched_bookings(brand.as_ref(), pii, q.limit.unwrap_or(100)).await?;
	Ok(Json(json!({ "bookings": rows.into_iter().map(booking_body).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachBody {
	lead: String,
}

async fn attach(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path(id): Path<String>, b: Result<Json<AttachBody>, JsonRejection>) -> ApiResult<Response> {
	if !caller.permissions.may(Leads::Edit) {
		return Err(ApiError::Forbidden);
	}
	let id = Uuid::parse_str(&id).map_err(|_| ApiError::NotFound)?;
	let lead = LeadId::parse(&body(b)?.lead)?;
	let event = panel.attach_booking(Actor(caller.user_id), id, &lead, Timestamp::now()).await?;
	Ok(created(event.raw(), false))
}
