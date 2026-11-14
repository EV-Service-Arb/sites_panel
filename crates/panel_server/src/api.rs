//! The operator API, `/api/v1`, for the sites' front end through the conductor. JSON in and out;
//! every request has passed the assertion gate (`gate::user`), so a [`Caller`] is in its
//! extensions. What an action means is the engine's (`panel::operator`); this only translates.
//!
//! Timestamps are RFC 3339 strings, money is minor units of its currency.

use std::{borrow::Cow, collections::BTreeMap};

use axum::{
	Extension, Json,
	body::Bytes,
	extract::{FromRequest, FromRequestParts, OptionalFromRequest, Path, Query, Request, State},
	http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts},
	response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ev_lib::api::{
	Created, GenContext, JsonSchema, OperationInput, OperationOutput, Upserted,
	openapi::{HeaderStyle, Operation, Parameter, ParameterData, ParameterSchemaOrContent, ReferenceOr, Response as Answer, SchemaObject, StatusCode as Status},
	schemars::{Schema, SchemaGenerator, json_schema},
};
use jiff::{SignedDuration, Timestamp, civil::Date, tz::TimeZone};
use panel::{
	Panel,
	operator::{ActionError, Actor, CallOutcome, Done, EventView, FunnelBy, FunnelSlice, LeadQuery, LeadView, NewLead, Payment, Pii, StageMove, SuspectFilter},
};
use panel_core::{
	Invalid,
	booking::{BookingMatch, BookingStatus, DayPart, Provider},
	fact::{LeadChannel, LeadFlow, MessageRef, Messenger},
	funnel::{MIN_SAMPLE, Share},
	ids::{BrandId, JobId, LeadId, LocationId},
	lead::Stage,
};
use sa_auth::{Leads, Pii as SeesPii, SourceKind};
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};
use ts_rs::TS;
use uuid::Uuid;

use crate::{
	gate::Caller,
	http::{ApiRoute, Gate, Section, api_routes},
};

/// The longest span the funnel sums over.
const MAX_FUNNEL_DAYS: i32 = 366;

/// The largest amount, in minor units, a quote or a payment may name: €100M, far past any
/// real job, well short of anything that overflows a sum.
const MAX_MINOR: i64 = 10_000_000_000;

/// The currencies a quote or a payment may be in. Checked here, not in the registry: a rebuild
/// must not re-judge what was journaled under an older list.
pub const CURRENCIES: [&str; 4] = ["EUR", "USD", "GBP", "AUD"];

/// The header a client sets to make a write safe to retry.
pub const IDEMPOTENCY_KEY: &str = "idempotency-key";

fn amount(what: &str, v: i64) -> ApiResult<i64> {
	if v.unsigned_abs() > MAX_MINOR.unsigned_abs() {
		return Err(ApiError::BadRequest(format!("{what} is more than {MAX_MINOR} minor units")));
	}
	Ok(v)
}

fn currency(v: &str) -> ApiResult<String> {
	if !CURRENCIES.contains(&v) {
		return Err(ApiError::BadRequest(format!("currency is one of {}", CURRENCIES.join(", "))));
	}
	Ok(v.to_owned())
}

pub(crate) fn routes() -> Vec<ApiRoute> {
	use axum::http::Method as M;
	let work = Gate::section(Section::Work);
	let edit = work.may(Leads::Edit);
	vec![
		ApiRoute::new(M::GET, "/me", Gate::USER, me),
		ApiRoute::new(M::GET, "/openapi.json", Gate::USER.or_conductor(), openapi),
		ApiRoute::new(M::GET, "/leads", work, list_leads),
		ApiRoute::new(M::POST, "/leads", edit, create_lead),
		ApiRoute::new(M::GET, "/leads/counts", work, lead_counts),
		ApiRoute::new(M::GET, "/leads/{brand}/{lead}", work, get_lead),
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/stage", edit, move_lead),
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/messaged", edit, mark_messaged),
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/no_answer", edit, no_answer),
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/calls/attempt", edit, attempt_call),
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/calls/{attempt}/outcome", edit, call_outcome),
		ApiRoute::new(M::POST, "/leads/{brand}/{lead}/payments", edit, record_payment),
		ApiRoute::new(M::GET, "/funnel", Gate::section(Section::Analysis), funnel),
		ApiRoute::new(M::GET, "/places", work, list_places),
	]
}

/// Why a request failed, as the front end is told.
#[derive(Debug)]
pub enum ApiError {
	BadRequest(String),
	Forbidden,
	NotFound,
	Conflict(String),
	/// A service the request needs is not configured or not answering; the message is a
	/// machine reason, e.g. `invoices_unavailable`.
	Unavailable(String),
	Internal(eyre::Report),
}

/// What every failed operation answers, its status saying which failure.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Error")]
pub struct ErrorDto {
	pub error: String,
}

impl IntoResponse for ApiError {
	fn into_response(self) -> Response {
		let (status, msg) = match self {
			Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
			Self::Forbidden => (StatusCode::FORBIDDEN, "your permissions do not allow this".to_owned()),
			Self::NotFound => (StatusCode::NOT_FOUND, "not found".to_owned()),
			Self::Conflict(m) => (StatusCode::CONFLICT, m),
			Self::Unavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
			Self::Internal(e) => {
				crate::report(&e, "operator API");
				(StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_owned())
			}
		};
		(status, Json(ErrorDto { error: msg })).into_response()
	}
}

