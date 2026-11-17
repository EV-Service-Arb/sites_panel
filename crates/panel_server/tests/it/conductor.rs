//! What the conductor's hubs read, as they read it: the notes its Telegram sends (and a button's
//! call coming back as the user who pressed), the change feed its live sockets relay, and when
//! each key last sent.

use axum::{
	body::Body,
	http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use jiff::Timestamp;
use panel::{
	operator::{Actor, NewLead},
	testing::{Conductor, TestDb, batch, event, grant, panel},
};
use panel_core::{fact::ManualChannel, ids::LocationId};
use panel_server::testing::{As, alias, app, send};
use sa_auth::SourceKind;
use sa_core::{ids::BrandId, notify::Locale};
use serde_json::{Value, json};
use tower::ServiceExt;

fn site_lead(id: &str, at: Timestamp) -> Value {
	let mut e = event(
		"lead.created",
		at,
		"site",
		json!({"brandId": "aquafix", "locationId": "royat", "leadId": id}),
		json!({"channel": "form"}),
	);
	e["pii"] = json!({"name": "Jean Dupont", "need": "fuite https://evil.example", "phone": "+33612345678"});
	e
}

async fn notes(app: &axum::Router, conductor: &Conductor, after: i64) -> Value {
	let (status, body) = send(
		app,
		conductor,
		&As::Conductor,
		Method::GET,
		&format!("/api/conductor/notifications?after={after}&limit=10"),
		None,
		&[],
	)
	.await;
	assert_eq!(status, StatusCode::OK, "{body}");
	body
}

#[tokio::test]
async fn a_new_lead_is_one_note_with_and_without_its_pii_and_its_buttons_call_back() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let panel = panel(&db).await;
	let app = app(panel.clone(), &conductor);
	let now = Timestamp::now();
	panel
		.ingest(&grant("aquafix-site", SourceKind::Site, &["aquafix"]), &batch("aquafix-site", &[site_lead("L-1", now)]), now)
		.await
		.unwrap();

	assert_eq!(panel.fan_out(now, Locale::Ru).await.unwrap(), 1);
	assert_eq!(panel.fan_out(now, Locale::Ru).await.unwrap(), 0, "told once");
	let body = notes(&app, &conductor, 0).await;
	let note = &body["notes"][0];
	assert_eq!(body["notes"].as_array().unwrap().len(), 1);
	assert_eq!(body["next"], note["seq"]);
	assert_eq!((note["rule"].as_str(), note["skip_user"].as_null()), (Some("new_lead"), Some(())));
	assert_eq!(note["text"], json!({"text": "Новая заявка\naquafix · royat", "entities": []}), "no PII");
	let pii = &note["text_pii"];
	let text = pii["text"].as_str().unwrap();
	assert_eq!(text, "Новая заявка\naquafix · royat\nИмя: Jean Dupont\nНужно: fuite https://evil.example\nТелефон: +33612345678");
	let units: Vec<u16> = text.encode_utf16().collect();
	let spans: Vec<String> = pii["entities"]
		.as_array()
		.unwrap()
		.iter()
		.map(|e| {
			assert_eq!(e["type"], "code");
			let (o, l) = (e["offset"].as_u64().unwrap() as usize, e["length"].as_u64().unwrap() as usize);
			String::from_utf16(&units[o..o + l]).unwrap()
		})
		.collect();
	assert_eq!(spans, ["Jean Dupont", "fuite https://evil.example"], "what the customer typed is code, never a link");
	assert_eq!(
		note["buttons"],
		json!([
			{"id": "take", "label": "Взял", "pressed": "Взял",
			 "call": {"operation": "move_lead", "path": {"brand": "aquafix", "lead": "L-1"}, "body": {"stage": "contacted"}}, "leaves": []},
			{"id": "no_answer", "label": "Не дозвонился", "pressed": "Не дозвонился",
			 "call": {"operation": "no_answer", "path": {"brand": "aquafix", "lead": "L-1"}, "body": null}, "leaves": ["take"]},
		])
	);
	assert_eq!(notes(&app, &conductor, body["next"].as_i64().unwrap()).await, json!({"notes": [], "next": body["next"]}));

	// A press: the button's operation, resolved in the document as the conductor's catalog does,
	// called as the user who pressed, under the key the outbox derives.
	let document = serde_json::to_value(panel_server::http::document()).unwrap();
	let olga = As::user(alias("sa:operator"));
	let press = |button: &'static str| {
		let call = note["buttons"].as_array().unwrap().iter().find(|b| b["id"] == button).unwrap()["call"].clone();
		let (template, method) = document["paths"]
			.as_object()
			.unwrap()
			.iter()
			.flat_map(|(path, item)| item.as_object().unwrap().iter().map(move |(m, op)| (path, m, op)))
			.find(|(.., op)| op["operationId"] == call["operation"])
			.map(|(path, m, _)| (path.clone(), m.to_uppercase()))
			.expect("the operation is in the document");
		let path = call["path"]
			.as_object()
			.unwrap()
			.iter()
			.fold(format!("/api/v1{template}"), |p, (name, v)| p.replace(&format!("{{{name}}}"), v.as_str().unwrap()));
		let (app, conductor, olga) = (app.clone(), &conductor, olga.clone());
		async move {
			let key = format!("tg/7/{button}");
			let body = Some(call["body"].clone()).filter(|b| !b.is_null());
			send(&app, conductor, &olga, Method::from_bytes(method.as_bytes()).unwrap(), &path, body, &[("idempotency-key", &key)])
				.await
				.0
		}
	};
	assert_eq!(press("no_answer").await, StatusCode::CREATED);
	assert_eq!(press("no_answer").await, StatusCode::OK, "pressed again: recorded once");
	assert_eq!(press("take").await, StatusCode::CREATED);
	assert_eq!(press("take").await, StatusCode::OK);
	let (status, _) = send(&app, &conductor, &olga, Method::POST, "/api/v1/leads/aquafix/L-1/no_answer", None, &[]).await;
	assert_eq!(status, StatusCode::BAD_REQUEST, "a no-answer without its key could be recorded twice");
	let (status, _) = send(
		&app,
		&conductor,
		&As::user(alias("sa:operator")),
		Method::POST,
		"/api/v1/leads/aquafix/L-9/no_answer",
		None,
		&[("idempotency-key", "k")],
	)
	.await;
	assert_eq!(status, StatusCode::NOT_FOUND);
	let (status, _) = send(
		&app,
		&conductor,
		&As::user([sa_auth::Work::Read.as_str()].into_iter().collect()),
		Method::POST,
		"/api/v1/leads/aquafix/L-1/no_answer",
		None,
		&[("idempotency-key", "k")],
	)
	.await;
	assert_eq!(status, StatusCode::FORBIDDEN);

	// Typed in by an operator: they are not told of their own lead.
	let by = uuid::Uuid::now_v7();
	let lead = NewLead {
		brand: BrandId::parse("aquafix").unwrap(),
		location: LocationId::parse("royat").unwrap(),
		need: "a tap".into(),
		phone: None,
		channel: ManualChannel::PhoneInbound,
	};
	panel.create_lead(Actor(by), lead, now).await.unwrap();
	assert_eq!(panel.fan_out(now, Locale::Ru).await.unwrap(), 1);
	let body = notes(&app, &conductor, body["next"].as_i64().unwrap()).await;
	assert_eq!(body["notes"][0]["skip_user"], by.to_string());
	assert!(body["notes"][0]["text_pii"]["text"].as_str().unwrap().ends_with("Нужно: a tap"));

	let (status, _) = send(&app, &conductor, &As::Conductor, Method::GET, "/api/conductor/notifications?after=0&limit=501", None, &[]).await;
	assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// The tables an `/api/v1` read's answer says it read.
