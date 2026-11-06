//! A brand's pricing over HTTP, end to end on the real router: the editor's routes behind the
//! conductor's assertion, the preview held to the pricing vectors' cases, and the sites' read under
//! `/api/internal`.

use axum::http::{Method, StatusCode};
use panel::testing::{Conductor, TestDb, panel};
use panel_server::testing::{As, Client, alias, app, send};
use serde_json::{Value, json};
use uuid::Uuid;

const ITEM: &str = "/api/v1/pricing/vifnet";
const LIVE: &str = "/api/internal/brands/vifnet/pricing?locale=fr";

fn fixture(path: &str) -> Value {
	let path = format!("{}/../../contracts/vectors/pricing/{path}", env!("CARGO_MANIFEST_DIR"));
	serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

/// What an `updated_by` reads for the user `sub`.
fn email(sub: Uuid) -> String {
	format!("{}@example.com", sub.simple())
}

#[tokio::test]
async fn an_operator_reads_and_previews_but_changes_nothing() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor);
	let b = Client {
		app: &app,
		conductor: &conductor,
		who: As::user(alias("sa:operator")),
	};

	let (status, item) = b.get(ITEM).await;
	assert_eq!(status, StatusCode::OK, "{item}");
	assert_eq!(
		item,
		json!({"brand_id": "vifnet", "locales": ["fr", "en"], "model": null, "updated_at": null, "updated_by": null})
	);
	assert_eq!(b.get("/api/v1/pricing").await, (StatusCode::OK, json!({"items": []})), "no brand known yet");
	assert_eq!(b.get(&format!("{ITEM}/changes")).await, (StatusCode::OK, json!({"changes": []})));

	let preview = json!({"model": fixture("valid/cleaning.json"), "need": "windows", "inputs": {}});
	assert_eq!(b.post(&format!("{ITEM}/preview"), preview).await, (StatusCode::OK, json!({"cents": 8900})));

	let put = json!({"model": fixture("valid/cleaning.json"), "expected_updated_at": null});
	assert_eq!(b.put(ITEM, put).await.0, StatusCode::FORBIDDEN);
	assert_eq!(b.delete(ITEM, Some(json!({"expected_updated_at": null}))).await.0, StatusCode::FORBIDDEN);
	assert_eq!(b.get("/api/v1/pricing/Vifnet").await.0, StatusCode::BAD_REQUEST, "not a brand id");

	let nobody = Client { who: As::Nobody, ..b };
	assert_eq!(nobody.get(ITEM).await, (StatusCode::UNAUTHORIZED, json!({"error": "unauthenticated"})));
}

