//! A brand's experiments over HTTP (`panel::experiment`): the screens' routes under `/api/v1`
//! behind the conductor's assertion, and the sites' read under `/api/internal`, which has none.
//!
//! ```text
//! GET /api/v1/experiments?brand                    {experiments: [Item]}: one brand's, or every brand's
//! PUT /api/v1/experiments/{brand}/{key}   admin    {enabled?, weights?, holdout?} → 200 Item; a field
//!                                                  absent is left, null put back to the declaration;
//!                                                  400 {error} invalid; 404 unknown or retired
//! GET /api/internal/brands/{brand}/experiments     {experiments: {key: {enabled?, weights?, holdout?}}}:
//!                                                  the overrides a landing takes — never a 404 or a 5xx
//! ```
//!
//! Item: `{brand, key, variants, declared: {weights, enabled, holdout, summary, declared_at},
//! override: null | {weights, enabled, holdout, changed_by (the admin's email, else their id),
//! changed_at}, effective: {weights,
//! enabled, holdout}, weights_changed_at, retired, posthog_url}`.

use std::time::Duration;

use axum::{
	Extension, Json,
	extract::{Path, Query, State},
	http::Method,
};
use jiff::Timestamp;
use panel::{
	Panel,
	experiment::{ExperimentError, ExperimentView, live_json},
};
use panel_core::{
	Invalid,
	experiment::{Patch, is_key},
	ids::BrandId,
};
use sa_auth::Experiments;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
	api::ApiError,
	gate::Caller,
	http::{ApiRoute, Section},
	places::editor,
};

/// How long a site's read may take before it is answered with no overrides.
const LIVE_WITHIN: Duration = Duration::from_millis(2500);

pub(crate) fn routes() -> Vec<ApiRoute> {
	let analysis = Some(Section::Analysis);
	vec![
		ApiRoute::new(Method::GET, "/experiments", analysis, list),
		ApiRoute::new(Method::PUT, "/experiments/{brand}/{key}", analysis, configure),
	]
}

fn ts(t: Option<Timestamp>) -> Value {
	json!(t.map(|t| t.to_string()))
}

