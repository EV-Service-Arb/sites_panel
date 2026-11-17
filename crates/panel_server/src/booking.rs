//! Booking over HTTP (`panel::booking`): an operator's routes under `/api/v1`, and the
//! providers' webhooks under `/api/hooks/booking`, which take no assertion — a provider's
//! signature, checked by its adapter, is what lets one in.
//!
//! ```text
//! POST /api/hooks/booking/{provider}/{brand}   a push provider's webhook → 200 {written, …};
//!                                              404 for a provider not registered (none is, yet)
//! ```

use std::{sync::Arc, time::Duration};

use axum::{
	Extension, Json, Router,
	body::{Body as Raw, to_bytes},
	error_handling::HandleErrorLayer,
	extract::{Path, State},
	http::{HeaderMap, Method, StatusCode},
	response::{Html, IntoResponse, Response},
	routing::post,
};
use ev_lib::api::{Body, Created, Ids, JsonSchema, Params, Refusal, Refuses, Upserted};
use jiff::Timestamp;
use panel::{
	Panel,
	booking::{BookingView, Provider, PushError, PushRequest, PushSources, SlotAction},
	operator::Actor,
};
use panel_core::{booking::BookingMatch, ids::LeadId};
use sa_auth::{Gate, Leads, Places, Work};
use sa_core::ids::BrandId;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tower::{BoxError, ServiceBuilder, limit::GlobalConcurrencyLimitLayer};
use tower_http::timeout::TimeoutLayer;
use uuid::Uuid;

use crate::{
	api::{ApiResult, Caller, Code, IdempotencyKey, Journaled, LeadPath, recorded},
	google_calendar::{ConsentError, Consents},
	http::ApiRoute,
	places::editor,
};

/// The operator's booking routes.
pub(crate) fn routes() -> Vec<ApiRoute> {
	use Method as M;
	let work = Gate::USER.needs(Work::Read);
	let edit = Gate::USER.needs(Leads::Edit);
	vec![
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/booking", edit, book_lead),
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/booking/status", edit, close_booking),
		ApiRoute::new(M::GET, "/bookings/unmatched", work, unmatched_bookings),
		ApiRoute::new(M::POST, "/bookings/{id}/attach", edit, attach_booking),
		ApiRoute::new(M::POST, "/bookings/google_calendar/{brand}/consent", Gate::USER.needs(Places::Edit), google_consent),
		ApiRoute::new(M::GET, "/bookings/google_calendar/callback", Gate::USER.needs(Places::Edit), google_callback),
	]
}

/// The owners' consent flow, when the Google OAuth client is configured.
type MaybeConsents = Option<Extension<Arc<Consents>>>;

fn consents(c: MaybeConsents) -> ApiResult<Arc<Consents>> {
	c.map(|Extension(c)| c)
		.ok_or_else(|| Refusal::new(Code::GoogleUnavailable, "no Google OAuth client is configured"))
}

impl Refuses<ConsentError> for Code {
	fn refusal(e: ConsentError) -> Refusal<Self> {
		match e {
			ConsentError::Elsewhere(_) => Refusal::new(Self::BadRequest, e.to_string()),
			ConsentError::Unknown => Refusal::new(Self::ConsentExpired, e.to_string()),
			ConsentError::Refused(_) => Refusal::new(Self::GoogleRefused, e.to_string()),
			ConsentError::Internal(e) => Self::refusal(e),
		}
	}
}

/// `{brand}`.
#[derive(Deserialize, JsonSchema)]
struct BrandPath {
	brand: BrandId,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "GoogleConsentAsked")]
struct ConsentBody {
	/// The calendar to read; the owner's own (`primary`) when absent.
	calendar_id: Option<String>,
	/// The page to come back to once Google answered: one of the conductor's.
	#[schemars(with = "String", url)]
	return_to: url::Url,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "GoogleConsentUrl")]
struct ConsentDto {
	/// Google's consent page, for the brand's calendar owner to open.
	url: String,
}

/// Asks the brand's calendar owner to let the panel read it: Google's consent page, which sends
/// them back through the callback to `return_to`.
async fn google_consent(caller: Caller, c: MaybeConsents, Ids(BrandPath { brand }): Ids<BrandPath>, Body(b): Body<ConsentBody>) -> ApiResult<Json<ConsentDto>> {
	let calendar_id = b.calendar_id.unwrap_or_else(|| "primary".to_owned());
	if !(1..=1024).contains(&calendar_id.len()) {
		return Err(Refusal::new(Code::BadRequest, "calendar_id is 1 to 1024 characters"));
	}
	let url = consents(c)?.ask(brand, calendar_id, b.return_to, editor(&caller), Timestamp::now())?;
	Ok(Json(ConsentDto { url }))
}

#[derive(Deserialize, JsonSchema)]
#[schemars(rename = "GoogleConsentAnswer")]
struct CallbackQuery {
	state: String,
	code: Option<String>,
	/// Google's word when the owner said no.
	error: Option<String>,
}

