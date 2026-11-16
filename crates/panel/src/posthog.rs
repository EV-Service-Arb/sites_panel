//! Links into the PostHog project the landings send to: the panel counts no visits, it opens
//! PostHog's insights on them.

use jiff::civil::Date;
use sa_core::ids::BrandId;
use serde_json::{Value, json};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PosthogProject {
	/// The app's origin, e.g. `https://us.posthog.com` (not the capture host).
	pub app_host: String,
	pub project_id: String,
}

impl PosthogProject {
	/// `{app_host}/project/{id}/insights/new#q=<query>`: an unsaved insight, nothing written to PostHog.
	pub(crate) fn insight_url(&self, query: &Value) -> String {
		format!(
			"{}/project/{}/insights/new#q={}",
			self.app_host.trim_end_matches('/'),
			self.project_id,
			uri_component(&query.to_string())
		)
	}

	/// A visit to a payment for the leads of `[from, to]`: `location_page_view` → `sa_lead_created`
	/// → `sa_lead_contacted` → `sa_job_won` → `sa_payment_received`, one brand's, or every brand's
	/// broken down by `brand_id`.
	pub(crate) fn funnel_url(&self, brand: Option<&BrandId>, from: Date, to: Date) -> String {
		let series =
			["location_page_view", "sa_lead_created", "sa_lead_contacted", "sa_job_won", "sa_payment_received"].map(|name| json!({"kind": "EventsNode", "event": name, "name": name}));
		let mut source = json!({
			"kind": "FunnelsQuery",
			"series": series,
			"dateRange": {"date_from": from.to_string(), "date_to": to.to_string()},
			"funnelsFilter": {"funnelVizType": "steps", "funnelWindowInterval": 90, "funnelWindowIntervalUnit": "day"}, // a payment lands weeks after the visit
		});
		match brand {
			Some(b) =>
				source["properties"] = json!({"type": "AND", "values": [{"type": "AND", "values": [
					{"key": "brand_id", "value": [b.as_str()], "operator": "exact", "type": "event"},
				]}]}),
			None => source["breakdownFilter"] = json!({"breakdown": "brand_id", "breakdown_type": "event"}),
		}
		self.insight_url(&json!({"kind": "InsightVizNode", "source": source}))
	}
}

/// JavaScript's `encodeURIComponent`.
fn uri_component(s: &str) -> String {
	let mut out = String::with_capacity(s.len() * 3);
	for b in s.bytes() {
		if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
			out.push(char::from(b));
		} else {
			out.push_str(&format!("%{b:02X}"));
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn encodes_as_javascript_does() {
		assert_eq!(uri_component(r#"{"a": "b c/é"}"#), "%7B%22a%22%3A%20%22b%20c%2F%C3%A9%22%7D");
		assert_eq!(uri_component("A-z_0.!~*'()"), "A-z_0.!~*'()");
	}
}