impl OperationOutput for ApiError {
	type Inner = ErrorDto;

	fn operation_response(ctx: &mut GenContext, op: &mut Operation) -> Option<Answer> {
		Json::<ErrorDto>::operation_response(ctx, op)
	}

	fn inferred_responses(ctx: &mut GenContext, op: &mut Operation) -> Vec<(Option<Status>, Answer)> {
		Self::operation_response(ctx, op).map(|r| (None, r)).into_iter().collect()
	}
}

impl From<eyre::Report> for ApiError {
	fn from(e: eyre::Report) -> Self {
		Self::Internal(e)
	}
}

impl From<Invalid> for ApiError {
	fn from(e: Invalid) -> Self {
		Self::BadRequest(e.0)
	}
}

impl From<ActionError> for ApiError {
	fn from(e: ActionError) -> Self {
		match e {
			ActionError::NotFound => Self::NotFound,
			ActionError::Conflict(m) => Self::Conflict(m.to_owned()),
			ActionError::Invalid(e) => Self::BadRequest(e.0),
			ActionError::Internal(e) => Self::Internal(e),
		}
	}
}

pub(crate) type ApiResult<T> = Result<T, ApiError>;

/// A JSON body, refused in the shape every failure has. Optional (`Option<Body<T>>`), an empty
/// body or `null` is none, and the content type is not asked for.
pub(crate) struct Body<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for Body<T> {
	type Rejection = ApiError;

	async fn from_request(req: Request, state: &S) -> Result<Self, ApiError> {
		<Json<T> as FromRequest<S>>::from_request(req, state)
			.await
			.map(|Json(v)| Self(v))
			.map_err(|e| ApiError::BadRequest(e.body_text()))
	}
}

impl<T: DeserializeOwned, S: Send + Sync> OptionalFromRequest<S> for Body<T> {
	type Rejection = ApiError;

	async fn from_request(req: Request, state: &S) -> Result<Option<Self>, ApiError> {
		let raw = <Bytes as FromRequest<S>>::from_request(req, state).await.map_err(|e| ApiError::BadRequest(e.body_text()))?;
		if raw.iter().all(u8::is_ascii_whitespace) {
			return Ok(None);
		}
		let v: Option<T> = serde_json::from_slice(&raw).map_err(|e| ApiError::BadRequest(format!("the body: {e}")))?;
		Ok(v.map(Self))
	}
}

impl<T: JsonSchema> OperationInput for Body<T> {
	fn operation_input(ctx: &mut GenContext, op: &mut Operation) {
		Json::<T>::operation_input(ctx, op);
	}
}

/// A query string, refused in the shape every failure has.
pub(crate) struct Params<T>(pub T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequestParts<S> for Params<T> {
	type Rejection = ApiError;

	async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
		Query::<T>::from_request_parts(parts, state)
			.await
			.map(|Query(v)| Self(v))
			.map_err(|e| ApiError::BadRequest(e.body_text()))
	}
}

impl<T: JsonSchema> OperationInput for Params<T> {
	fn operation_input(ctx: &mut GenContext, op: &mut Operation) {
		Query::<T>::operation_input(ctx, op);
	}
}

/// The ids in a path, by their names in it; one that is not an id at all names nothing: a 404.
pub(crate) struct Ids<T>(pub T);

impl<T: DeserializeOwned + Send, S: Send + Sync> FromRequestParts<S> for Ids<T> {
	type Rejection = ApiError;

	async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
		Path::<T>::from_request_parts(parts, state).await.map(|Path(v)| Self(v)).map_err(|_| ApiError::NotFound)
	}
}

impl<T: JsonSchema> OperationInput for Ids<T> {
	fn operation_input(ctx: &mut GenContext, op: &mut Operation) {
		Path::<T>::operation_input(ctx, op);
	}
}

/// With `#[serde(default)]`: a field that is there, `null` included, apart from one left out.
pub(crate) fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<T>, D::Error> {
	T::deserialize(d).map(Some)
}

/// `{brand}/{lead}`.
#[derive(Deserialize, JsonSchema)]
pub(crate) struct LeadPath {
	pub brand: BrandId,
	pub lead: LeadId,
}

/// The client's `Idempotency-Key`, when it sent one: a write safe to retry.
pub(crate) struct IdempotencyKey(pub Option<String>);

/// The same, required: where a retry without one would act twice.
pub(crate) struct RequiredIdempotencyKey(pub String);

fn idempotency_key(parts: &Parts) -> ApiResult<Option<String>> {
	let Some(v) = parts.headers.get(IDEMPOTENCY_KEY) else { return Ok(None) };
	let v = v.to_str().ok().filter(|v| (1..=128).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_graphic()));
	v.map(|v| Some(v.to_owned()))
		.ok_or_else(|| ApiError::BadRequest("Idempotency-Key is 1–128 visible ASCII characters".into()))
}

impl<S: Send + Sync> FromRequestParts<S> for IdempotencyKey {
	type Rejection = ApiError;

	async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
		idempotency_key(parts).map(Self)
	}
}

impl<S: Send + Sync> FromRequestParts<S> for RequiredIdempotencyKey {
	type Rejection = ApiError;