#[tokio::test]
async fn an_admin_saves_a_model_and_the_sites_read_it() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor);
	let sub = Uuid::now_v7();
	let b = Client {
		app: &app,
		conductor: &conductor,
		who: As::User(sub, alias("sa:admin")),
	};
	let by = email(sub);
	let site = Client { who: As::Nobody, ..b };
	let cleaning = fixture("valid/cleaning.json");

	assert_eq!(site.get(LIVE).await, (StatusCode::OK, json!({})), "never set: {{}}, never a 404");
	for odd in ["/api/internal/brands/Vifnet/pricing", "/api/internal/brands/nobody-knows/pricing"] {
		assert_eq!(site.get(odd).await, (StatusCode::OK, json!({})), "{odd}");
	}

	let put = json!({"model": cleaning, "expected_updated_at": null});

	let mut invalid = cleaning.clone();
	invalid["needs"]["standard"]["inputs"][2] = json!("pets");
	let (status, body) = b.put(ITEM, json!({"model": invalid, "expected_updated_at": null})).await;
	assert_eq!(
		(status, body),
		(
			StatusCode::UNPROCESSABLE_ENTITY,
			json!({"error": "no input \"pets\"", "code": "noInput", "path": "needs.standard.inputs[2]"})
		)
	);
	let mut unlabelled = cleaning.clone();
	unlabelled["inputs"][1]["options"][0]["labels"] = json!({"fr": "Studio"});
	let (status, body) = b.put(ITEM, json!({"model": unlabelled, "expected_updated_at": null})).await;
	assert_eq!(
		(status, body),
		(
			StatusCode::UNPROCESSABLE_ENTITY,
			json!({"error": "no \"en\" label", "code": "labelMissing", "path": "inputs[1].options[0].labels.en"})
		)
	);
	let (status, body) = b.put(ITEM, json!({"model": [], "expected_updated_at": null})).await;
	assert_eq!((status, body), (StatusCode::UNPROCESSABLE_ENTITY, json!({"error": "an object", "code": "object", "path": ""})));

	let (status, saved) = b.put(ITEM, put.clone()).await;
	assert_eq!(status, StatusCode::OK, "{saved}");
	assert_eq!(saved["model"], cleaning);
	assert_eq!(saved["updated_by"], by.as_str());
	assert_eq!(saved["locales"], json!(["fr", "en"]));
	let updated_at = saved["updated_at"].clone();
	assert!(updated_at.is_string());
	assert_eq!(site.get(LIVE).await, (StatusCode::OK, cleaning.clone()), "the whole model, every locale's labels");
	assert_eq!(b.get(ITEM).await, (StatusCode::OK, saved.clone()));
	assert_eq!(b.get("/api/v1/pricing").await.1, json!({"items": [saved.clone()]}));

	let (status, stale) = b.put(ITEM, put).await;
	assert_eq!((status, stale), (StatusCode::CONFLICT, json!({"error": "stale", "current": saved})));

	let mut cheaper = cleaning.clone();
	cheaper["minimumCents"] = json!(3900);
	let (status, next) = b.put(ITEM, json!({"model": cheaper, "expected_updated_at": updated_at})).await;
	assert_eq!(status, StatusCode::OK, "{next}");
	let (status, _) = b.delete(ITEM, Some(json!({"expected_updated_at": updated_at}))).await;
	assert_eq!(status, StatusCode::CONFLICT, "a removal names the version it removes");
	assert_eq!(b.delete(ITEM, Some(json!({}))).await.0, StatusCode::BAD_REQUEST);
	let (status, removed) = b.delete(ITEM, Some(json!({"expected_updated_at": next["updated_at"]}))).await;
	assert_eq!(status, StatusCode::OK, "{removed}");
	assert_eq!((removed["model"].clone(), removed["updated_by"].clone()), (Value::Null, json!(by)));
	assert_eq!(site.get(LIVE).await, (StatusCode::OK, json!({})), "back to the baked model");

	let (status, history) = b.get(&format!("{ITEM}/changes")).await;
	assert_eq!(status, StatusCode::OK);
	let summary: Vec<(&str, &Value, &Value, &str)> = history["changes"]
		.as_array()
		.unwrap()
		.iter()
		.map(|c| (c["kind"].as_str().unwrap(), &c["valid_from"], &c["needs"], c["by"].as_str().unwrap()))
		.collect();
	assert_eq!(
		summary,
		[
			("remove", &Value::Null, &Value::Null, by.as_str()),
			("set", &json!("2026-10-01"), &json!(2), by.as_str()),
			("set", &json!("2026-10-01"), &json!(2), by.as_str()),
		]
	);
}

/// The editor's preview is what the site shows: the vectors' cases, priced over HTTP, to the cent.
#[tokio::test]
async fn the_preview_prices_the_vectors_cases() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor);
	let b = Client {
		app: &app,
		conductor: &conductor,
		who: As::user(alias("sa:operator")),
	};
	let Value::Array(cases) = fixture("cases.json") else { panic!("a list") };
	let cleaning: Vec<&Value> = cases.iter().filter(|c| c["name"].as_str().unwrap().starts_with("cleaning:")).collect();
	assert!(cleaning.len() >= 3);
	for case in cleaning {
		let body = json!({"model": case["model"], "need": case["need"], "inputs": case["inputs"]});
		let (status, answer) = b.post("/api/v1/pricing/vifnet/preview", body).await;
		assert_eq!((status, &answer["cents"]), (StatusCode::OK, &case["cents"]), "{}", case["name"]);
	}
	let mut draft = cases[0]["model"].clone();
	draft["roundToCents"] = json!(0);
	let (status, answer) = b.post("/api/v1/pricing/vifnet/preview", json!({"model": draft, "need": "standard", "inputs": {}})).await;
	assert_eq!(
		(status, answer),
		(
			StatusCode::UNPROCESSABLE_ENTITY,
			json!({"error": "an integer from 1 to 100000000", "code": "int", "path": "roundToCents"})
		)
	);
}

/// A store that fails is a site that keeps its baked model: `{}`, never a 5xx.
#[tokio::test]
async fn the_sites_read_survives_a_failing_store() {
	let db = TestDb::create().await;
	let panel = panel(&db).await;
	let pool = db.pool().await;
	sqlx::query("DROP TABLE pricing_changes").execute(&pool).await.unwrap();
	sqlx::query("DROP TABLE pricing").execute(&pool).await.unwrap();
	let conductor = Conductor::new("k1");
	let app = app(panel, &conductor);
	assert_eq!(send(&app, &conductor, &As::Nobody, Method::GET, LIVE, None, &[]).await, (StatusCode::OK, json!({})));
}
