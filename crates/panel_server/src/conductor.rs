//! What the conductor's hubs read (`/api/conductor/*`, under its own assertion): the change feed
//! its live sockets relay, the notes its Telegram sends, and when each source key last sent.
//!
//! ```text
//! GET /changes                     application/x-ndjson, one object per line, open until the
//!                                  process stops: {"type":"resync"} first, after a rebuild and
//!                                  when the reader fell 1024 behind; {"type":"changed", topic,
//!                                  brand_id?, id?, at, permission}; {"type":"ping"} every 25 s
//! GET /notifications?after&limit   {notes: [Note], next}: the notes after `after`, ≤ limit (≤ 500)
//! GET /sources/received            {<key_id>: <RFC 3339 of its last event's arrival>}
//! GET /internal-reads              {since: <boot, RFC 3339>, reads: {<brand>: {locations?, pricing?,
//!                                  experiments?: <RFC 3339 of the last `/api/internal` read>}}}
//! ```

use std::{
	collections::{BTreeMap, HashMap},
	sync::{Arc, Mutex},
	time::Duration,
};

use axum::{
	Json, Router,
	body::{Body, Bytes},
	extract::{Query, State},
	http::{HeaderValue, header},
	response::{IntoResponse, Response},
	routing::get,
};
use jiff::Timestamp;
use panel::{
	Panel,
	live::{Change, Signal},
	notifications::Notification,
};
use panel_core::{
	ids::BrandId,
	notify::{Button, Locale, Rendered},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast::{Receiver, error::RecvError};

use crate::api::{ApiError, ApiResult};

/// A line at least this often, so the conductor tells a quiet feed from a dead one.
const PING_EVERY: Duration = Duration::from_secs(25);

/// The most notes one read answers.
const MAX_NOTES: i64 = 500;

/// The most brands [`InternalReads`] keeps; kitstart sites are a handful.
const MAX_READ_BRANDS: usize = 256;

#[derive(Clone, Copy)]
pub(crate) enum InternalEndpoint {
	Locations,
	Pricing,
	Experiments,
}

impl InternalEndpoint {
	fn as_str(self) -> &'static str {
		match self {
			Self::Locations => "locations",
			Self::Pricing => "pricing",
			Self::Experiments => "experiments",
		}
	}
}

/// The last `/api/internal` read per brand and endpoint, since boot. Memory only: the read path
/// writes nothing to the database. A brand past [`MAX_READ_BRANDS`] is not recorded and none is
/// evicted, so what the record holds stays what was first seen, and an unauthenticated caller
/// inventing brand names cannot push real ones out.
pub(crate) struct InternalReads {
	since: Timestamp,
	last: Mutex<HashMap<BrandId, BTreeMap<&'static str, Timestamp>>>,
}

impl InternalReads {
	pub(crate) fn new() -> Arc<Self> {
		Arc::new(Self {
			since: Timestamp::now(),
			last: Mutex::default(),
		})
	}

	pub(crate) fn record(&self, brand: &BrandId, endpoint: InternalEndpoint) {
		let mut last = self.last.lock().expect("no panic while held");
		if last.len() >= MAX_READ_BRANDS && !last.contains_key(brand) {
			return;
		}
		last.entry(brand.clone()).or_default().insert(endpoint.as_str(), Timestamp::now());
	}
}

#[derive(Clone)]
struct Reads {
	panel: Panel,
	locale: Locale,
	internal: Arc<InternalReads>,
}

/// `/notifications`, `/sources/received` and `/internal-reads`.
pub(crate) fn reads(panel: Panel, locale: Locale, internal: Arc<InternalReads>) -> Router {
	Router::new()
		.route("/notifications", get(notifications))
		.route("/sources/received", get(received))
		.route("/internal-reads", get(internal_reads))
		.with_state(Reads { panel, locale, internal })
}

/// `/changes`: unbounded in time, so outside the reads' timeout.
pub(crate) fn changes(panel: Panel) -> Router {
	Router::new().route("/changes", get(feed)).with_state(panel)
}

fn line(v: &Value) -> Bytes {
	let mut out = serde_json::to_vec(v).expect("JSON");
	out.push(b'\n');
	out.into()
}

fn changed(c: &Change) -> Value {
	let mut v = json!({
		"type": "changed",
		"topic": c.topic.as_str(),
		"at": c.at.to_string(),
		"permission": c.permission(),
	});
	if let Some(b) = &c.brand {
		v["brand_id"] = b.as_str().into();
	}
	if let Some(id) = &c.id {
		v["id"] = id.as_str().into();
	}
	v
}

/// What the feed sends next: a line, or nothing more.
async fn next_line(rx: &mut Receiver<Signal>, ping: &mut tokio::time::Interval) -> Option<Bytes> {
	tokio::select! {
		signal = rx.recv() => match signal {
			Ok(Signal::Changed(c)) => Some(line(&changed(&c))),
			Ok(Signal::Resync) | Err(RecvError::Lagged(_)) => Some(line(&json!({"type": "resync"}))),
			Ok(Signal::GoingAway) | Err(RecvError::Closed) => None,
		},
		_ = ping.tick() => Some(line(&json!({"type": "ping"}))),
	}
}

