//! PostHog's capture API (reqwest) and the background pass of `serve` that sends what the
//! journal queued for it ([`panel::capture`]).
//!
//! `POST {POSTHOG_HOST}/batch/` with the project's key — a `phc_` key, public by design (the
//! landings ship it to every browser), so not a secret. Outbound only: the pods need egress to
//! the capture host (`us.i.posthog.com:443`). Without `POSTHOG_PROJECT_API_KEY` nothing is
//! queued and none of this runs; `serve` says so.
//!
//! Not `ev_lib::analytics`: its client sends one event per request without an `uuid` or a
//! `timestamp`, so a retry would count twice and every event would land at its send time, and
//! the feature pulls in the Dioxus front end's crates.

use std::time::Duration;

use panel::{
	Panel,
	capture::{Captured, Capturer, SendError},
};
use serde_json::{Value, json};
use tokio::sync::watch;

/// How often the outbox is looked at.
const TICK: Duration = Duration::from_secs(5);

/// PostHog's capture API for one project.
#[derive(Clone)]
pub struct CaptureApi {
	http: reqwest::Client,
	url: String,
	key: String,
}

impl std::fmt::Debug for CaptureApi {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("CaptureApi").field("url", &self.url).finish_non_exhaustive()
	}
}

impl CaptureApi {
	/// `host`: the capture origin, e.g. `https://us.i.posthog.com`, or a test's mock.
	pub fn new(host: &str, key: &str) -> eyre::Result<Self> {
		let _ = rustls::crypto::ring::default_provider().install_default(); //IGNORED_ERROR: Err is "a provider is already installed", which is all reqwest needs
		let http = reqwest::Client::builder()
			.connect_timeout(Duration::from_secs(5))
			.timeout(Duration::from_secs(20))
			.redirect(reqwest::redirect::Policy::none())
			.build()?;
		Ok(Self {
			http,
			url: format!("{}/batch/", host.trim_end_matches('/')),
			key: key.to_owned(),
		})
	}

	/// The body of `/batch/`.
	pub fn body(&self, batch: &[Captured]) -> Value {
		let events: Vec<Value> = batch
			.iter()
			.map(|c| {
				let mut properties = c.properties.clone();
				properties["$lib"] = json!("sa-panel");
				json!({
					"event": c.event,
					"distinct_id": c.distinct_id,
					"uuid": c.uuid.to_string(),
					"timestamp": c.timestamp.to_string(),
					"properties": properties,
				})
			})
			.collect();
		json!({"api_key": self.key, "batch": events})
	}
}

impl Capturer for CaptureApi {
	async fn send(&self, batch: &[Captured]) -> Result<(), SendError> {
		let res = self
			.http
			.post(&self.url)
			.json(&self.body(batch))
			.send()
			.await
			.map_err(|e| SendError::Retry(format!("no answer: {e}")))?;
		let status = res.status();
		if status.is_success() {
			return Ok(());
		}
		// PostHog's own words, cut short; the key is never in them.
		let why = format!("{status}: {}", res.text().await.unwrap_or_default().chars().take(300).collect::<String>());
		if status.is_server_error() || status.as_u16() == 429 || status.as_u16() == 408 {
			Err(SendError::Retry(why))
		} else {
			Err(SendError::Refused(why))
		}
	}
}

/// Sends what is due until `shutdown` turns true: a pass every `TICK`, the next at once while
/// a full batch went.
pub async fn run(panel: Panel, api: CaptureApi, shutdown: watch::Receiver<bool>) {
	crate::every(TICK, shutdown, "sending events to posthog", || async {
		loop {
			let done = panel.capture_pass(&api, jiff::Timestamp::now()).await?;
			if done.sent + done.retried + done.dropped < panel::capture::BATCH as u64 || done.retried > 0 {
				return Ok(());
			}
		}
	})
	.await;
}