	async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
		idempotency_key(parts)?.map(Self).ok_or_else(|| ApiError::BadRequest("Idempotency-Key is required".into()))
	}
}

fn idempotency_header(ctx: &mut GenContext, op: &mut Operation, required: bool) {
	op.parameters.push(ReferenceOr::Item(Parameter::Header {
		parameter_data: ParameterData {
			name: "Idempotency-Key".into(),
			description: Some(
				"1–128 visible ASCII characters. The action's event id derives from (user, action, subject, key): a retry finds the first attempt and is answered 200 with \
				 its body, journaling nothing."
					.into(),
			),
			required,
			format: ParameterSchemaOrContent::Schema(SchemaObject {
				json_schema: ctx.schema.subschema_for::<String>(),
				example: None,
				external_docs: None,
			}),
			extensions: Default::default(),
			deprecated: None,
			example: None,
			examples: Default::default(),
			explode: None,
		},
		style: HeaderStyle::Simple,
	}));
}

impl OperationInput for IdempotencyKey {
	fn operation_input(ctx: &mut GenContext, op: &mut Operation) {
		idempotency_header(ctx, op, false);
	}
}

impl OperationInput for RequiredIdempotencyKey {
	fn operation_input(ctx: &mut GenContext, op: &mut Operation) {
		idempotency_header(ctx, op, true);
	}
}

/// `201` for what was recorded now, `200` for a retry answered with what the first recorded.
pub(crate) fn recorded<T, U>(done: Done<T>, answer: impl FnOnce(T) -> U) -> Upserted<U> {
	match done.replayed {
		true => Upserted::Existed(answer(done.value)),
		false => Upserted::Created(answer(done.value)),
	}
}

/// The event a write journaled.
#[derive(JsonSchema, Serialize)]
pub(crate) struct Journaled {
	pub event_id: Uuid,
}

/// The point where the caller's permissions decide whether PII is shown.
pub(crate) fn pii(caller: &Caller) -> Pii {
	if caller.permissions.may(SeesPii::See) { Pii::Reveal } else { Pii::Withhold }
}

/// `from` and `to` of a report, UTC days, both included: by default the last 30 days, at
/// most [`MAX_FUNNEL_DAYS`].
fn window(from: Option<Date>, to: Option<Date>) -> ApiResult<(Date, Date)> {
	let to = to.unwrap_or_else(|| Timestamp::now().to_zoned(TimeZone::UTC).date());
	let from = match from {
		Some(d) => d,
		None => to.checked_sub(SignedDuration::from_hours(24 * 29)).map_err(|e| ApiError::Internal(e.into()))?,
	};
	if from > to {
		return Err(ApiError::BadRequest("from is after to".into()));
	}
	if (to - from).get_days() >= MAX_FUNNEL_DAYS {
		return Err(ApiError::BadRequest(format!("at most {MAX_FUNNEL_DAYS} days at once")));
	}
	Ok((from, to))
}

/// The first instant of a UTC day.
fn day_start(d: Date) -> ApiResult<Timestamp> {
	Ok(d.to_zoned(TimeZone::UTC).map_err(|e| ApiError::Internal(e.into()))?.timestamp())
}

// ── /me ─────────────────────────────────────────────────────────────────────────────────

/// `GET /me`.
#[derive(JsonSchema, Serialize, TS)]
pub struct Me {
	/// The concierge user id: what a user's own marks are kept under.
	sub: Uuid,
	/// The caller's `sa:sites:*` permissions.
	#[ts(type = "Array<Permission>")]
	permissions: Vec<String>,
}

/// The caller, as the conductor vouched for them: their user id and `sa:sites:*` permissions.
async fn me(Extension(caller): Extension<Caller>) -> Json<Me> {
	Json(Me {
		sub: caller.user_id,
		permissions: caller.permissions.iter().map(str::to_owned).collect(),
	})
}

/// The `/api/v1` document as built at boot, and its ETag.
#[derive(Clone)]
pub(crate) struct Catalog {
	json: Bytes,
	etag: HeaderValue,
}

impl Catalog {
	pub(crate) fn new() -> Self {
		let doc = ev_lib::api::document("Service-Arb sites", env!("CARGO_PKG_VERSION"), "/api/v1", &api_routes(), ev_lib::api_docs!());
		let json = serde_json::to_vec(&doc).expect("a document serializes");
		let digest: String = Sha256::digest(&json).iter().take(8).map(|b| format!("{b:02x}")).collect();
		Self {
			json: json.into(),
			etag: HeaderValue::from_str(&format!("\"{digest}\"")).expect("hex in quotes"),
		}
	}
}

/// The document, or the word that the caller's copy is it.
pub(crate) enum Served {
	Document(Catalog),
	Unchanged(Catalog),
}

/// An OpenAPI 3.1 document.
struct OpenApiDocument;

impl JsonSchema for OpenApiDocument {
	fn schema_name() -> Cow<'static, str> {
		"OpenApi".into()
	}

	fn json_schema(_: &mut SchemaGenerator) -> Schema {
		json_schema!({ "type": "object", "description": "An OpenAPI 3.1 document." })
	}
}