/// Google's redirect after the owner's consent: the brand's refresh token kept, sealed, and a
/// page that sends the owner back to the one the consent was asked from. 409 `consent_expired` for a consent
/// asked more than 10 minutes ago, or never.
async fn google_callback(State(panel): State<Panel>, c: MaybeConsents, Params(q): Params<CallbackQuery>) -> ApiResult<Html<String>> {
	let code = q.code.as_deref().ok_or(q.error.as_deref().unwrap_or("no code"));
	let now = Timestamp::now();
	let (consent, by, back) = consents(c)?.answer(&q.state, code, now).await?;
	panel.connect_google_calendar(&by, &consent, now).await?;
	// A page that goes on to `back`: the table's answers are 2xx, so not a 303.
	let back = back.as_str().replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;");
	Ok(Html(format!(
		"<!doctype html><meta charset=utf-8><meta http-equiv=refresh content=\"0;url={back}\"><title>Google Calendar connected</title><a href=\"{back}\">Google Calendar connected: back to the panel</a>"
	)))
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

async fn hook(State(hooks): State<Hooks>, Path((provider, brand)): Path<(String, String)>, headers: HeaderMap, body: Raw) -> Response {
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

/// An operator's slot for a lead.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "BookingSlot")]
enum SlotBody {
	/// Booked for this slot, RFC 3339 instants.
	Set { start_at: String, end_at: Option<String> },
	/// No slot any more.
	Clear,
}

/// Sets or clears a lead's booked slot by hand (`booking.set`, `booking.cleared`).
///
/// 400 for an instant that is not one; 409 when the booking is not where the action starts from.
async fn book_lead(
	State(panel): State<Panel>,
	caller: Caller,
	Ids(LeadPath { brand, lead }): Ids<LeadPath>,
	IdempotencyKey(key): IdempotencyKey,
	Body(b): Body<SlotBody>,
) -> ApiResult<Upserted<Journaled>> {
	let action = match b {
		SlotBody::Set { start_at, end_at } => SlotAction::Set { start_at, end_at },
		SlotBody::Clear => SlotAction::Clear,
	};
	let done = panel.book_once(Actor(caller.user_id), &brand, &lead, action, Timestamp::now(), key.as_deref()).await?;
	Ok(recorded(done, |e| Journaled { event_id: e.raw() }))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "BookingEnd")]
struct CloseBody {
	/// done | no_show | canceled
	status: String,
}

/// How a lead's booking ended (`booking.status_changed`).
///
/// 409 unless the lead is booked.
async fn close_booking(
	State(panel): State<Panel>,
	caller: Caller,
	Ids(LeadPath { brand, lead }): Ids<LeadPath>,
	IdempotencyKey(key): IdempotencyKey,
	Body(b): Body<CloseBody>,
) -> ApiResult<Upserted<Journaled>> {
	let done = panel
		.close_booking_once(Actor(caller.user_id), &brand, &lead, &b.status, Timestamp::now(), key.as_deref())
		.await?;
	Ok(recorded(done, |e| Journaled { event_id: e.raw() }))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UnmatchedQuery {
	brand: Option<BrandId>,
	/// 100 when absent.
	limit: Option<u32>,
}

/// A provider's booking as the front end reads it.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ProviderBooking")]
struct ProviderBookingDto {
	id: Uuid,
	brand: BrandId,
	provider: Provider,
	external_ref: String,
	/// booked | canceled
	status: &'static str,
	start_at: Timestamp,
	end_at: Option<Timestamp>,
	booked_at: Option<Timestamp>,
	last_event_at: Timestamp,
	/// The lead it joined; null while unmatched.
	lead_id: Option<LeadId>,
	/// How it joined the lead.
	r#match: Option<BookingMatch>,
	/// The attendee, for a caller who sees PII.
	#[serde(skip_serializing_if = "Option::is_none")]
	contact: Option<Value>,
}

impl From<BookingView> for ProviderBookingDto {
	fn from(v: BookingView) -> Self {
		let r = v.row;
		let (lead_id, r#match) = r.lead.unzip();
		Self {
			id: r.id,
			brand: r.brand_id,
			provider: r.provider,
			external_ref: r.external_ref,
			status: if r.booked { "booked" } else { "canceled" },
			start_at: r.start_at,
			end_at: r.end_at,
			booked_at: r.booked_at,
			last_event_at: r.last_event_at,
			lead_id,
			r#match,
			contact: v.contact,
		}
	}
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ProviderBookings")]
struct BookingsDto {
	bookings: Vec<ProviderBookingDto>,
}

/// The providers' bookings no lead was found for, the next slot first; at most 200.
async fn unmatched_bookings(State(panel): State<Panel>, caller: Caller, Params(q): Params<UnmatchedQuery>) -> ApiResult<Json<BookingsDto>> {
	let rows = panel.unmatched_bookings(q.brand.as_ref(), crate::api::pii(&caller), q.limit.unwrap_or(100)).await?;
	Ok(Json(BookingsDto {
		bookings: rows.into_iter().map(ProviderBookingDto::from).collect(),
	}))
}

/// `{id}` of a provider's booking.
#[derive(Deserialize, JsonSchema)]
struct BookingPath {
	id: Uuid,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "BookingAttach")]
struct AttachBody {
	lead: LeadId,
}

/// Joins a provider's booking to a lead of its brand by hand (`booking.attached`).
///
/// 404 for a booking not known, or no such lead of its brand; 409 when it was joined by hand
/// already.
async fn attach_booking(State(panel): State<Panel>, caller: Caller, Ids(BookingPath { id }): Ids<BookingPath>, Body(b): Body<AttachBody>) -> ApiResult<Created<Journaled>> {
	let event = panel.attach_booking(Actor(caller.user_id), id, &b.lead, Timestamp::now()).await?;
	Ok(Created(Journaled { event_id: event.raw() }))
}
