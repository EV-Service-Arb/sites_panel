//! The operator API, `/api/v1`, for the sites' front end through the conductor. JSON in and out;
//! every request has passed the assertion gate (`gate::user`), so a [`Caller`] is in its
//! extensions. What an action means is the engine's (`panel::operator`); this only translates.
//!
//! Timestamps are RFC 3339 strings, money is minor units of its currency.

use std::{borrow::Cow, collections::BTreeMap};

use axum::{
	Json,
	extract::{FromRequestParts, State},
	http::{StatusCode, request::Parts},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use ev_lib::api::{
	Body, Created, ErrorCode, GenContext, Ids, JsonSchema, OperationInput, Params, Refusal, Refuses, Upserted,
	openapi::{HeaderStyle, Operation, Parameter, ParameterData, ParameterSchemaOrContent, ReferenceOr, SchemaObject},
	schemars::{Schema, SchemaGenerator, json_schema},
};
use jiff::{SignedDuration, Timestamp, civil::Date, tz::TimeZone};
use panel::{
	Panel,
	operator::{ActionError, Actor, CallOutcome, Conflict, Done, EventView, FunnelBy, FunnelSlice, LeadQuery, LeadView, NewLead, Payment, Pii, StageMove, SuspectFilter},
	store::events::Status as EventStatus,
};
use panel_core::{
	booking::{BookingMatch, BookingStatus, DayPart, Provider},
	fact::{self, ContactChannel, LeadChannel, LeadFlow, LeadSuspect, LostReason, ManualChannel, MessageRef, Messenger},
	funnel::{MIN_SAMPLE, Share},
	ids::{JobId, LeadId, LocationId},
	lead::Stage,
	pricing::InputId,
};
use sa_auth::{Analysis, Catalog, Denied, Gate, Leads, PermissionSet, Pii as SeesPii, Service, SourceKind, User, Work};
use sa_core::{Invalid, ids::BrandId};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

use crate::http::ApiRoute;

/// The longest span the funnel sums over.
const MAX_FUNNEL_DAYS: i32 = 366;

/// The largest amount, in minor units, a quote or a payment may name: €100M, far past any
/// real job, well short of anything that overflows a sum.
const MAX_MINOR: i64 = 10_000_000_000;

/// The currencies a quote or a payment may be in. Checked here, not in the registry: a rebuild
/// must not re-judge what was journaled under an older list.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, Serialize, strum::IntoStaticStr, strum::VariantArray)]
#[serde(rename_all = "UPPERCASE")]
#[strum(serialize_all = "UPPERCASE")]
pub enum Currency {
	Eur,
	Usd,
	Gbp,
	Aud,
}

/// The header a client sets to make a write safe to retry.
pub const IDEMPOTENCY_KEY: &str = "idempotency-key";

fn amount(what: &str, v: i64) -> ApiResult<i64> {
	if v.unsigned_abs() > MAX_MINOR.unsigned_abs() {
		return Err(Refusal::new(Code::BadRequest, format!("{what} is more than {MAX_MINOR} minor units")));
	}
	Ok(v)
}

pub(crate) fn routes() -> Vec<ApiRoute> {
	use axum::http::Method as M;
	let work = Gate::USER.needs(Work::Read);
	let edit = Gate::USER.needs(Leads::Edit);
	vec![
		ApiRoute::new(M::GET, "/me", Gate::USER, me),
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
		ApiRoute::new(M::GET, "/funnel", Gate::USER.needs(Analysis::Read), funnel),
		ApiRoute::new(M::GET, "/places", work, list_places),
	]
}

/// Why an `/api/v1` operation refuses, beside its gate's (`sa_auth::Denied`) and its extractors'
/// (`ev_lib::api::Malformed`): a word the front end and an agent act on, `error` for a person.
#[derive(Clone, Copy, Debug, Serialize, strum::VariantArray)]
#[serde(rename_all = "snake_case")]
pub enum Code {
	/// What was sent does not pass a check; `error` names the field.
	BadRequest,
	NotFound,
	/// `expected_updated_at` is not what the place or the pricing stands at: read it again.
	Stale,
	PlaceExists,
	/// Another place is the landing of that GMB.
	GmbTaken,
	/// A place's settings refused; `error` names each field and why.
	InvalidSettings,
	/// A price list refused; `error` names its first problem by its path from the model's root.
	InvalidPricing,
	NoWonJob,
	/// An `Idempotency-Key` used for another action, or another invoice.
	IdempotencyKeyTaken,
	BookingDone,
	NoBooking,
	NotBooked,
	AlreadyAttached,
	/// No banking ScopeLedger is configured.
	InvoicesUnavailable,
	/// Banking has not granted the panel its scope.
	NotGranted,
	/// Banking could not be reached; the same request may be sent again.
	LedgerUnavailable,
	InvoiceNotOpen,
	InvoiceProcessing,
	ScopeFrozen,
	/// Banking's own idempotency refused the request.
	IdempotencyConflict,
	/// Banking refused for a reason of its own, which `error` carries.
	LedgerRefused,
	Internal,
}

impl ErrorCode for Code {
	fn status(self) -> StatusCode {
		match self {
			Self::BadRequest => StatusCode::BAD_REQUEST,
			Self::NotFound => StatusCode::NOT_FOUND,
			Self::InvalidSettings | Self::InvalidPricing => StatusCode::UNPROCESSABLE_ENTITY,
			Self::Stale
			| Self::PlaceExists
			| Self::GmbTaken
			| Self::NoWonJob
			| Self::IdempotencyKeyTaken
			| Self::BookingDone
			| Self::NoBooking
			| Self::NotBooked
			| Self::AlreadyAttached
			| Self::InvoiceNotOpen
			| Self::InvoiceProcessing
			| Self::ScopeFrozen
			| Self::IdempotencyConflict
			| Self::LedgerRefused => StatusCode::CONFLICT,
			Self::InvoicesUnavailable | Self::NotGranted | Self::LedgerUnavailable => StatusCode::SERVICE_UNAVAILABLE,
			Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
		}
	}
}

pub(crate) type ApiResult<T> = Result<T, Refusal<Code>>;

impl Refuses<eyre::Report> for Code {
	fn refusal(e: eyre::Report) -> Refusal<Self> {
		crate::report(&e, "operator API");
		Refusal::new(Self::Internal, "internal error")
	}
}

impl Refuses<Invalid> for Code {
	fn refusal(e: Invalid) -> Refusal<Self> {
		Refusal::new(Self::BadRequest, e.0)
	}
}

impl Refuses<ActionError> for Code {
	fn refusal(e: ActionError) -> Refusal<Self> {
		match e {
			ActionError::NotFound => Refusal::new(Self::NotFound, "not found"),
			ActionError::Conflict(c) => Refusal::new(
				match c {
					Conflict::NoWonJob => Self::NoWonJob,
					Conflict::IdempotencyKeyTaken => Self::IdempotencyKeyTaken,
					Conflict::BookingDone => Self::BookingDone,
					Conflict::NoBooking => Self::NoBooking,
					Conflict::NotBooked => Self::NotBooked,
					Conflict::AlreadyAttached => Self::AlreadyAttached,
				},
				c.to_string(),
			),
			ActionError::Invalid(e) => Self::refusal(e),
			ActionError::Internal(e) => Self::refusal(e),
		}
	}
}

/// The person an `/api/v1` route's gate admitted, their `sub` as the user id it is.
#[derive(Clone, Debug)]
pub(crate) struct Caller {
	pub user_id: Uuid,
	pub email: String,
	/// Their `sa:sites:*` permissions, and no others.
	pub permissions: PermissionSet,
}

impl<S: Send + Sync> FromRequestParts<S> for Caller {
	type Rejection = Refusal<Denied>;

	async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Refusal<Denied>> {
		let Ok(user) = User::from_request_parts(parts, state).await;
		let user_id = Uuid::parse_str(&user.sub).map_err(|_| Refusal::new(Denied::Unauthenticated, format!("the assertion's sub {:?} is not a user id", user.sub)))?;
		Ok(Self {
			user_id,
			email: user.email,
			permissions: user.permissions,
		})
	}
}

impl OperationInput for Caller {}

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
		.ok_or_else(|| Refusal::new(Code::BadRequest, "Idempotency-Key is 1–128 visible ASCII characters"))
}

