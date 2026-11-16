//! A brand's experiments over HTTP (`panel::experiment`): the screens' routes under `/api/v1`
//! behind the conductor's assertion, and the sites' read under `/api/internal`, which has none.
//!
//! ```text
//! GET /api/internal/brands/{brand}/experiments     {experiments: {key: {enabled?, weights?, holdout?}}}:
//!                                                  the overrides a landing takes — never a 404 or a 5xx
//! ```

use std::{sync::Arc, time::Duration};

use axum::{
	Extension, Json,
	extract::{Path, State},
	http::Method,
};
use ev_lib::api::JsonSchema;
use jiff::Timestamp;
use panel::{
	Panel,
	experiment::{ExperimentError, ExperimentView, LiveExperiments},
};
use panel_core::experiment::{ExperimentKey, Patch, Variant};
use sa_auth::Experiments;
use sa_core::{Invalid, ids::BrandId};
use serde::{Deserialize, Serialize};

use crate::{
	api::{ApiError, ApiResult, Body, Ids, Params, present},
	conductor::{InternalEndpoint, InternalReads},
	gate::Caller,
	http::{ApiRoute, Gate, Section},
	places::editor,
};

/// How long a site's read may take before it is answered with no overrides.
const LIVE_WITHIN: Duration = Duration::from_millis(2500);

pub(crate) fn routes() -> Vec<ApiRoute> {
	let analysis = Gate::section(Section::Analysis);
	vec![
		ApiRoute::new(Method::GET, "/experiments", analysis, list_experiments),
		ApiRoute::new(Method::PUT, "/experiments/{brand}/{key}", analysis.may(Experiments::Edit), configure_experiment),
	]
}

/// The weights, the switch and the holdout of an experiment.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ExperimentSplit")]
struct SplitDto {
	/// One per variant.
	weights: Vec<f64>,
	enabled: bool,
	/// The share of visitors kept out of it, in [0, 1).
	holdout: Option<f64>,
}

/// As the landing's latest declaration said it.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ExperimentDeclared")]
struct DeclaredDto {
	#[serde(flatten)]
	split: SplitDto,
	/// The hypothesis, in a line.
	summary: Option<String>,
	declared_at: Timestamp,
}

/// What an admin set over the declaration; a field null is the declaration's.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "ExperimentOverride")]
struct OverrideDto {
	weights: Option<Vec<f64>>,
	enabled: Option<bool>,
	holdout: Option<f64>,
	/// The admin's email, else their id.
	changed_by: String,
	changed_at: Timestamp,
}

/// One experiment of a brand: declared by its landing, overridden by an admin.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Experiment")]
struct ExperimentDto {
	brand: BrandId,
	key: ExperimentKey,
	/// The first is the control.
	variants: Vec<Variant>,
	declared: DeclaredDto,
	/// Null when nothing is set, or everything was put back.
	r#override: Option<OverrideDto>,
	/// What the landing runs: the declaration with the override laid over it.
	effective: SplitDto,
	weights_changed_at: Option<Timestamp>,
	/// Its landing no longer declares it.
	retired: bool,
	/// Its funnel in PostHog; null without a project configured.
	posthog_url: Option<String>,
}

