//! The HTTP API: `/health`; ingest and a bot's lookup, under a source's assertion; the operator
//! API (`/api/v1/*`, [`crate::api`] and the others' routes) under a user's; the conductor's reads
//! (`/api/conductor/*`, [`crate::conductor`]) under its own ([`crate::gate`]); and, open, the
//! sites' reads (`/api/internal`, [`crate::places`], [`crate::pricing`], [`crate::experiments`]),
//! the booking webhooks (`/api/hooks`) and the front end's bundle (`/mfe/*`).
//!
//! `POST /api/ingest/v1/events` comes from the conductor, which checked the source's MAC over the
//! raw body (docs/ARCHITECTURE.md, Ingest): its assertion names the key and binds the body's
//! digest. The handler only translates; what is accepted and why is the engine's. A request
//! without an assertion is refused before its body is read, and the body is read up to
//! [`MAX_BODY`] only. The route sheds load past a number of requests at once, and bounds how long
//! a body may take to arrive and a request in all ([`Limits`]).
//!
//! `GET /api/ingest/v1/leads/by-ref/{brand}/{ref}` is a bot's lookup of the lead a messenger
//! ref names, under the same assertion over an empty body (docs/BOT-API.md): `200` with the lead,
//! `404` when no lead of the brand carries the ref, `403` for a key that is not a bot's of that
//! brand, `400` for a brand or ref that cannot be one, `401` as for ingest.
//!
//! Answers: `207` with a verdict per event (protojson `sa.v1.IngestResponse`); `409` with
//! `Retry-After` and the same verdicts when an event names what has not arrived yet (a
//! `booking.requested` before its `lead.created`: `deferred`) — kitstart's outbox retries a
//! 409 for `booking.requested`, and what was journaled of the batch is a duplicate then;
//! `401` without a source's assertion for this body; `400` for a body that is not a batch; `408`
//! for a body too slow to arrive; `413` past [`MAX_BODY`]; `503` when shedding load; `500` for
//! our own failures, reported.

use std::{path::PathBuf, sync::Arc, time::Duration};

use axum::{
	Extension, Json, Router,
	body::to_bytes,
	error_handling::HandleErrorLayer,
	extract::{Path, Request, State},
	handler::Handler,
	http::{HeaderValue, Method, StatusCode, header},
	middleware::{self, Next},
	response::{IntoResponse, Response},
	routing::{MethodFilter, MethodRouter, get, on, post},
};
use panel::{IngestError, Outcome, Panel, booking::PushSources};
use panel_contracts::v1::{EventResult, IngestResponse};
use panel_core::notify::Locale;
use sa_auth::{Analysis, Keys, PermissionSet, Work};
use serde_json::{Value, json};
use tower::{BoxError, ServiceBuilder, limit::GlobalConcurrencyLimitLayer};
use tower_http::{
	services::ServeDir,
	timeout::{RequestBodyTimeoutLayer, TimeoutError, TimeoutLayer},
};

use crate::{
	api, booking, conductor, experiments,
	gate::{self, UserGate},
	invoices, places, pricing,
};

/// A part of the sites, as its screens are grouped, and the permission that opens it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Section {
	/// Leads, bookings, places, pricing, invoices: the daily work.
	Work,
	/// The overview's funnel, experiments.
	Analysis,
}

impl Section {
	pub fn may(self, permissions: &PermissionSet) -> bool {
		match self {
			Self::Work => permissions.may(Work::Read),
			Self::Analysis => permissions.may(Analysis::Read),
		}
	}
}

/// One `/api/v1` route served on the engine's state, and who passes its gate.
pub struct ApiRoute {
	pub method: Method,
	/// Under `/api/v1`.
	pub path: &'static str,
	/// `None`: every signed-in user.
	pub section: Option<Section>,
	handler: MethodRouter<Panel>,
}