impl<S: Send + Sync> FromRequestParts<S> for IdempotencyKey {
	type Rejection = Refusal<Code>;

	async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Refusal<Code>> {
		idempotency_key(parts).map(Self)
	}
}

impl<S: Send + Sync> FromRequestParts<S> for RequiredIdempotencyKey {
	type Rejection = Refusal<Code>;

	async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Refusal<Code>> {
		idempotency_key(parts)?.map(Self).ok_or_else(|| Refusal::new(Code::BadRequest, "Idempotency-Key is required"))
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
		None => to.checked_sub(SignedDuration::from_hours(24 * 29)).map_err(|e| Refusal::from(eyre::Report::from(e)))?,
	};
	if from > to {
		return Err(Refusal::new(Code::BadRequest, "from is after to"));
	}
	if (to - from).get_days() >= MAX_FUNNEL_DAYS {
		return Err(Refusal::new(Code::BadRequest, format!("at most {MAX_FUNNEL_DAYS} days at once")));
	}
	Ok((from, to))
}

/// The first instant of a UTC day.
fn day_start(d: Date) -> ApiResult<Timestamp> {
	Ok(d.to_zoned(TimeZone::UTC).map_err(|e| Refusal::from(eyre::Report::from(e)))?.timestamp())
}

// ── /me ─────────────────────────────────────────────────────────────────────────────────

