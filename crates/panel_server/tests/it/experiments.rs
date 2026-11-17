//! A brand's experiments over HTTP, end to end on the real router: a landing's declaration
//! ingested through a site key, the screens' list, an admin's change (operators refused), the
//! sites' read under `/api/internal`, a retired experiment, and the rebuild landing on the same.

use axum::http::StatusCode;
use jiff::{SignedDuration, Timestamp};
use panel::{
	Panel,
	posthog::PosthogProject,
	testing::{Conductor, TestDb, batch, event, grant, panel},
};
use panel_server::testing::{As, Client, alias, app};
use sa_auth::SourceKind;
use serde_json::{Value, json};
use uuid::Uuid;

const LIVE: &str = "/api/internal/brands/aquafix/experiments";

/// The landing of aquafix declaring `experiments` at `at`.
async fn declare(panel: &Panel, experiments: Value, at: Timestamp) {
	let e = event("experiments.declared", at, "site", json!({"brandId": "aquafix"}), json!({ "experiments": experiments }));
	let got = panel
		.ingest(&grant("aquafix-site", SourceKind::Site, &["aquafix"]), &batch("aquafix-site", &[e]), at)
		.await
		.unwrap();
	assert_eq!(got[0].outcome, panel::Outcome::Accepted { unregistered: false }, "{got:?}");
}