impl IntoResponse for Served {
	fn into_response(self) -> Response {
		match self {
			Self::Document(c) => ([(header::CONTENT_TYPE, HeaderValue::from_static("application/json")), (header::ETAG, c.etag)], c.json).into_response(),
			Self::Unchanged(c) => (StatusCode::NOT_MODIFIED, [(header::ETAG, c.etag)]).into_response(),
		}
	}
}

impl OperationOutput for Served {
	type Inner = ();

	fn inferred_responses(ctx: &mut GenContext, op: &mut Operation) -> Vec<(Option<Status>, Answer)> {
		let unchanged = Answer {
			description: "The copy `If-None-Match` named is the document.".into(),
			..Answer::default()
		};
		Json::<OpenApiDocument>::operation_response(ctx, op)
			.map(|r| (Some(Status::Code(200)), r))
			.into_iter()
			.chain([(Some(Status::Code(304)), unchanged)])
			.collect()
	}
}

/// The OpenAPI document of every `/api/v1` operation, each with the permission that opens it
/// (`x-permission`).
///
/// The whole of it, whoever asks; built at boot. It carries an ETag, and an `If-None-Match`
/// naming it is answered 304. The conductor pulls it under its own assertion.
async fn openapi(Extension(catalog): Extension<Catalog>, headers: HeaderMap) -> Served {
	match headers.get(header::IF_NONE_MATCH) == Some(&catalog.etag) {
		true => Served::Unchanged(catalog),
		false => Served::Document(catalog),
	}
}

// ── leads ───────────────────────────────────────────────────────────────────────────────

/// A lead: the projection's row, its wait for a first contact, and what the customer left.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Lead")]
struct LeadDto {
	brand: String,
	lead_id: String,
	location: Option<String>,
	job_id: Option<String>,
	stage: Stage,
	/// form | phone_inbound | callback | whatsapp | telegram; null for a lead seen before its
	/// creation.
	channel: Option<String>,
	/// The ref the customer carries into a messenger (`AQ-7K3F`), when the landing made one.
	message_ref: Option<String>,
	/// When the customer first wrote on a messenger, and on which; null while they have not.
	messaged_at: Option<Timestamp>,
	messaged_channel: Option<Messenger>,
	/// `rate_limited` | `too_fast` when the landing's antispam doubted it; null otherwise.
	suspect: Option<String>,
	/// Typed in by hand (an event of kind `panel`).
	manual: bool,
	/// When each stage was first reached.
	created_at: Option<Timestamp>,
	contacted_at: Option<Timestamp>,
	quoted_at: Option<Timestamp>,
	won_at: Option<Timestamp>,
	completed_at: Option<Timestamp>,
	paid_at: Option<Timestamp>,
	lost_at: Option<Timestamp>,
	lost_reason: Option<String>,
	/// `quote` | `estimate` | `fixed`: the landing's flow; null when it said none.
	flow: Option<String>,
	/// The price an estimate or a fixed price showed, integer cents EUR TTC.
	quoted_cents: Option<i64>,
	/// `YYYY-MM-DD`: when the pricing model behind `quoted_cents` took effect.
	pricing_valid_from: Option<String>,
	/// An estimate's inputs, input id → value id.
	estimate_inputs: Option<Value>,
	/// Its booking; `status: none` when it has none.
	booking: LeadBookingDto,
	last_event_at: Timestamp,
	/// Set while it waits for its first contact.
	sla: Option<SlaDto>,
	/// What the customer left (name, phone, need, …), for a caller holding
	/// `sa:sites:work:pii:see`; absent otherwise or when there is none.
	#[serde(skip_serializing_if = "Option::is_none")]
	pii: Option<Value>,
}

/// A lead's booking, folded from its facts.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "LeadBooking")]
struct LeadBookingDto {
	status: BookingStatus,
	provider: Option<Provider>,
	start_at: Option<Timestamp>,
	end_at: Option<Timestamp>,
	/// The provider's booking, when the slot is one.
	external_ref: Option<String>,
	/// How the slot joined the lead.
	r#match: Option<BookingMatch>,
	/// The visitor's wish.
	preferred_date: Option<Date>,
	preferred_part: Option<DayPart>,
}

/// A lead's wait for its first contact: overdue after 30 minutes.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Sla")]
struct SlaDto {
	waiting_since: Timestamp,
	waiting_seconds: i64,
	overdue: bool,
}

fn lead_dto(v: LeadView, now: Timestamp) -> LeadDto {
	let r = v.row;
	LeadDto {
		sla: v.waiting.map(|w| SlaDto {
			waiting_since: w.since,
			waiting_seconds: now.duration_since(w.since).as_secs(),
			overdue: w.overdue,
		}),
		pii: v.pii,
		brand: r.brand_id,
		lead_id: r.lead_id,
		location: r.location_id,
		job_id: r.job_id,
		stage: r.stage,
		channel: r.channel,
		message_ref: r.message_ref,
		messaged_at: r.messaged.map(|(at, _)| at),
		messaged_channel: r.messaged.map(|(_, m)| m),
		suspect: r.suspect,
		manual: r.manual,
		created_at: r.created_at,
		contacted_at: r.contacted_at,
		quoted_at: r.quoted_at,
		won_at: r.won_at,
		completed_at: r.completed_at,
		paid_at: r.paid_at,
		lost_at: r.lost_at,
		lost_reason: r.lost_reason,
		flow: r.flow,
		quoted_cents: r.quoted_cents,
		pricing_valid_from: r.pricing_valid_from,
		estimate_inputs: r.estimate_inputs,
		booking: LeadBookingDto {
			status: r.booking.status,
			provider: r.booking.provider,
			start_at: r.booking.start_at,
			end_at: r.booking.end_at,
			external_ref: r.booking.external_ref,
			r#match: r.booking.matched,
			preferred_date: r.booking.preferred_date,
			preferred_part: r.booking.preferred_part,
		},
		last_event_at: r.last_event_at,
	}
}