impl ApiRoute {
	pub(crate) fn new<H: Handler<T, Panel>, T: 'static>(method: Method, path: &'static str, section: Option<Section>, handler: H) -> Self {
		let filter = MethodFilter::try_from(method.clone()).expect("the routes name standard methods");
		Self {
			method,
			path,
			section,
			handler: on(filter, handler),
		}
	}
}

/// Every `/api/v1` route on the engine's state.
pub fn api_routes() -> Vec<ApiRoute> {
	[api::routes(), booking::routes(), places::routes(), pricing::routes(), experiments::routes(), invoices::routes()]
		.into_iter()
		.flatten()
		.collect()
}

/// 500 events of about 8 KiB each: a batch is refused well before it strains anything.
pub const MAX_BODY: usize = 4 * 1024 * 1024;

/// What one ingest request may cost.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
	/// Ingest requests served at once; past it, `503` rather than a queue.
	pub max_concurrent: usize,
	/// How long a body may take to arrive.
	pub body_timeout: Duration,
	/// How long a request may take in all: 500 short transactions, with room to spare.
	pub request_timeout: Duration,
	/// `/api/v1` and the conductor's reads, at once.
	pub api_concurrent: usize,
	/// `/api/v1` and the conductor's reads: a few queries each (the change feed is unbounded).
	pub api_timeout: Duration,
}

impl Default for Limits {
	fn default() -> Self {
		Self {
			max_concurrent: 32,
			body_timeout: Duration::from_secs(10),
			request_timeout: Duration::from_secs(30),
			api_concurrent: 32,
			api_timeout: Duration::from_secs(15),
		}
	}
}

/// What `serve` answers with.
pub struct Sites {
	pub panel: Panel,
	/// The conductor's public keys (`PANEL_ASSERTION_KEYS`).
	pub keys: Keys,
	/// The notes' buttons' language (`TELEGRAM_LOCALE`).
	pub locale: Locale,
	/// The booking webhooks answered (`POST /api/hooks/booking/…`); a provider not among them
	/// is a 404.
	pub hooks: PushSources,
	/// The front end's bundle (`PANEL_MFE_DIR`); `None`: `/mfe/*` is a 404.
	pub mfe: Option<PathBuf>,
}

/// The sites' reads, mounted by [`router`] and listed in the contract's `provides`.
const INTERNAL: &str = "/api/internal";

/// Every path the router mounts for another service to call (`--print-provided-paths`).
pub const PROVIDED_PATHS: [&str; 1] = [INTERNAL];

pub fn router(sites: Sites, limits: Limits) -> Router {
	let keys = Arc::new(sites.keys);
	let v1 = api_routes().into_iter().fold(Router::new(), |routes, r| {
		let gate = UserGate {
			keys: keys.clone(),
			section: r.section,
		};
		routes.merge(Router::new().route(r.path, r.handler).route_layer(middleware::from_fn_with_state(gate, gate::user)))
	});
	let v1 = bounded(v1.with_state(sites.panel.clone()), limits.api_concurrent, limits.api_timeout);
	let internal_reads = conductor::InternalReads::new();
	let reads = bounded(
		conductor::reads(sites.panel.clone(), sites.locale, internal_reads.clone()),
		limits.api_concurrent,
		limits.api_timeout,
	);
	let to_conductor = reads
		.merge(conductor::changes(sites.panel.clone()))
		.route_layer(middleware::from_fn_with_state(keys.clone(), gate::conductor));
	let ingest_layers = ServiceBuilder::new()
		// The only error the layers below raise is the shed: everything else is a response.
		.layer(HandleErrorLayer::new(|_: BoxError| async { error(StatusCode::SERVICE_UNAVAILABLE, "busy, try again") }))
		.load_shed()
		.concurrency_limit(limits.max_concurrent)
		.layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, limits.request_timeout))
		.layer(RequestBodyTimeoutLayer::new(limits.body_timeout));
	let ingest = Router::new()
		.route("/api/ingest/v1/events", post(ingest).layer(ingest_layers.clone()))
		.route("/api/ingest/v1/leads/by-ref/{brand}/{message_ref}", get(lead_by_ref).layer(ingest_layers))
		.with_state(Ingest { panel: sites.panel.clone(), keys });
	let mut app = Router::new()
		.route("/health", get(|| async { "ok" }))
		.merge(ingest)
		.nest(INTERNAL, places::internal().with_state(sites.panel.clone()).layer(Extension(internal_reads)))
		.merge(booking::hooks(sites.panel, sites.hooks))
		.nest("/api/v1", v1)
		.nest("/api/conductor", to_conductor);
	if let Some(dir) = sites.mfe {
		app = app.nest("/mfe", Router::new().fallback_service(ServeDir::new(dir)).layer(middleware::from_fn(mfe_caching)));
	}
	app.layer(middleware::map_response(nosniff))
}