/// `GET /me`.
#[derive(JsonSchema, Serialize, TS)]
pub struct Me {
	/// The concierge user id: what a user's own marks are kept under.
	sub: Uuid,
	/// The caller's `sa:sites:*` permissions.
	#[ts(type = "Array<Permission>")]
	permissions: Vec<SitesPermission>,
}

/// A `sa:sites:*` permission, one of those the conductor's catalog names.
#[derive(Serialize)]
#[serde(transparent)]
struct SitesPermission(String);

impl JsonSchema for SitesPermission {
	fn schema_name() -> Cow<'static, str> {
		"Permission".into()
	}

	fn json_schema(_: &mut SchemaGenerator) -> Schema {
		let words: Vec<String> = Catalog::collect("sa", 0).permissions.into_iter().filter(|p| p.starts_with(Service::Sites.prefix())).collect();
		json_schema!({ "type": "string", "enum": words })
	}
}

/// The caller, as the conductor vouched for them: their user id and `sa:sites:*` permissions.
async fn me(caller: Caller) -> Json<Me> {
	Json(Me {
		sub: caller.user_id,
		permissions: caller.permissions.iter().map(|p| SitesPermission(p.to_owned())).collect(),
	})
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
	/// Null for a lead seen before its creation.
	channel: Option<LeadChannel>,
	/// The ref the customer carries into a messenger (`AQ-7K3F`), when the landing made one.
	message_ref: Option<String>,
	/// When the customer first wrote on a messenger, and on which; null while they have not.
	messaged_at: Option<Timestamp>,
	messaged_channel: Option<Messenger>,
	/// Why the landing's antispam doubted it; null otherwise.
	suspect: Option<LeadSuspect>,
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
	/// A slug: the panel's `LostReason` words, or another a `lead.lost` of before them said.
	lost_reason: Option<String>,
	/// The landing's flow; null when it said none.
	flow: Option<LeadFlow>,
	/// The price an estimate or a fixed price showed, integer cents EUR TTC.
	quoted_cents: Option<i64>,
	/// When the pricing model behind `quoted_cents` took effect.
	pricing_valid_from: Option<Date>,
	/// An estimate's inputs, input id → value id.
	estimate_inputs: Option<BTreeMap<InputId, InputId>>,
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
	gmb: Option<Gmbs>,
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

/// The conductor's selection of GMBs, ids split by commas (`3,7`): only what belongs to the
/// places linked to one of them. Empty selects none; absent narrows nothing.
pub(crate) struct Gmbs(pub Vec<i64>);

impl<'de> Deserialize<'de> for Gmbs {
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		let raw = String::deserialize(d)?;
		if raw.is_empty() {
			return Ok(Self(Vec::new()));
		}
		raw.split(',')
			.map(|id| id.parse().map_err(|_| serde::de::Error::custom(format!("gmb is GMB ids split by commas, not {raw}"))))
			.collect::<Result<_, _>>()
			.map(Self)
	}
}

impl JsonSchema for Gmbs {
	fn schema_name() -> Cow<'static, str> {
		"Gmbs".into()
	}

	fn json_schema(_: &mut SchemaGenerator) -> Schema {
		json_schema!({
			"type": "string",
			"pattern": "^([0-9]+(,[0-9]+)*)?$",
			"description": "GMB ids split by commas (`3,7`): only what belongs to the places linked to one of them; empty selects none."
		})
	}
}

fn cursor_encode((at, brand, lead): &(Timestamp, String, String)) -> String {
	URL_SAFE_NO_PAD.encode(format!("{at}|{brand}|{lead}"))
}