/// Which of the suspect leads to list.
#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "SuspectFilter")]
enum Suspect {
	/// Only those the landing's antispam doubted.
	Only,
	/// None of them.
	Exclude,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LeadsQuery {
	stage: Option<Stage>,
	brand: Option<BrandId>,
	/// The conductor's selection of GMBs, ids split by commas (`3,7`): only the leads of places
	/// linked to one of them. Empty selects none; absent narrows nothing.
	gmb: Option<String>,
	/// Only the leads overdue for their first contact.
	overdue: Option<bool>,
	/// Created on or after this UTC day.
	created_from: Option<Date>,
	/// Created on or before this UTC day.
	created_to: Option<Date>,
	/// Absent: every lead.
	suspect: Option<Suspect>,
	/// Absent: every lead; a lead that said no flow is under none of the three.
	flow: Option<LeadFlow>,
	booking: Option<BookingStatus>,
	channel: Option<LeadChannel>,
	/// A messenger ref as the customer quotes it or an operator pastes it (`Réf. aq 7k3f`:
	/// label, case, spaces; O→0, I/L→1 in the code): the leads carrying it.
	message_ref: Option<String>,
	/// `next_cursor` of the page before.
	cursor: Option<String>,
	/// At most 200; 50 when absent.
	limit: Option<u32>,
}

/// `gmb` of a read: the conductor's selection, `1,7,12`; empty selects none, absent narrows nothing.
fn gmbs(raw: Option<&str>) -> ApiResult<Option<Vec<i64>>> {
	let Some(raw) = raw else { return Ok(None) };
	if raw.is_empty() {
		return Ok(Some(Vec::new()));
	}
	raw.split(',')
		.map(|id| id.parse().map_err(|_| ApiError::BadRequest(format!("gmb is GMB ids split by commas, not {raw}"))))
		.collect::<ApiResult<_>>()
		.map(Some)
}

fn cursor_encode((at, brand, lead): &(Timestamp, String, String)) -> String {
	URL_SAFE_NO_PAD.encode(format!("{at}|{brand}|{lead}"))
}

fn cursor_decode(raw: &str) -> ApiResult<(Timestamp, String, String)> {
	let bad = || ApiError::BadRequest("cursor is not one this API gave".into());
	let text = String::from_utf8(URL_SAFE_NO_PAD.decode(raw).map_err(|_| bad())?).map_err(|_| bad())?;
	let mut parts = text.splitn(3, '|');
	let (Some(at), Some(brand), Some(lead)) = (parts.next(), parts.next(), parts.next()) else {
		return Err(bad());
	};
	Ok((at.parse().map_err(|_| bad())?, brand.to_owned(), lead.to_owned()))
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "LeadPage")]
struct LeadPageDto {
	leads: Vec<LeadDto>,
	/// Asks for the next page; null on the last.
	next_cursor: Option<String>,
}

/// The leads, newest created first, a page at a time.
///
/// Each filter narrows; `created_*` are UTC days, both included.
async fn list_leads(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Params(q): Params<LeadsQuery>) -> ApiResult<Json<LeadPageDto>> {
	if let (Some(f), Some(t)) = (q.created_from, q.created_to)
		&& f > t
	{
		return Err(ApiError::BadRequest("created_from is after created_to".into()));
	}
	let next_day = |d: Date| d.tomorrow().map_err(|e| ApiError::Internal(e.into())).and_then(day_start);
	let query = LeadQuery {
		stage: q.stage,
		brand: q.brand,
		gmbs: gmbs(q.gmb.as_deref())?,
		overdue: q.overdue.unwrap_or(false),
		created_from: q.created_from.map(day_start).transpose()?,
		created_before: q.created_to.map(next_day).transpose()?,
		suspect: match q.suspect {
			None => SuspectFilter::All,
			Some(Suspect::Only) => SuspectFilter::Only,
			Some(Suspect::Exclude) => SuspectFilter::Exclude,
		},
		flow: q.flow,
		booking: q.booking,
		channel: q.channel,
		message_ref: q
			.message_ref
			.as_deref()
			.map(|r| MessageRef::from_typed(r).map_err(|_| ApiError::BadRequest("message_ref is not like AQ-7K3F".into())))
			.transpose()?,
		after: q.cursor.as_deref().map(cursor_decode).transpose()?,
		limit: q.limit.unwrap_or(50),
	};
	let now = Timestamp::now();
	let page = panel.leads(&query, pii(&caller), now).await?;
	Ok(Json(LeadPageDto {
		next_cursor: page.next.as_ref().map(cursor_encode),
		leads: page.leads.into_iter().map(|v| lead_dto(v, now)).collect(),
	}))
}