async fn reads(app: &axum::Router, conductor: &Conductor, path: &str) -> Vec<String> {
	let assertion = conductor.user(uuid::Uuid::now_v7(), &alias("sa:operator"), "GET", path);
	let res = app
		.clone()
		.oneshot(Request::get(path).header(sa_auth::HEADER, assertion).body(Body::empty()).unwrap())
		.await
		.unwrap();
	assert_eq!(res.status(), StatusCode::OK, "{path}");
	let header = res.headers()["x-sa-reads"].to_str().unwrap();
	header.split(", ").filter(|t| !t.is_empty()).map(str::to_owned).collect()
}

#[tokio::test]
async fn a_write_is_told_by_its_tables_and_refreshes_only_the_reads_of_them() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let panel = panel(&db).await;
	let app = app(panel.clone(), &conductor);
	let path = sa_core::feeds::CHANGES;
	let req = Request::get(path).header(sa_auth::HEADER, conductor.conductor("GET", path)).body(Body::empty()).unwrap();
	let res = app.clone().oneshot(req).await.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
	assert_eq!(res.headers()["content-type"], "application/x-ndjson");
	let mut body = res.into_body();
	let mut next = async || -> Value {
		let frame = tokio::time::timeout(std::time::Duration::from_secs(5), body.frame()).await.expect("a line").unwrap().unwrap();
		let line = frame.into_data().unwrap();
		assert_eq!(line.last(), Some(&b'\n'));
		serde_json::from_slice(&line).unwrap()
	};
	assert_eq!(next().await, json!({"type": "resync"}));

	let lead = json!({"brand": "aquafix", "location": "royat", "need": "a tap"});
	let (status, _) = send(&app, &conductor, &As::user(alias("sa:operator")), Method::POST, "/api/v1/leads", Some(lead), &[]).await;
	assert_eq!(status, StatusCode::CREATED);
	let mut topics = Vec::new();
	while !topics.iter().any(|t| t == "sites.leads") {
		let line = next().await;
		assert_eq!(line["type"], "changed", "{line}");
		topics.push(line["topic"].as_str().unwrap().to_owned());
	}

	assert!(
		reads(&app, &conductor, "/api/v1/leads").await.contains(&"sites.leads".to_owned()),
		"the list follows the leads it read"
	);
	let invoices = reads(&app, &conductor, "/api/v1/invoices").await;
	assert!(!invoices.contains(&"sites.leads".to_owned()), "the invoices do not follow the leads: {invoices:?}");

	panel.bus().going_away();
	let ended = tokio::time::timeout(std::time::Duration::from_secs(5), async {
		while let Some(frame) = body.frame().await {
			frame.unwrap();
		}
	});
	assert!(ended.await.is_ok(), "the feed ends when the process stops");
}