/// The bundle's entry names (under `/mfe`) are asked for on every load; the rest are named by
/// their hash.
const MFE_ENTRIES: [&str; 2] = ["/mfe-sites.js", "/mfe-sites.css"];

async fn mfe_caching(req: Request, next: Next) -> Response {
	let path = req.uri().path().to_owned();
	let mut res = next.run(req).await;
	if res.status().is_success() {
		let policy = if MFE_ENTRIES.contains(&path.as_str()) {
			"no-cache"
		} else {
			"public, max-age=31536000, immutable"
		};
		res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
	}
	res
}

#[derive(Clone)]
struct Ingest {
	panel: Panel,
	keys: Arc<Keys>,
}

/// At most `concurrent` at once across `routes`, past it `503` rather than a queue; at most
/// `timeout` each. The semaphore is shared: `Router::layer` wraps every route on its own, and
/// a plain concurrency limit would give each route a budget of its own.
fn bounded<S: Clone + Send + Sync + 'static>(routes: Router<S>, concurrent: usize, timeout: Duration) -> Router<S> {
	let permits = Arc::new(tokio::sync::Semaphore::new(concurrent));
	routes.layer(
		ServiceBuilder::new()
			// The only error the layers below raise is the shed: everything else is a response.
			.layer(HandleErrorLayer::new(|_: BoxError| async { error(StatusCode::SERVICE_UNAVAILABLE, "busy, try again") }))
			.load_shed()
			.layer(GlobalConcurrencyLimitLayer::with_semaphore(permits))
			.layer(TimeoutLayer::with_status_code(StatusCode::SERVICE_UNAVAILABLE, timeout)),
	)
}

/// Every answer is what its `Content-Type` says, never sniffed into something else.
async fn nosniff(mut res: Response) -> Response {
	res.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
	res
}

fn error(status: StatusCode, msg: impl Into<String>) -> Response {
	(status, Json(json!({ "error": msg.into() }))).into_response()
}

/// When a batch with a deferred event is to come again, in seconds: a lead's own event is
/// usually a moment behind.
const DEFERRED_RETRY_AFTER: &str = "30";

/// What went wrong reading a body: too big, too slow, or cut off.
fn body_error(e: axum::Error) -> Response {
	let inner = e.into_inner();
	let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(inner.as_ref());
	while let Some(c) = cause {
		if c.is::<http_body_util::LengthLimitError>() {
			return error(StatusCode::PAYLOAD_TOO_LARGE, format!("the body is larger than {MAX_BODY} bytes"));
		}
		if c.is::<TimeoutError>() {
			return error(StatusCode::REQUEST_TIMEOUT, "the body took too long to arrive");
		}
		cause = c.source();
	}
	error(StatusCode::BAD_REQUEST, "the body could not be read")
}