/// One event of a lead, as its card shows it.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "LeadEvent")]
struct EventDto {
	id: Uuid,
	r#type: String,
	type_version: i32,
	occurred_at: Timestamp,
	received_at: Timestamp,
	source_kind: String,
	source_id: String,
	/// Typed in by hand (kind `panel`).
	manual: bool,
	job_id: Option<String>,
	/// accepted | unregistered | …: how the registry judged it.
	status: String,
	status_reason: Option<String>,
	/// The event's properties, as its type has them.
	properties: Value,
	/// What the customer left, for a caller who sees it.
	#[serde(skip_serializing_if = "Option::is_none")]
	pii: Option<Value>,
}

fn event_dto(e: EventView) -> EventDto {
	let r = e.row;
	EventDto {
		manual: r.source_kind == SourceKind::Panel.as_str(),
		id: r.id,
		r#type: r.r#type,
		type_version: r.type_version,
		occurred_at: r.occurred_at,
		received_at: r.received_at,
		source_kind: r.source_kind,
		source_id: r.source_id,
		job_id: r.job_id,
		status: r.status,
		status_reason: r.status_reason,
		properties: r.properties,
		pii: e.pii,
	}
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "LeadCard")]
struct LeadCardDto {
	lead: LeadDto,
	events: Vec<EventDto>,
}

/// A lead's card: the lead and every event about it.
async fn get_lead(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Ids(LeadPath { brand, lead }): Ids<LeadPath>) -> ApiResult<Json<LeadCardDto>> {
	let now = Timestamp::now();
	let (view, events) = panel.lead_card(&brand, &lead, pii(&caller), now).await?.ok_or(ApiError::NotFound)?;
	Ok(Json(LeadCardDto {
		lead: lead_dto(view, now),
		events: events.into_iter().map(event_dto).collect(),
	}))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "LeadCreation")]
struct CreateLead {
	brand: BrandId,
	location: LocationId,
	need: String,
	phone: Option<String>,
	/// phone_inbound (the default) | whatsapp | telegram: a customer who called, or wrote on a
	/// messenger without the landing.
	channel: Option<LeadChannel>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "NewLead")]
struct NewLeadDto {
	brand: BrandId,
	/// `p-<uuidv7>`.
	lead_id: LeadId,
	event_id: Uuid,
}

/// Creates a lead by hand (`lead.created`, kind `panel`).
async fn create_lead(State(panel): State<Panel>, Extension(caller): Extension<Caller>, IdempotencyKey(key): IdempotencyKey, Body(b): Body<CreateLead>) -> ApiResult<Upserted<NewLeadDto>> {
	let new = NewLead {
		brand: b.brand,
		location: b.location,
		need: b.need,
		phone: b.phone,
		channel: match b.channel {
			None => LeadChannel::PhoneInbound,
			Some(c) if LeadChannel::MANUAL.contains(&c) => c,
			Some(_) => return Err(ApiError::BadRequest("channel is one of phone_inbound, whatsapp, telegram".into())),
		},
	};
	let brand = new.brand.clone();
	let done = panel.create_lead_once(Actor(caller.user_id), new, Timestamp::now(), key.as_deref()).await?;
	Ok(recorded(done, |(lead_id, event)| NewLeadDto {
		brand,
		lead_id,
		event_id: event.raw(),
	}))
}

/// Where a lead moves, and what the move says.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "StageMove")]
enum StageBody {
	Contacted {
		channel: Option<String>,
	},
	Quoted {
		/// Minor units, |amount| ≤ 10^10.
		amount: Option<i64>,
		/// EUR | USD | GBP | AUD.
		currency: Option<String>,
	},
	Won {
		job_id: Option<String>,
	},
	Lost {
		reason: String,
		note: Option<String>,
	},
	Completed,
}

/// Moves a lead to a stage (`lead.contacted`, `lead.quoted`, `job.won`, `lead.lost`,
/// `job.completed`).
///
/// Stages move forward; `lost` from anywhere, and later progress reopens a lost lead. 409 when
/// the move is not from where the lead stands.
async fn move_lead(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Ids(LeadPath { brand, lead }): Ids<LeadPath>,
	IdempotencyKey(key): IdempotencyKey,
	Body(b): Body<StageBody>,
) -> ApiResult<Upserted<Journaled>> {
	let to = match b {
		StageBody::Contacted { channel } => StageMove::Contacted { channel },
		StageBody::Quoted { amount: a, currency: c } => StageMove::Quoted {
			amount: a.map(|a| amount("amount", a)).transpose()?,
			currency: c.as_deref().map(currency).transpose()?,
		},
		StageBody::Won { job_id } => StageMove::Won {
			job_id: job_id.as_deref().map(JobId::parse).transpose()?,
		},
		StageBody::Lost { reason, note } => StageMove::Lost { reason, note },
		StageBody::Completed => StageMove::Completed,
	};
	let done = panel.move_lead_once(Actor(caller.user_id), &brand, &lead, to, Timestamp::now(), key.as_deref()).await?;
	Ok(recorded(done, |e| Journaled { event_id: e.raw() }))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "Messaged")]
struct MessagedBody {
	channel: Messenger,
}

