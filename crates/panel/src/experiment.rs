//! A brand's experiments as configuration (`panel_core::experiment`): what the screens list,
//! what an admin changes (journaled as `experiment.configured`, through the registry like any
//! operator action), and what a landing is answered.
//!
//! The statistics are PostHog's: each experiment carries a link to its funnel there
//! ([`posthog_url`]) when the project is known.

use std::collections::BTreeMap;

use eyre::WrapErr;
use jiff::Timestamp;
use panel_contracts::SCHEMA;
use panel_core::{
	Invalid,
	experiment::{Field, Patch, State, label},
	ids::BrandId,
	place::Editor,
};
use serde::Serialize;
use serde_json::json;
use ts_rs::TS;

use crate::{Panel, posthog::PosthogProject, store::experiments as stored};

/// One experiment as the screens show it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentView {
	pub brand: BrandId,
	pub state: State,
	/// Its funnel in PostHog; `None` without a project configured.
	pub posthog_url: Option<String>,
}

/// Why an admin's change was not made.
#[derive(Debug, thiserror::Error)]
pub enum ExperimentError {
	/// No such experiment of the brand, or one its landing no longer declares.
	#[error("not found")]
	NotFound,
	#[error("{0}")]
	Invalid(Invalid),
	#[error(transparent)]
	Internal(#[from] eyre::Report),
}

/// The fields of an override a landing takes, as it is sent them: only those set, each valid
/// against the experiment's current declaration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
#[ts(optional_fields)]
pub struct LiveOverride {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub enabled: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub weights: Option<Vec<f64>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub holdout: Option<f64>,
}

/// What a landing reads of its brand's experiments: the overrides of the current ones, by key.
#[derive(Clone, Debug, Default, PartialEq, Serialize, TS)]
pub struct LiveExperiments {
	pub experiments: BTreeMap<String, LiveOverride>,
}

/// A funnel `experiment_exposed` →
/// `experiment_lead` of this experiment and brand, forced (QA) visits left out, broken down by
/// variant, since the weights last changed or the experiment was first declared.
pub fn posthog_url(project: &PosthogProject, brand: &BrandId, state: &State) -> String {
	let from = state.weights_changed_at.unwrap_or(state.first_declared_at);
	let event = |name: &str| json!({"kind": "EventsNode", "event": name, "name": name});
	let prop = |key: &str, value: &str, operator: &str| json!({"key": key, "value": [value], "operator": operator, "type": "event"});
	let query = json!({
		"kind": "InsightVizNode",
		"source": {
			"kind": "FunnelsQuery",
			"series": [event("experiment_exposed"), event("experiment_lead")],
			"properties": {"type": "AND", "values": [{"type": "AND", "values": [
				prop("experiment", &state.declared.key, "exact"),
				prop("brand_id", brand.as_str(), "exact"),
				prop("forced", "true", "is_not"),
			]}]},
			"breakdownFilter": {"breakdown": "variant", "breakdown_type": "event"},
			"dateRange": {"date_from": from.to_string()},
			"funnelsFilter": {"funnelVizType": "steps"},
		},
	});
	project.insight_url(&query)
}

impl Panel {
	fn view(&self, brand: BrandId, state: State) -> ExperimentView {
		let posthog_url = self.posthog.as_ref().map(|p| posthog_url(p, &brand, &state));
		ExperimentView { brand, state, posthog_url }
	}

	/// Every experiment of one brand, or of all, by brand then key; retired ones included.
	pub async fn experiments(&self, brand: Option<&BrandId>) -> eyre::Result<Vec<ExperimentView>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		Ok(stored::list(&mut conn, brand).await?.into_iter().map(|(b, s)| self.view(b, s)).collect())
	}

	async fn experiment(&self, brand: &BrandId, key: &str) -> eyre::Result<Option<State>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		Ok(stored::list(&mut conn, Some(brand)).await?.into_iter().map(|(_, s)| s).find(|s| s.declared.key == key))
	}

	/// What a brand's landings lay over their code: per current experiment, the override fields
	/// that fit its declaration. Nothing for a brand the panel does not know.
	pub async fn live_experiments(&self, brand: &BrandId) -> eyre::Result<LiveExperiments> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		let mut out = BTreeMap::new();
		for (_, s) in stored::list(&mut conn, Some(brand)).await? {
			if s.retired {
				continue;
			}
			let (enabled, weights, holdout) = s.valid_override();
			let o = LiveOverride {
				enabled,
				weights: weights.map(<[f64]>::to_vec),
				holdout,
			};
			if o != LiveOverride::default() {
				out.insert(s.declared.key.clone(), o);
			}
		}
		Ok(LiveExperiments { experiments: out })
	}

	/// An admin's change to one experiment: journaled as `experiment.configured` when it changes
	/// what is set — named by the editor's label, from the editor's id (or `cli`) — then
	/// answered as the screens show it.
	pub async fn configure_experiment(&self, by: &Editor, brand: &BrandId, patch: Patch, now: Timestamp) -> Result<ExperimentView, ExperimentError> {
		let name = label(by.label()).map_err(|e| ExperimentError::Invalid(Invalid::new(e.0.trim_start_matches("properties.").to_owned())))?;
		let source_id = by.user_id().map_or_else(|| "cli".to_owned(), |u| u.to_string());
		let current = self.experiment(brand, &patch.key).await?.filter(|s| !s.retired).ok_or(ExperimentError::NotFound)?;
		current.check_patch(&patch).map_err(ExperimentError::Invalid)?;
		if !current.changes(&patch) {
			return Ok(self.view(brand.clone(), current));
		}
		let mut properties = json!({"key": patch.key, "by": name});
		let mut reset = Vec::new();
		match &patch.enabled {
			Field::Keep => {}
			Field::Set(v) => properties["enabled"] = json!(v),
			Field::Reset => reset.push("enabled"),
		}
		match &patch.weights {
			Field::Keep => {}
			Field::Set(v) => properties["weights"] = json!(v),
			Field::Reset => reset.push("weights"),
		}
		match &patch.holdout {
			Field::Keep => {}
			Field::Set(v) => properties["holdout"] = json!(v),
			Field::Reset => reset.push("holdout"),
		}
		if !reset.is_empty() {
			properties["reset"] = json!(reset);
		}
		let raw = json!({
			"id": crate::operator::new_event_id(now).to_string(),
			"schema": SCHEMA,
			"type": "experiment.configured",
			"typeVersion": 1,
			"occurredAt": now.to_string(),
			"source": {"kind": "panel", "id": source_id},
			"subject": {"brandId": brand.as_str()},
			"properties": properties,
		});
		match self.write_own(raw, now).await?.map_err(ExperimentError::Invalid)?.0 {
			crate::Outcome::Accepted { .. } => {}
			crate::Outcome::Duplicate => return Err(ExperimentError::Internal(eyre::eyre!("a fresh event id was taken"))),
			// Deferred is ingest's, for a source's booking ahead of its lead; never the panel's own.
			crate::Outcome::Rejected(e) | crate::Outcome::Deferred(e) => return Err(ExperimentError::Invalid(e)),
		}
		tracing::info!(source_id, %brand, key = patch.key, "experiment configured");
		let state = self.experiment(brand, &patch.key).await?.ok_or_else(|| eyre::eyre!("an experiment configured and gone"))?;
		Ok(self.view(brand.clone(), state))
	}
}