impl From<ExperimentView> for ExperimentDto {
	fn from(v: ExperimentView) -> Self {
		let s = v.state;
		let effective = s.effective();
		Self {
			brand: v.brand,
			declared: DeclaredDto {
				split: SplitDto {
					weights: s.declared.weights,
					enabled: s.declared.enabled,
					holdout: s.declared.holdout,
				},
				summary: s.declared.summary,
				declared_at: s.declared_at,
			},
			key: s.declared.key,
			variants: s.declared.variants,
			r#override: s.over.map(|o| OverrideDto {
				weights: o.weights,
				enabled: o.enabled,
				holdout: o.holdout,
				changed_by: o.changed_by,
				changed_at: o.changed_at,
			}),
			effective: SplitDto {
				weights: effective.weights,
				enabled: effective.enabled,
				holdout: effective.holdout,
			},
			weights_changed_at: s.weights_changed_at,
			retired: s.retired,
			posthog_url: v.posthog_url,
		}
	}
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListQuery {
	/// Absent: every brand's.
	brand: Option<BrandId>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Experiments")]
struct ExperimentsDto {
	experiments: Vec<ExperimentDto>,
}

/// A brand's experiments, or every brand's: declared, overridden, and in effect.
async fn list_experiments(State(panel): State<Panel>, Params(q): Params<ListQuery>) -> ApiResult<Json<ExperimentsDto>> {
	Ok(Json(ExperimentsDto {
		experiments: panel.experiments(q.brand.as_ref()).await?.into_iter().map(ExperimentDto::from).collect(),
	}))
}

/// `{brand}/{key}`.
#[derive(Deserialize, JsonSchema)]
struct ExperimentPath {
	brand: BrandId,
	key: ExperimentKey,
}

/// A change: each field absent (left), null (put back to the declaration) or a value (set).
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ExperimentChange")]
struct ConfigureBody {
	#[serde(default, deserialize_with = "present")]
	#[schemars(with = "Option<bool>")]
	enabled: Option<Option<bool>>,
	/// One per variant, each ≥ 0, their sum > 0.
	#[serde(default, deserialize_with = "present")]
	#[schemars(with = "Option<Vec<f64>>")]
	weights: Option<Option<Vec<f64>>>,
	/// In [0, 1).
	#[serde(default, deserialize_with = "present")]
	#[schemars(with = "Option<f64>")]
	holdout: Option<Option<f64>>,
}

fn patch(key: &str, b: ConfigureBody) -> Result<Patch, Invalid> {
	if b.weights.as_ref().is_some_and(|w| w.as_ref().is_some_and(Vec::is_empty)) {
		return Err(Invalid::new("weights are one per variant"));
	}
	let reset: Vec<String> = [
		("enabled", matches!(b.enabled, Some(None))),
		("weights", matches!(b.weights, Some(None))),
		("holdout", matches!(b.holdout, Some(None))),
	]
	.into_iter()
	.filter(|(_, reset)| *reset)
	.map(|(name, _)| name.to_owned())
	.collect();
	Patch::parse(key, b.enabled.flatten(), b.weights.flatten().unwrap_or_default(), b.holdout.flatten(), &reset).map_err(|e| Invalid::new(e.0.trim_start_matches("properties.").to_owned()))
}

/// Overrides how a landing runs one of its experiments: switched on or off, its weights, its
/// holdout (`experiment.configured`).
///
/// 400 for a change the declaration cannot take (weights against its variants); 404 for an
/// experiment unknown or retired, or a key that cannot be one. A change that changes nothing
/// journals nothing.
async fn configure_experiment(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	Ids(ExperimentPath { brand, key }): Ids<ExperimentPath>,
	Body(b): Body<ConfigureBody>,
) -> ApiResult<Json<ExperimentDto>> {
	let patch = patch(key.as_str(), b)?;
	match panel.configure_experiment(&editor(&caller), &brand, patch, Timestamp::now()).await {
		Ok(v) => Ok(Json(v.into())),
		Err(ExperimentError::NotFound) => Err(ApiError::NotFound),
		Err(ExperimentError::Invalid(e)) => Err(ApiError::BadRequest(e.0)),
		Err(ExperimentError::Internal(e)) => Err(ApiError::Internal(e)),
	}
}

/// What a site gets: its brand's overrides, or none — and none too for a brand id that cannot
/// be one, a store that fails or does not answer in time. The site then runs its code's config,
/// which is what it would do on an error.
pub(crate) async fn live(State(panel): State<Panel>, Extension(reads): Extension<Arc<InternalReads>>, Path(brand): Path<String>) -> Json<LiveExperiments> {
	let none = || Json(LiveExperiments::default());
	let Ok(brand) = BrandId::parse(&brand) else {
		return none();
	};
	reads.record(&brand, InternalEndpoint::Experiments);
	match tokio::time::timeout(LIVE_WITHIN, panel.live_experiments(&brand)).await {
		Ok(Ok(live)) => Json(live),
		Ok(Err(e)) => {
			crate::report(&e, "serving a brand's experiments to a site");
			none()
		}
		Err(_) => {
			tracing::warn!(%brand, "serving a brand's experiments to a site took too long: answered none");
			none()
		}
	}
}

#[cfg(test)]
mod tests {
	use panel_core::experiment::Field;
	use serde_json::{Value, json};

	use super::*;

	fn body(v: Value) -> Result<Patch, String> {
		let b: ConfigureBody = serde_json::from_value(v).map_err(|e| format!("the body: {e}"))?;
		patch("hero", b).map_err(|e| e.0)
	}

	#[test]
	fn absent_leaves_null_puts_back() {
		let p = body(json!({"enabled": false, "weights": null, "holdout": 0})).unwrap();
		assert_eq!((p.enabled, p.weights, p.holdout), (Field::Set(false), Field::Reset, Field::Set(0.0)));
		assert!(body(json!({})).unwrap().is_empty());
		for (bad, want) in [
			(json!({"variants": ["a"]}), "unknown field"),
			(json!({"enabled": "no"}), "invalid type"),
			(json!({"weights": [1, "x"]}), "invalid type"),
			(json!({"weights": []}), "weights"),
			(json!({"holdout": 1}), "holdout is not in [0, 1)"),
			(json!(7), "the body"),
		] {
			let e = body(bad.clone()).unwrap_err();
			assert!(e.contains(want), "{bad}: {e}");
		}
	}
}