/// The customer wrote on a messenger, as the operator saw it (`lead.messaged`).
async fn mark_messaged(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Ids(LeadPath { brand, lead }): Ids<LeadPath>,
	IdempotencyKey(key): IdempotencyKey,
	Body(b): Body<MessagedBody>,
) -> ApiResult<Upserted<Journaled>> {
	let done = panel
		.mark_messaged_once(Actor(caller.user_id), &brand, &lead, b.channel, Timestamp::now(), key.as_deref())
		.await?;
	Ok(recorded(done, |e| Journaled { event_id: e.raw() }))
}

/// A call nobody answered (`call.attempted` + `call.logged{no_answer}`).
///
/// A note's button: its Idempotency-Key is required, so a press repeated records nothing twice.
async fn no_answer(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Ids(LeadPath { brand, lead }): Ids<LeadPath>,
	RequiredIdempotencyKey(key): RequiredIdempotencyKey,
) -> ApiResult<Upserted<Journaled>> {
	let done = panel.no_answer_once(Actor(caller.user_id), &brand, &lead, Timestamp::now(), &key).await?;
	Ok(recorded(done, |e| Journaled { event_id: e.raw() }))
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "CallAttempt")]
struct AttemptDto {
	attempt_id: Uuid,
}

/// Starts a call to the customer (`call.attempted`); its outcome is logged against the attempt.
async fn attempt_call(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Ids(LeadPath { brand, lead }): Ids<LeadPath>) -> ApiResult<Created<AttemptDto>> {
	let attempt = panel.attempt_call(Actor(caller.user_id), &brand, &lead, Timestamp::now()).await?;
	Ok(Created(AttemptDto { attempt_id: attempt.raw() }))
}

/// `{brand}/{lead}/calls/{attempt}`.
#[derive(Deserialize, JsonSchema)]
struct AttemptPath {
	brand: BrandId,
	lead: LeadId,
	attempt: Uuid,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "CallOutcome")]
struct OutcomeBody {
	/// answered | no_answer | wrong_number | later
	outcome: String,
}

/// How an attempted call went (`call.logged`).
async fn call_outcome(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Ids(AttemptPath { brand, lead, attempt }): Ids<AttemptPath>,
	Body(b): Body<OutcomeBody>,
) -> ApiResult<Created<Journaled>> {
	let call = CallOutcome { attempt, outcome: b.outcome };
	let event = panel.log_call(Actor(caller.user_id), &brand, &lead, call, Timestamp::now()).await?;
	Ok(Created(Journaled { event_id: event.raw() }))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "Payment")]
struct PaymentBody {
	/// Minor units, |amount| ≤ 10^10.
	billed: i64,
	/// Minor units, |amount| ≤ 10^10.
	commission: i64,
	/// EUR | USD | GBP | AUD.
	currency: String,
}

/// Records a payment for a lead's job, entered by hand (`payment.received`).
async fn record_payment(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Ids(LeadPath { brand, lead }): Ids<LeadPath>,
	IdempotencyKey(key): IdempotencyKey,
	Body(b): Body<PaymentBody>,
) -> ApiResult<Upserted<Journaled>> {
	let p = Payment {
		billed: amount("billed", b.billed)?,
		commission: amount("commission", b.commission)?,
		currency: currency(&b.currency)?,
	};
	let done = panel.record_payment_once(Actor(caller.user_id), &brand, &lead, p, Timestamp::now(), key.as_deref()).await?;
	Ok(recorded(done, |e| Journaled { event_id: e.raw() }))
}

// ── funnel ──────────────────────────────────────────────────────────────────────────────

/// How the funnel is cut.
#[derive(Clone, Copy, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "FunnelBy")]
enum By {
	/// One slice per brand's location; `location` null for the leads naming none.
	Location,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FunnelQuery {
	/// A UTC day; 29 days before `to` when absent.
	from: Option<Date>,
	/// A UTC day, included; today when absent. At most 366 days from `from`.
	to: Option<Date>,
	brand: Option<BrandId>,
	/// Absent: the whole funnel.
	by: Option<By>,
	/// The conductor's selection of GMBs, as `GET /leads` takes it.
	gmb: Option<String>,
}

/// A share as the front end may show it: `percent` only when `of` is large enough, and
/// `small_sample` saying so otherwise, so "12 of 17" is all that can be drawn.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Share")]
struct ShareDto {
	n: u64,
	of: u64,
	percent: Option<u64>,
	small_sample: bool,
}

impl From<Share> for ShareDto {
	fn from(s: Share) -> Self {
		Self {
			n: s.n,
			of: s.of,
			percent: s.percent,
			small_sample: s.small_sample(),
		}
	}
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "FunnelStep")]
struct StepDto {
	stage: Stage,
	reached: u64,
	of_previous: Option<ShareDto>,
	of_leads: ShareDto,
}

/// What a slice's leads were paid in one currency, in its minor units; never converted.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Paid")]
struct PaidDto {
	currency: String,
	billed: i64,
	commission: i64,
	count: u64,
}

/// A slice's funnel: its steps, the lost and manual shares, and its payments.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "FunnelSlice")]
struct SliceDto {
	stages: Vec<StepDto>,
	lost: ShareDto,
	manual: ShareDto,
	payments: Vec<PaidDto>,
}

