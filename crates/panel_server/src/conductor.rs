//! What the conductor's hubs read (`sa_core::feeds`, under its own assertion): the change feed
//! its live sockets relay, the notes its Telegram sends, when each source key last sent, which
//! place each GMB is the landing of, and the sites' reads since boot.

use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
	time::Duration,
};

use axum::{
	Json, Router,
	extract::{FromRef, State},
	response::Response,
	routing::get,
};
use ev_lib::api::{Params, Refusal, op_id};
use jiff::Timestamp;
use panel::{Panel, notifications::Notification};
use panel_core::{
	ids::LeadId,
	notify::{Button as SiteButton, Rendered},
};
use sa_alerts::{Call, Note, NotePage, NoteText};
use sa_auth::{Conductor, Service, Verifier};
use sa_core::{
	feeds::{self, BrandReads, Place, Places, Received},
	ids::BrandId,
	notify::Locale,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{self, ApiResult, Code, LeadPath};

/// The most notes one read answers.
const MAX_NOTES: i64 = 500;

/// The most brands [`InternalReads`] keeps; the sites are a handful.
const MAX_READ_BRANDS: usize = 256;

#[derive(Clone, Copy)]
pub(crate) enum InternalEndpoint {
	Locations,
	Pricing,
	Experiments,
}

/// The last `/api/internal` read per brand and endpoint, since boot. Memory only: the read path
/// writes nothing to the database. A brand past [`MAX_READ_BRANDS`] is not recorded and none is
/// evicted, so what the record holds stays what was first seen, and an unauthenticated caller
/// inventing brand names cannot push real ones out.
pub(crate) struct InternalReads {
	since: Timestamp,
	last: Mutex<HashMap<BrandId, BrandReads>>,
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
		let reads = last.entry(brand.clone()).or_default();
		let at = match endpoint {
			InternalEndpoint::Locations => &mut reads.locations,
			InternalEndpoint::Pricing => &mut reads.pricing,
			InternalEndpoint::Experiments => &mut reads.experiments,
		};
		*at = Some(Timestamp::now());
	}
}

#[derive(Clone)]
struct Reads {
	panel: Panel,
	locale: Locale,
	internal: Arc<InternalReads>,
	verifier: Verifier,
}

impl FromRef<Reads> for Verifier {
	fn from_ref(r: &Reads) -> Self {
		r.verifier.clone()
	}
}

/// The notes, the receipts, the places and the internal reads.
pub(crate) fn reads(panel: Panel, locale: Locale, internal: Arc<InternalReads>, verifier: Verifier) -> Router {
	Router::new()
		.route(feeds::NOTIFICATIONS, get(notifications))
		.route(feeds::SOURCES_RECEIVED, get(received))
		.route(feeds::PLACES, get(places))
		.route(feeds::INTERNAL_READS, get(internal_reads))
		.with_state(Reads { panel, locale, internal, verifier })
}

#[derive(Clone)]
struct Feed {
	panel: Panel,
	verifier: Verifier,
}

impl FromRef<Feed> for Verifier {
	fn from_ref(f: &Feed) -> Self {
		f.verifier.clone()
	}
}

/// The change feed: unbounded in time, so outside the reads' timeout.
pub(crate) fn changes(panel: Panel, verifier: Verifier) -> Router {
	Router::new().route(feeds::CHANGES, get(feed)).with_state(Feed { panel, verifier })
}

/// The tables each committed write touches (`panel::live`), until the process stops.
async fn feed(_: Conductor, State(Feed { panel, .. }): State<Feed>) -> Response {
	feeds::changes(panel.bus().subscribe(), panel.bus().gone())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NotesQuery {
	after: i64,
	limit: i64,
}

fn text(r: &Rendered) -> NoteText {
	let entities = r.code.iter().map(|e| json!({"type": "code", "offset": e.offset, "length": e.length})).collect();
	NoteText { text: r.text.clone(), entities }
}

/// A button's action: the operation the conductor calls for the user who pressed, by its id
/// in the table, the path's parameters by their names in it.
fn button(b: SiteButton, lead: &LeadPath, locale: Locale) -> sa_alerts::Button {
	let Value::Object(path) = serde_json::to_value(lead).expect("ids serialize") else {
		unreachable!("a struct serializes as an object")
	};
	let call = match b {
		SiteButton::Take => Call {
			operation: op_id(api::move_lead).into(),
			path,
			body: json!({"stage": "contacted"}),
		},
		SiteButton::NoAnswer => Call {
			operation: op_id(api::no_answer).into(),
			path,
			body: Value::Null,
		},
	};
	sa_alerts::Button {
		id: b.as_str().into(),
		label: b.label(locale).into(),
		pressed: b.pressed(locale).into(),
		call,
		leaves: b.leaves().iter().map(|left| left.as_str().into()).collect(),
	}
}

fn note(n: &Notification, locale: Locale) -> ApiResult<Note> {
	let buttons = match (&n.lead, n.buttons.as_slice()) {
		(_, []) => Vec::new(),
		(Some((brand, lead)), buttons) => {
			let stored = |e: sa_core::Invalid| eyre::eyre!("note {}'s lead {brand}/{lead}: {e}", n.seq);
			let lead = LeadPath {
				brand: BrandId::parse(brand).map_err(stored)?,
				lead: LeadId::parse(lead).map_err(stored)?,
			};
			buttons.iter().map(|button| self::button(*button, &lead, locale)).collect()
		}
		(None, _) => return Err(Refusal::from(eyre::eyre!("note {} has buttons and no lead", n.seq))),
	};
	let note = Note {
		seq: n.seq,
		rule: n.rule,
		event_id: n.event_id,
		created_at: n.created_at,
		text: text(&n.text),
		text_pii: n.text_pii.as_ref().map(text),
		skip_user: n.skip_user,
		buttons,
		gmbs: Vec::new(),
		photo: None,
	};
	note.check(Service::Sites).map_err(|error| Refusal::from(eyre::Report::from(error)))?;
	Ok(note)
}

async fn notifications(_: Conductor, State(r): State<Reads>, Params(q): Params<NotesQuery>) -> ApiResult<Json<NotePage>> {
	if !(1..=MAX_NOTES).contains(&q.limit) || q.after < 0 {
		return Err(Refusal::new(Code::BadRequest, format!("limit is 1–{MAX_NOTES}, after ≥ 0")));
	}
	let notes = r.panel.notifications(q.after, q.limit).await?;
	let next = notes.last().map_or(q.after, |note| note.seq);
	let notes = notes.iter().map(|note| self::note(note, r.locale)).collect::<ApiResult<Vec<_>>>()?;
	Ok(Json(NotePage { notes, next }))
}

async fn received(_: Conductor, State(r): State<Reads>) -> ApiResult<Json<Received>> {
	Ok(Json(r.panel.received_by_key().await?.into_iter().collect()))
}

async fn places(_: Conductor, State(r): State<Reads>) -> ApiResult<Json<Places>> {
	let places = r
		.panel
		.gmbs(Timestamp::now())
		.await?
		.into_iter()
		.filter(|&(.., withdrawn)| !withdrawn)
		.map(|(gmb_id, brand, slug, ..)| Place { gmb_id, brand, slug })
		.collect();
	Ok(Json(Places { places }))
}

async fn internal_reads(_: Conductor, State(r): State<Reads>) -> Json<feeds::InternalReads> {
	let last = r.internal.last.lock().expect("no panic while held");
	Json(feeds::InternalReads {
		since: r.internal.since,
		reads: last.iter().map(|(brand, reads)| (brand.as_str().to_owned(), reads.clone())).collect(),
	})
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