#[tokio::test]
async fn a_declaration_an_override_and_what_the_site_is_told() {
	let db = TestDb::create().await;
	let project = PosthogProject {
		app_host: "https://us.posthog.com".into(),
		project_id: "614067".into(),
	};
	let panel = panel(&db).await.with_posthog_project(Some(project));
	let conductor = Conductor::new("k1");
	let app = app(panel.clone(), &conductor);
	let sub = Uuid::now_v7();
	let b = Client {
		app: &app,
		conductor: &conductor,
		who: As::User(sub, alias("sa:admin")),
	};
	let site = Client { who: As::Nobody, ..b };

	assert_eq!(site.get(LIVE).await, (StatusCode::OK, json!({"experiments": {}})), "nothing declared yet");
	assert_eq!(site.get("/api/internal/brands/Not-A-Brand/experiments").await, (StatusCode::OK, json!({"experiments": {}})));

	let t0 = Timestamp::now() - SignedDuration::from_mins(10);
	declare(
		&panel,
		json!([
			{"key": "lead_layout", "variants": ["a", "b"], "weights": [1, 1], "enabled": true, "summary": "One step converts better"},
			{"key": "hero", "variants": ["a", "b", "c"], "weights": [1, 1, 1], "enabled": true, "holdout": 0.1}
		]),
		t0,
	)
	.await;
	let (status, list) = b.get("/api/v1/experiments?brand=aquafix").await;
	assert_eq!(status, StatusCode::OK, "{list}");
	let item = |list: &Value, key: &str| list["experiments"].as_array().unwrap().iter().find(|e| e["key"] == key).unwrap().clone();
	let layout = item(&list, "lead_layout");
	assert_eq!(layout["declared"]["summary"], "One step converts better");
	assert_eq!(layout["override"], Value::Null);
	assert_eq!(layout["effective"], json!({"weights": [1.0, 1.0], "enabled": true, "holdout": null}));
	assert_eq!((layout["retired"].clone(), layout["weights_changed_at"].clone()), (json!(false), Value::Null));
	let url = layout["posthog_url"].as_str().unwrap();
	assert!(
		url.starts_with("https://us.posthog.com/project/614067/insights/new#q=%7B%22kind%22%3A%22InsightVizNode%22"),
		"{url}"
	);
	assert_eq!(b.get("/api/v1/experiments").await.1["experiments"].as_array().unwrap().len(), 2, "every brand without ?brand");

	// The funnel opens PostHog's, from the visit, on the same brand and days.
	let funnel = b.get("/api/v1/funnel?brand=aquafix&from=2026-09-01&to=2026-09-30").await.1;
	let url = funnel["posthog_url"].as_str().unwrap();
	assert!(url.starts_with("https://us.posthog.com/project/614067/insights/new#q="), "{url}");
	for needle in ["location_page_view", "sa_payment_received", "%22aquafix%22", "2026-09-01", "2026-09-30"] {
		assert!(url.contains(needle), "{needle} in {url}");
	}
	let every = b.get("/api/v1/funnel").await.1;
	assert!(every["posthog_url"].as_str().unwrap().contains("breakdownFilter"), "every brand's, side by side");

	// An admin's change: weights and the kill switch; null puts a field back, 0 is a holdout.
	let path = "/api/v1/experiments/aquafix/hero";
	let (status, hero) = b.put(path, json!({"weights": [2, 1, 1], "holdout": 0})).await;
	assert_eq!(status, StatusCode::OK, "{hero}");
	assert_eq!(hero["effective"], json!({"weights": [2.0, 1.0, 1.0], "enabled": true, "holdout": 0.0}));
	assert_eq!(
		hero["override"]["changed_by"],
		format!("{}@example.com", sub.simple()).as_str(),
		"named as a place's or a price list's history names them"
	);
	let source: String = sqlx::query_scalar("SELECT source_id FROM events WHERE type = 'experiment.configured'")
		.fetch_one(panel.store().pool())
		.await
		.unwrap();
	assert_eq!(source, sub.to_string(), "the source is the admin's id");
	assert!(hero["weights_changed_at"].is_string());
	let (_, hero) = b.put(path, json!({"holdout": null, "enabled": false})).await;
	assert_eq!(hero["effective"], json!({"weights": [2.0, 1.0, 1.0], "enabled": false, "holdout": 0.1}));
	assert_eq!(site.get(LIVE).await.1, json!({"experiments": {"hero": {"enabled": false, "weights": [2.0, 1.0, 1.0]}}}));

	for (body, status) in [
		(json!({"weights": [1, 1]}), StatusCode::BAD_REQUEST),
		(json!({"weights": [0, 0, 0]}), StatusCode::BAD_REQUEST),
		(json!({"holdout": 1}), StatusCode::BAD_REQUEST),
		(json!({"variants": ["a"]}), StatusCode::BAD_REQUEST),
	] {
		let (got, err) = b.put(path, body.clone()).await;
		assert_eq!(got, status, "{body}: {err}");
		assert!(err["error"].is_string(), "{err}");
	}
	assert_eq!(b.put("/api/v1/experiments/aquafix/nope", json!({"enabled": false})).await.0, StatusCode::NOT_FOUND);
	assert_eq!(b.put("/api/v1/experiments/vifnet/hero", json!({"enabled": false})).await.0, StatusCode::NOT_FOUND);

	// The landing drops `hero`: retired, shown, not changed, and nothing for the site.
	declare(
		&panel,
		json!([{"key": "lead_layout", "variants": ["a", "b"], "weights": [1, 1], "enabled": true}]),
		t0 + SignedDuration::from_mins(1),
	)
	.await;
	let list = b.get("/api/v1/experiments?brand=aquafix").await.1;
	assert_eq!(item(&list, "hero")["retired"], true);
	assert_eq!(b.put(path, json!({"enabled": true})).await.0, StatusCode::NOT_FOUND);
	assert_eq!(site.get(LIVE).await.1, json!({"experiments": {}}));

	let before = b.get("/api/v1/experiments").await.1;
	panel.rebuild_projections().await.unwrap();
	assert_eq!(b.get("/api/v1/experiments").await.1, before, "the rebuild lands on the same");

	// An operator reads, and changes nothing.
	let o = Client {
		who: As::user(alias("sa:operator")),
		..b
	};
	assert_eq!(o.get("/api/v1/experiments").await.0, StatusCode::OK);
	let refused = o.put("/api/v1/experiments/aquafix/lead_layout", json!({"enabled": false})).await;
	assert_eq!(refused.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_override_that_no_longer_fits_is_not_sent() {
	let db = TestDb::create().await;
	let panel = panel(&db).await;
	let conductor = Conductor::new("k1");
	let app = app(panel.clone(), &conductor);
	let b = Client {
		app: &app,
		conductor: &conductor,
		who: As::user(alias("sa:admin")),
	};
	let site = Client { who: As::Nobody, ..b };
	let t0 = Timestamp::now() - SignedDuration::from_mins(10);
	declare(&panel, json!([{"key": "hero", "variants": ["a", "b"], "weights": [1, 1], "enabled": true}]), t0).await;
	let (status, _) = b.put("/api/v1/experiments/aquafix/hero", json!({"weights": [3, 1]})).await;
	assert_eq!(status, StatusCode::OK);
	declare(
		&panel,
		json!([{"key": "hero", "variants": ["a", "b", "c"], "weights": [1, 1, 1], "enabled": true}]),
		t0 + SignedDuration::from_mins(1),
	)
	.await;
	assert_eq!(site.get(LIVE).await.1, json!({"experiments": {}}), "two weights for three variants: the site keeps its code's");
	let hero = &b.get("/api/v1/experiments").await.1["experiments"][0];
	assert_eq!(hero["override"]["weights"], json!([3.0, 1.0]), "kept as set");
	assert_eq!(hero["effective"]["weights"], json!([1.0, 1.0, 1.0]));
	assert_eq!(hero["posthog_url"], Value::Null, "no project configured");
	assert_eq!(b.get("/api/v1/funnel").await.1["posthog_url"], Value::Null);
}