fn item(v: &ExperimentView) -> Value {
	let s = &v.state;
	let effective = s.effective();
	json!({
		"brand": v.brand.as_str(),
		"key": s.declared.key,
		"variants": s.declared.variants,
		"declared": {
			"weights": s.declared.weights,
			"enabled": s.declared.enabled,
			"holdout": s.declared.holdout,
			"summary": s.declared.summary,
			"declared_at": s.declared_at.to_string(),
		},
		"override": s.over.as_ref().map(|o| json!({
			"weights": o.weights,
			"enabled": o.enabled,
			"holdout": o.holdout,
			"changed_by": o.changed_by,
			"changed_at": o.changed_at.to_string(),
		})),
		"effective": {"weights": effective.weights, "enabled": effective.enabled, "holdout": effective.holdout},
		"weights_changed_at": ts(s.weights_changed_at),
		"retired": s.retired,
		"posthog_url": v.posthog_url,
	})
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListQuery {
	brand: Option<String>,
}

async fn list(State(panel): State<Panel>, q: Result<Query<ListQuery>, axum::extract::rejection::QueryRejection>) -> Result<Json<Value>, ApiError> {
	let Query(q) = q.map_err(|e| ApiError::BadRequest(e.body_text()))?;
	let brand = q.brand.as_deref().map(BrandId::parse).transpose()?;
	let items: Vec<Value> = panel.experiments(brand.as_ref()).await?.iter().map(item).collect();
	Ok(Json(json!({ "experiments": items })))
}

/// The body of a change: each field absent (left), null (put back) or a value (set).
fn patch(key: &str, body: &Value) -> Result<Patch, Invalid> {
	let Value::Object(fields) = body else {
		return Err(Invalid::new("the body is {\"enabled\"?, \"weights\"?, \"holdout\"?}"));
	};
	if let Some(other) = fields.keys().find(|k| !["enabled", "weights", "holdout"].contains(&k.as_str())) {
		return Err(Invalid::new(format!("unknown field {other:?}: the body is {{\"enabled\"?, \"weights\"?, \"holdout\"?}}")));
	}
	let mut reset = Vec::new();
	let mut field = |name: &str, fields: &Map<String, Value>| match fields.get(name) {
		Some(Value::Null) => {
			reset.push(name.to_owned());
			None
		}
		other => other.cloned(),
	};
	let enabled = field("enabled", fields);
	let weights = field("weights", fields);
	let holdout = field("holdout", fields);
	let enabled = enabled.map(|v| v.as_bool().ok_or_else(|| Invalid::new("enabled is true, false or null"))).transpose()?;
	let weights = match weights {
		None => Vec::new(),
		Some(Value::Array(ws)) => ws.iter().map(|w| w.as_f64().ok_or_else(|| Invalid::new("weights are numbers"))).collect::<Result<_, _>>()?,
		Some(_) => return Err(Invalid::new("weights are an array of numbers, or null")),
	};
	if fields.get("weights").is_some_and(|w| w.as_array().is_some_and(Vec::is_empty)) {
		return Err(Invalid::new("weights are one per variant"));
	}
	let holdout = holdout.map(|v| v.as_f64().ok_or_else(|| Invalid::new("holdout is a number in [0, 1), or null"))).transpose()?;
	Patch::parse(key, enabled, weights, holdout, &reset).map_err(|e| Invalid::new(e.0.trim_start_matches("properties.").to_owned()))
}

async fn configure(State(panel): State<Panel>, Extension(caller): Extension<Caller>, Path((brand, key)): Path<(String, String)>, body: axum::body::Bytes) -> Result<Json<Value>, ApiError> {
	if !caller.permissions.may(Experiments::Edit) {
		return Err(ApiError::Forbidden);
	}
	let brand = BrandId::parse(&brand).map_err(|_| ApiError::NotFound)?;
	if !is_key(&key) {
		return Err(ApiError::NotFound);
	}
	let body: Value = serde_json::from_slice(&body).map_err(|_| ApiError::BadRequest("the body is not JSON".into()))?;
	let patch = patch(&key, &body)?;
	match panel.configure_experiment(&editor(&caller), &brand, patch, Timestamp::now()).await {
		Ok(v) => Ok(Json(item(&v))),
		Err(ExperimentError::NotFound) => Err(ApiError::NotFound),
		Err(ExperimentError::Invalid(e)) => Err(ApiError::BadRequest(e.0)),
		Err(ExperimentError::Internal(e)) => Err(ApiError::Internal(e)),
	}
}

/// What a site gets: its brand's overrides, or none — and none too for a brand id that cannot
/// be one, a store that fails or does not answer in time. The site then runs its code's config,
/// which is what it would do on an error.
pub(crate) async fn live(State(panel): State<Panel>, Path(brand): Path<String>) -> Json<Value> {
	let none = || Json(json!({ "experiments": {} }));
	let Ok(brand) = BrandId::parse(&brand) else {
		return none();
	};
	match tokio::time::timeout(LIVE_WITHIN, panel.live_experiments(&brand)).await {
		Ok(Ok(overrides)) => {
			let experiments: Map<String, Value> = overrides.iter().map(|(k, o)| (k.clone(), live_json(o))).collect();
			Json(json!({ "experiments": experiments }))
		}
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

	use super::*;

	#[test]
	fn absent_leaves_null_puts_back() {
		let p = patch("hero", &json!({"enabled": false, "weights": null, "holdout": 0})).unwrap();
		assert_eq!((p.enabled, p.weights, p.holdout), (Field::Set(false), Field::Reset, Field::Set(0.0)));
		assert!(patch("hero", &json!({})).unwrap().is_empty());
		for (bad, want) in [
			(json!({"variants": ["a"]}), "unknown field"),
			(json!({"enabled": "no"}), "enabled"),
			(json!({"weights": [1, "x"]}), "weights"),
			(json!({"weights": []}), "weights"),
			(json!({"holdout": 1}), "holdout is not in [0, 1)"),
			(json!([]), "the body"),
		] {
			let e = patch("hero", &bad).unwrap_err();
			assert!(e.0.contains(want), "{bad}: {e}");
		}
	}
}