impl From<FunnelSlice> for SliceDto {
	fn from(s: FunnelSlice) -> Self {
		let totals = s.totals;
		Self {
			stages: totals
				.steps()
				.into_iter()
				.map(|s| StepDto {
					stage: s.stage,
					reached: s.reached,
					of_previous: s.of_previous.map(ShareDto::from),
					of_leads: s.of_leads.into(),
				})
				.collect(),
			lost: Share::new(totals.lost, totals.leads).into(),
			manual: Share::new(totals.manual, totals.leads).into(),
			payments: s
				.payments
				.into_iter()
				.map(|p| PaidDto {
					currency: p.currency,
					billed: p.billed,
					commission: p.commission,
					count: p.payments,
				})
				.collect(),
		}
	}
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "LocationFunnel")]
struct LocationSliceDto {
	brand: Option<String>,
	location: Option<String>,
	#[serde(flatten)]
	slice: SliceDto,
}

#[derive(JsonSchema, Serialize)]
#[serde(untagged)]
enum Slices {
	Whole(SliceDto),
	ByLocation { by: By, locations: Vec<LocationSliceDto> },
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Funnel")]
struct FunnelDto {
	from: Date,
	to: Date,
	brand: Option<BrandId>,
	/// Under this many, a share has no percent.
	min_sample: u64,
	/// The same funnel from the visit on in PostHog; null without a project configured.
	posthog_url: Option<String>,
	#[serde(flatten)]
	slices: Slices,
}

/// The funnel of the leads created within a window of UTC days: each stage reached, the lost
/// and manual shares, the payments.
///
/// Payments are of those same leads, whenever paid. An empty window has no location rows.
/// Visits and intents are PostHog's: `posthog_url` opens the funnel there.
async fn funnel(State(panel): State<Panel>, Params(q): Params<FunnelQuery>) -> ApiResult<Json<FunnelDto>> {
	let (from, to) = window(q.from, q.to)?;
	let by = match q.by {
		None => FunnelBy::All,
		Some(By::Location) => FunnelBy::Location,
	};
	let selected = gmbs(q.gmb.as_deref())?;
	let slices = panel.funnel_slices(from, to, q.brand.as_ref(), selected.as_deref(), by).await?;
	let slices = match by {
		FunnelBy::All => Slices::Whole(
			slices
				.into_iter()
				.next()
				.ok_or_else(|| ApiError::Internal(eyre::eyre!("the whole funnel came back empty")))?
				.into(),
		),
		FunnelBy::Location => Slices::ByLocation {
			by: By::Location,
			locations: slices
				.into_iter()
				.map(|s| LocationSliceDto {
					brand: s.brand.clone(),
					location: s.location.clone(),
					slice: s.into(),
				})
				.collect(),
		},
	};
	Ok(Json(FunnelDto {
		from,
		to,
		posthog_url: panel.posthog_funnel_url(q.brand.as_ref(), from, to),
		brand: q.brand,
		min_sample: MIN_SAMPLE,
		slices,
	}))
}

// ── places, counts ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlacesQuery {
	/// The conductor's selection of GMBs, as `GET /leads` takes it.
	gmb: Option<String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "PlaceSummary")]
struct PlaceRowDto {
	brand: String,
	location: String,
	last_lead_at: Option<Timestamp>,
	/// Live settings with a field set at least.
	has_settings: bool,
	withdrawn: bool,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Places")]
struct PlacesDto {
	places: Vec<PlaceRowDto>,
}

/// Every place the panel knows: every location a lead names, and every place registered.
async fn list_places(State(panel): State<Panel>, Params(q): Params<PlacesQuery>) -> ApiResult<Json<PlacesDto>> {
	let places = panel
		.places(gmbs(q.gmb.as_deref())?.as_deref())
		.await?
		.into_iter()
		.map(|p| PlaceRowDto {
			brand: p.brand_id,
			location: p.location_id,
			last_lead_at: p.last_lead_at,
			has_settings: p.has_settings,
			withdrawn: p.withdrawn,
		})
		.collect();
	Ok(Json(PlacesDto { places }))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CountsQuery {
	brand: Option<BrandId>,
	/// The conductor's selection of GMBs, as `GET /leads` takes it.
	gmb: Option<String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "LeadCounts")]
struct CountsDto {
	/// Every stage, 0 included.
	stages: BTreeMap<Stage, u64>,
	overdue: u64,
	total: u64,
}

/// How many leads are at each stage, and how many are overdue for their first contact.
async fn lead_counts(State(panel): State<Panel>, Params(q): Params<CountsQuery>) -> ApiResult<Json<CountsDto>> {
	let counts = panel.lead_counts(q.brand.as_ref(), gmbs(q.gmb.as_deref())?.as_deref(), Timestamp::now()).await?;
	Ok(Json(CountsDto {
		total: counts.stages.iter().map(|(_, n)| n).sum(),
		stages: counts.stages.into_iter().collect(),
		overdue: counts.overdue,
	}))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn cursors_round_trip() {
		let c = ("2026-09-30T10:00:00Z".parse().unwrap(), "aquafix".to_owned(), "L-1".to_owned());
		assert_eq!(cursor_decode(&cursor_encode(&c)).unwrap(), c);
		assert!(cursor_decode("nope!").is_err());
		assert!(cursor_decode(&URL_SAFE_NO_PAD.encode("x|y")).is_err());
	}
}