#[tokio::test]
async fn each_key_s_last_receipt() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let panel = panel(&db).await;
	let app = app(panel.clone(), &conductor);
	let (_, empty) = send(&app, &conductor, &As::Conductor, Method::GET, "/api/conductor/sources/received", None, &[]).await;
	assert_eq!(empty, json!({}));
	let (earlier, now) = ("2026-10-01T08:00:00Z".parse::<Timestamp>().unwrap(), "2026-10-01T09:30:00Z".parse::<Timestamp>().unwrap());
	let site = grant("aquafix-site", SourceKind::Site, &["aquafix"]);
	panel.ingest(&site, &batch("aquafix-site", &[site_lead("L-1", earlier)]), earlier).await.unwrap();
	panel.ingest(&site, &batch("aquafix-site", &[site_lead("L-2", now)]), now).await.unwrap();
	panel
		.create_lead(
			Actor(uuid::Uuid::now_v7()),
			NewLead {
				brand: BrandId::parse("aquafix").unwrap(),
				location: LocationId::parse("royat").unwrap(),
				need: "a tap".into(),
				phone: None,
				channel: ManualChannel::PhoneInbound,
			},
			now,
		)
		.await
		.unwrap();
	let (status, body) = send(&app, &conductor, &As::Conductor, Method::GET, "/api/conductor/sources/received", None, &[]).await;
	assert_eq!(
		(status, body),
		(StatusCode::OK, json!({"aquafix-site": "2026-10-01T09:30:00Z"})),
		"the panel's own events name no key"
	);
}

#[tokio::test]
async fn the_conductor_sees_which_internal_reads_a_brand_made_since_boot() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor);
	let read = |who: &'static As, uri: &'static str| send(&app, &conductor, who, Method::GET, uri, None, &[]);

	let (_, before) = read(&As::Conductor, "/api/conductor/internal-reads").await;
	assert_eq!(before["reads"], json!({}));
	let since = before["since"].as_str().unwrap().parse::<Timestamp>().unwrap();

	read(&As::Nobody, "/api/internal/brands/aquafix/locations/royat").await;
	read(&As::Nobody, "/api/internal/brands/aquafix/pricing").await;

	let (status, after) = read(&As::Conductor, "/api/conductor/internal-reads").await;
	assert_eq!(status, StatusCode::OK, "{after}");
	assert_eq!(after["since"], before["since"]);
	let seen = &after["reads"]["aquafix"];
	for endpoint in ["locations", "pricing"] {
		assert!(seen[endpoint].as_str().unwrap().parse::<Timestamp>().unwrap() >= since, "{endpoint}");
	}
	assert_eq!(seen["experiments"], Value::Null, "never read");
	assert!(after["reads"].get("vifnet").is_none());

	let (status, _) = read(&As::Nobody, "/api/conductor/internal-reads").await;
	assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_conductor_reads_which_place_each_gmb_is_the_landing_of() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor);
	let admin = As::user(panel::testing::admin());
	for (slug, gmb) in [("royat", Some(7)), ("vichy", None), ("thiers", Some(8))] {
		let (status, body) = send(&app, &conductor, &admin, Method::POST, "/api/v1/places", Some(json!({"brand": "aquafix", "slug": slug})), &[]).await;
		assert_eq!(status, StatusCode::CREATED, "{body}");
		if let Some(gmb) = gmb {
			let (status, body) = send(
				&app,
				&conductor,
				&admin,
				Method::PUT,
				&format!("/api/v1/places/aquafix/{slug}/gmb"),
				Some(json!({"gmb_id": gmb})),
				&[],
			)
			.await;
			assert_eq!(status, StatusCode::OK, "{body}");
		}
	}
	let (status, body) = send(&app, &conductor, &admin, Method::POST, "/api/v1/places/aquafix/thiers/withdraw", None, &[]).await;
	assert_eq!(status, StatusCode::OK, "{body}");

	let (status, body) = send(&app, &conductor, &As::Conductor, Method::GET, "/api/conductor/places", None, &[]).await;
	assert_eq!(
		(status, body),
		(StatusCode::OK, json!({"places": [{"gmb_id": 7, "brand": "aquafix", "slug": "royat"}]})),
		"without a gmb_id, or withdrawn: not listed"
	);
	let (status, _) = send(&app, &conductor, &As::Nobody, Method::GET, "/api/conductor/places", None, &[]).await;
	assert_eq!(status, StatusCode::UNAUTHORIZED);
}