fn cursor_decode(raw: &str) -> ApiResult<(Timestamp, String, String)> {
	let bad = || Refusal::new(Code::BadRequest, "cursor is not one this API gave");
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
async fn list_leads(State(panel): State<Panel>, caller: Caller, Params(q): Params<LeadsQuery>) -> ApiResult<Json<LeadPageDto>> {
	if let (Some(f), Some(t)) = (q.created_from, q.created_to)
		&& f > t
	{
		return Err(Refusal::new(Code::BadRequest, "created_from is after created_to"));
	}
	let next_day = |d: Date| d.tomorrow().map_err(|e| Refusal::from(eyre::Report::from(e))).and_then(day_start);
	let query = LeadQuery {
		stage: q.stage,
		brand: q.brand,
		gmbs: q.gmb.map(|g| g.0),
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
			.map(|r| MessageRef::from_typed(r).map_err(|_| Refusal::new(Code::BadRequest, "message_ref is not like AQ-7K3F")))
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
	/// How the registry judged it.
	status: EventStatus,
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
async fn get_lead(State(panel): State<Panel>, caller: Caller, Ids(LeadPath { brand, lead }): Ids<LeadPath>) -> ApiResult<Json<LeadCardDto>> {
	let now = Timestamp::now();
	let (view, events) = panel.lead_card(&brand, &lead, pii(&caller), now).await?.ok_or(Refusal::new(Code::NotFound, "not found"))?;
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
	/// A customer who called (`phone_inbound`, when absent), or wrote on a messenger without the
	/// landing.
	channel: Option<ManualChannel>,
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
async fn create_lead(State(panel): State<Panel>, caller: Caller, IdempotencyKey(key): IdempotencyKey, Body(b): Body<CreateLead>) -> ApiResult<Upserted<NewLeadDto>> {
	let new = NewLead {
		brand: b.brand,
		location: b.location,
		need: b.need,
		phone: b.phone,
		channel: b.channel.unwrap_or(ManualChannel::PhoneInbound),
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
		channel: Option<ContactChannel>,
	},
	Quoted {
		/// Minor units, |amount| ≤ 10^10.
		amount: Option<i64>,
		currency: Option<Currency>,
	},
	Won {
		job_id: Option<String>,
	},
	Lost {
		reason: LostReason,
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
	caller: Caller,
	Ids(LeadPath { brand, lead }): Ids<LeadPath>,
	IdempotencyKey(key): IdempotencyKey,
	Body(b): Body<StageBody>,
) -> ApiResult<Upserted<Journaled>> {
	let to = match b {
		StageBody::Contacted { channel } => StageMove::Contacted { channel },
		StageBody::Quoted { amount: a, currency: c } => StageMove::Quoted {
			amount: a.map(|a| amount("amount", a)).transpose()?,
			currency: c.map(|c| <&str>::from(c).to_owned()),
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
	caller: Caller,
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
	caller: Caller,
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
async fn attempt_call(State(panel): State<Panel>, caller: Caller, Ids(LeadPath { brand, lead }): Ids<LeadPath>) -> ApiResult<Created<AttemptDto>> {
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
	outcome: fact::CallOutcome,
}

/// How an attempted call went (`call.logged`).
async fn call_outcome(State(panel): State<Panel>, caller: Caller, Ids(AttemptPath { brand, lead, attempt }): Ids<AttemptPath>, Body(b): Body<OutcomeBody>) -> ApiResult<Created<Journaled>> {
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
	currency: Currency,
}

/// Records a payment for a lead's job, entered by hand (`payment.received`).
async fn record_payment(
	State(panel): State<Panel>,
	caller: Caller,
	Ids(LeadPath { brand, lead }): Ids<LeadPath>,
	IdempotencyKey(key): IdempotencyKey,
	Body(b): Body<PaymentBody>,
) -> ApiResult<Upserted<Journaled>> {
	let p = Payment {
		billed: amount("billed", b.billed)?,
		commission: amount("commission", b.commission)?,
		currency: <&str>::from(b.currency).to_owned(),
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
	gmb: Option<Gmbs>,
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
	let selected = q.gmb.map(|g| g.0);
	let slices = panel.funnel_slices(from, to, q.brand.as_ref(), selected.as_deref(), by).await?;
	let slices = match by {
		FunnelBy::All => Slices::Whole(slices.into_iter().next().ok_or_else(|| Refusal::from(eyre::eyre!("the whole funnel came back empty")))?.into()),
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
	gmb: Option<Gmbs>,
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
		.places(q.gmb.as_ref().map(|g| g.0.as_slice()))
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
	gmb: Option<Gmbs>,
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
	let counts = panel.lead_counts(q.brand.as_ref(), q.gmb.as_ref().map(|g| g.0.as_slice()), Timestamp::now()).await?;
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