async fn feed(State(panel): State<Panel>) -> Response {
	// Subscribed before the first line: whatever commits after the resync is in the stream.
	let rx = panel.bus().subscribe();
	let mut ping = tokio::time::interval_at(tokio::time::Instant::now() + PING_EVERY, PING_EVERY);
	ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
	let first = futures::stream::once(async { Ok::<_, std::convert::Infallible>(line(&json!({"type": "resync"}))) });
	let rest = futures::stream::unfold((rx, ping), |(mut rx, mut ping)| async move {
		let l = next_line(&mut rx, &mut ping).await?;
		Some((Ok(l), (rx, ping)))
	});
	let mut res = Body::from_stream(futures::StreamExt::chain(first, rest)).into_response();
	res.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static("application/x-ndjson"));
	res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
	res
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NotesQuery {
	after: i64,
	limit: i64,
}

fn text(r: &Rendered) -> Value {
	let entities: Vec<Value> = r.code.iter().map(|e| json!({"type": "code", "offset": e.offset, "length": e.length})).collect();
	json!({ "text": r.text, "entities": entities })
}

/// A button's action, as the conductor calls it for the user who pressed.
fn button(b: Button, (brand, lead): &(String, String), locale: Locale) -> Value {
	let call = match b {
		Button::Take => json!({"method": "POST", "path": format!("/api/v1/leads/{brand}/{lead}/stage"), "body": {"stage": "contacted"}}),
		Button::NoAnswer => json!({"method": "POST", "path": format!("/api/v1/leads/{brand}/{lead}/no_answer"), "body": null}),
	};
	json!({
		"id": b.as_str(),
		"label": b.label(locale),
		"pressed": b.pressed(locale),
		"call": call,
		"leaves": b.leaves().iter().map(|l| l.as_str()).collect::<Vec<_>>(),
	})
}

fn note(n: &Notification, locale: Locale) -> ApiResult<Value> {
	let buttons = match (&n.lead, n.buttons.as_slice()) {
		(_, []) => Vec::new(),
		(Some(lead), buttons) => buttons.iter().map(|b| button(*b, lead, locale)).collect(),
		(None, _) => return Err(ApiError::Internal(eyre::eyre!("note {} has buttons and no lead", n.seq))),
	};
	Ok(json!({
		"seq": n.seq,
		"rule": n.rule,
		"event_id": n.event_id.to_string(),
		"created_at": n.created_at.to_string(),
		"text": text(&n.text),
		"text_pii": n.text_pii.as_ref().map(text),
		"skip_user": n.skip_user.map(|u| u.to_string()),
		"buttons": buttons,
	}))
}

async fn notifications(State(r): State<Reads>, q: Result<Query<NotesQuery>, axum::extract::rejection::QueryRejection>) -> ApiResult<Json<Value>> {
	let Query(q) = q.map_err(|e| ApiError::BadRequest(e.body_text()))?;
	if !(1..=MAX_NOTES).contains(&q.limit) || q.after < 0 {
		return Err(ApiError::BadRequest(format!("limit is 1–{MAX_NOTES}, after ≥ 0")));
	}
	let notes = r.panel.notifications(q.after, q.limit).await?;
	let next = notes.last().map_or(q.after, |n| n.seq);
	let notes = notes.iter().map(|n| note(n, r.locale)).collect::<ApiResult<Vec<_>>>()?;
	Ok(Json(json!({ "notes": notes, "next": next })))
}

async fn received(State(r): State<Reads>) -> ApiResult<Json<Value>> {
	let by_key: serde_json::Map<String, Value> = r.panel.received_by_key().await?.into_iter().map(|(k, at)| (k, at.to_string().into())).collect();
	Ok(Json(Value::Object(by_key)))
}

async fn internal_reads(State(r): State<Reads>) -> Json<Value> {
	let last = r.internal.last.lock().expect("no panic while held");
	let reads: BTreeMap<&str, BTreeMap<&str, String>> = last
		.iter()
		.map(|(brand, endpoints)| (brand.as_str(), endpoints.iter().map(|(e, at)| (*e, at.to_string())).collect()))
		.collect();
	Json(json!({ "since": r.internal.since.to_string(), "reads": reads }))
}

/// How often the fan-out looks for what is due.
const FAN_OUT_EVERY: Duration = Duration::from_secs(2);

/// The fan-out, every `FAN_OUT_EVERY` until `shutdown` (`serve`'s).
pub async fn fan_out(panel: Panel, locale: Locale, shutdown: tokio::sync::watch::Receiver<bool>) {
	crate::every(FAN_OUT_EVERY, shutdown, "the notifications' fan-out", || async {
		panel.fan_out(jiff::Timestamp::now(), locale).await?;
		Ok(())
	})
	.await;
}