/// `Body` stays the last extractor and is read by hand: the assertion binds the body exactly as
/// it came, and nothing is read without one.
async fn ingest(State(ingest): State<Ingest>, req: Request) -> Response {
	let (parts, body) = req.into_parts();
	if !gate::asserted(&parts.headers) {
		return gate::unauthenticated();
	}
	let body = match to_bytes(body, MAX_BODY).await {
		Ok(body) => body,
		Err(e) => return body_error(e),
	};
	let Some(grant) = gate::source(&ingest.keys, &parts.headers, &parts.method, parts.uri.path(), &body) else {
		return gate::unauthenticated();
	};
	let now = jiff::Timestamp::now();
	match ingest.panel.ingest(&grant, &body, now).await {
		Ok(verdicts) => {
			let deferred = verdicts.iter().any(|v| matches!(v.outcome, Outcome::Deferred(_)));
			let results = verdicts
				.into_iter()
				.map(|v| {
					let (status, reason) = match v.outcome {
						Outcome::Accepted { unregistered: false } => ("accepted", None),
						Outcome::Accepted { unregistered: true } => ("accepted", Some("type not registered: stored, not projected".to_owned())),
						Outcome::Duplicate => ("duplicate", None),
						Outcome::Rejected(e) => ("rejected", Some(e.0)),
						Outcome::Deferred(e) => ("deferred", Some(e.0)),
					};
					EventResult {
						// `wire::MAX_BATCH` keeps this far below u32::MAX.
						index: u32::try_from(v.index).unwrap_or(u32::MAX),
						id: v.id,
						status: status.to_owned(),
						reason,
					}
				})
				.collect();
			if deferred {
				// A 207's `rejected` is final to kitstart's outbox; a 409 (or 425) on
				// `booking.requested` it sends again (lib#185). Not a 5xx: nothing failed here.
				let mut res = (StatusCode::CONFLICT, Json(IngestResponse { results })).into_response();
				res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from_static(DEFERRED_RETRY_AFTER));
				return res;
			}
			(StatusCode::MULTI_STATUS, Json(IngestResponse { results })).into_response()
		}
		Err(e) => ingest_error(e, "ingest failed"),
	}
}

/// A refusal of the whole request, ingest's or a lookup's.
fn ingest_error(e: IngestError, what: &'static str) -> Response {
	match e {
		IngestError::Forbidden(why) => error(StatusCode::FORBIDDEN, why),
		IngestError::BadRequest(e) => error(StatusCode::BAD_REQUEST, e.0),
		IngestError::Internal(e) => {
			crate::report(&e, what);
			error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
		}
	}
}

/// A bot's lookup of the lead a ref names, under its key's assertion over an empty body (the
/// conductor checked the MAC over the method and the path). Never the customer's name or phone.
async fn lead_by_ref(State(ingest): State<Ingest>, Path((brand, message_ref)): Path<(String, String)>, req: Request) -> Response {
	let Some(grant) = gate::source(&ingest.keys, req.headers(), req.method(), req.uri().path(), b"") else {
		return gate::unauthenticated();
	};
	match ingest.panel.lead_by_ref(&grant, &brand, &message_ref).await {
		Ok(Some(lead)) => {
			let mut body = json!({
				"lead_id": lead.lead_id,
				"channel": lead.channel,
				"stage": lead.stage,
				"created_at": lead.created_at.map(|t| t.to_string()),
				"message_ref": lead.message_ref,
			});
			let optional = [
				("need", lead.need.map(Value::from)),
				("locality", lead.locality.map(Value::from)),
				("quoted_cents", lead.quoted_cents.map(Value::from)),
				("flow", lead.flow.map(Value::from)),
				("messaged_at", lead.messaged_at.map(|t| Value::from(t.to_string()))),
				("messaged_channel", lead.messaged_channel.map(Value::from)),
			];
			for (k, v) in optional {
				if let Some(v) = v {
					body[k] = v;
				}
			}
			(StatusCode::OK, Json(body)).into_response()
		}
		Ok(None) => error(StatusCode::NOT_FOUND, "no lead of the brand carries that ref"),
		Err(e) => ingest_error(e, "lookup by ref failed"),
	}
}
