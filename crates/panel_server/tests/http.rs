//! The HTTP surface end to end, through the real router: who gets in (the conductor's assertion,
//! and only for what it was signed for), ingest under a source's, a bot's lookup.

use axum::{
	body::Body,
	http::{Method, Request, StatusCode},
};
use jiff::Timestamp;
use panel::testing::{Conductor, TestDb, batch, event, grant, messenger_lead, panel};
use panel_server::testing::{As, app, app_with, raw, send};
use sa_auth::{Principal, Service, SourceKind};
use serde_json::{Value, json};
use tower::ServiceExt;

const INGEST: &str = "/api/ingest/v1/events";

fn site() -> As {
	As::source("aquafix-site", SourceKind::Site, &["aquafix"])
}

fn json_of(bytes: &[u8]) -> Value {
	serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

/// `body` posted to `path` with `assertion` as the header, whatever it was signed for.
async fn with_assertion(app: &axum::Router, method: Method, path: &str, assertion: Option<String>, body: Vec<u8>) -> (StatusCode, Value) {
	let mut req = Request::builder().method(method).uri(path);
	if let Some(a) = assertion {
		req = req.header(sa_auth::HEADER, a);
	}
	let res = app.clone().oneshot(req.body(Body::from(body)).unwrap()).await.unwrap();
	let status = res.status();
	(status, json_of(&axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap()))
}

#[tokio::test]
async fn ingest_over_http() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor);
	let now = Timestamp::now();
	let lead = json!({"brandId": "aquafix", "leadId": "L-1"});
	let created = event("lead.created", now, "site", lead.clone(), json!({"channel": "form"}));
	let events = [
		created.clone(),
		created,
		event("lead.created", now, "site", json!({"brandId": "vifnet", "leadId": "V-1"}), json!({"channel": "form"})),
		event("review.new", now, "site", lead, json!({})),
	];
	let body = batch("aquafix-site", &events);

	let (status, answer) = raw(&app, &conductor, &site(), Method::POST, INGEST, &[], body.clone()).await;
	let answer = json_of(&answer);
	assert_eq!(status, StatusCode::MULTI_STATUS, "{answer}");
	let results = answer["results"].as_array().unwrap();
	let statuses: Vec<&str> = results.iter().map(|r| r["status"].as_str().unwrap()).collect();
	assert_eq!(statuses, ["accepted", "duplicate", "rejected", "accepted"]);
	assert_eq!(results[2]["reason"], "this key may not write for brand vifnet", "the grant is the assertion's");
	assert_eq!(results[2]["index"], 2);
	assert_eq!(results[0]["id"], events[0]["id"]);
	assert!(results[3]["reason"].as_str().unwrap().contains("not registered"));

	let (status, answer) = raw(&app, &conductor, &site(), Method::POST, INGEST, &[], batch("aquafix-site", &[])).await;
	assert_eq!(status, StatusCode::BAD_REQUEST, "{:?}", json_of(&answer));
	let (status, _) = raw(&app, &conductor, &site(), Method::POST, INGEST, &[], vec![b' '; panel_server::http::MAX_BODY + 1]).await;
	assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

	let res = app.clone().oneshot(Request::get("/health").body(Body::empty()).unwrap()).await.unwrap();
	assert_eq!(res.status(), StatusCode::OK);
}

/// Every way a request may not be the conductor's, or not this request's: one answer, 401.
#[tokio::test]
async fn only_the_conductors_assertion_for_this_request_gets_in() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor);
	let now = Timestamp::now();
	let body = batch(
		"aquafix-site",
		&[event("lead.created", now, "site", json!({"brandId": "aquafix", "leadId": "L-1"}), json!({"channel": "form"}))],
	);
	let source = |c: &Conductor, body: &[u8]| c.source("aquafix-site", SourceKind::Site, &["aquafix"], "POST", INGEST, body);
	let unauthenticated = json!({"error": "unauthenticated"});
	let operator = panel_server::testing::alias("sa:operator");
	let refused = [
		("no assertion", Method::POST, INGEST, None, body.clone()),
		("a key of the same name", Method::POST, INGEST, Some(source(&Conductor::new("k1"), &body)), body.clone()),
		("a key it does not know", Method::POST, INGEST, Some(source(&Conductor::new("k2"), &body)), body.clone()),
		("another body", Method::POST, INGEST, Some(source(&conductor, b"{\"events\":[]}")), body.clone()),
		(
			"another service",
			Method::POST,
			INGEST,
			Some(conductor.sign(
				Service::ReviewArchive,
				Principal::Source(sa_auth::Source::new("aquafix-site".into(), SourceKind::Site, ["aquafix".to_owned()].into(), &body)),
				"POST",
				INGEST,
			)),
			body.clone(),
		),
		(
			"a user on ingest",
			Method::POST,
			INGEST,
			Some(conductor.user(uuid::Uuid::now_v7(), &operator, "POST", INGEST)),
			body.clone(),
		),
		(
			"a kind no key is issued for",
			Method::POST,
			INGEST,
			Some(conductor.source("aquafix-ops", SourceKind::Panel, &["aquafix"], "POST", INGEST, &body)),
			body.clone(),
		),
		("no assertion on /api/v1", Method::GET, "/api/v1/leads", None, Vec::new()),
		(
			"another path",
			Method::GET,
			"/api/v1/leads",
			Some(conductor.user(uuid::Uuid::now_v7(), &operator, "GET", "/api/v1/places")),
			Vec::new(),
		),
		(
			"another method",
			Method::GET,
			"/api/v1/leads",
			Some(conductor.user(uuid::Uuid::now_v7(), &operator, "POST", "/api/v1/leads")),
			Vec::new(),
		),
		(
			"the conductor on /api/v1",
			Method::GET,
			"/api/v1/leads",
			Some(conductor.conductor("GET", "/api/v1/leads")),
			Vec::new(),
		),
		(
			"a user on the conductor's feeds",
			Method::GET,
			"/api/conductor/sources/received",
			Some(conductor.user(uuid::Uuid::now_v7(), &panel_server::testing::alias("sa:admin"), "GET", "/api/conductor/sources/received")),
			Vec::new(),
		),
		(
			"a user whose sub is no user id",
			Method::GET,
			"/api/v1/leads",
			Some(conductor.sign(
				Service::Sites,
				Principal::User(sa_auth::User {
					sub: "olga".into(),
					email: "o@example.com".into(),
					email_verified: true,
					name: "Olga".into(),
					permissions: operator.iter().filter(|p| p.starts_with("sa:sites:")).collect(),
				}),
				"GET",
				"/api/v1/leads",
			)),
			Vec::new(),
		),
	];
	for (why, method, path, assertion, body) in refused {
		assert_eq!(
			with_assertion(&app, method, path, assertion, body).await,
			(StatusCode::UNAUTHORIZED, unauthenticated.clone()),
			"{why}"
		);
	}
	let n: i64 = sqlx::query_scalar("SELECT count(*) FROM events").fetch_one(&db.pool().await).await.unwrap();
	assert_eq!(n, 0, "nothing refused was journaled");

	let (status, _) = send(&app, &conductor, &As::user(operator.clone()), Method::GET, "/api/v1/leads?limit=5", None, &[]).await;
	assert_eq!(status, StatusCode::OK, "the query is not part of what is signed");
	let work_only: sa_auth::PermissionSet = [sa_auth::Work::Read.as_str()].into_iter().collect();
	let (status, _) = send(&app, &conductor, &As::user(work_only.clone()), Method::GET, "/api/v1/funnel", None, &[]).await;
	assert_eq!(status, StatusCode::FORBIDDEN, "signed in, outside the section");
	let sub = uuid::Uuid::now_v7();
	let (status, me) = send(&app, &conductor, &As::User(sub, work_only), Method::GET, "/api/v1/me", None, &[]).await;
	assert_eq!((status, me), (StatusCode::OK, json!({"sub": sub.to_string(), "permissions": ["sa:sites:work:read"]})));
	let (status, _) = send(&app, &conductor, &As::Nobody, Method::GET, "/api/internal/brands/aquafix/locations/royat", None, &[]).await;
	assert_eq!(status, StatusCode::OK, "the landings' reads take no assertion");
}

/// A body that never arrives.
fn stalled() -> Body {
	Body::from_stream(futures::stream::pending::<Result<Vec<u8>, std::io::Error>>())
}

fn post(assertion: Option<&str>, body: Body) -> Request<Body> {
	let mut req = Request::post(INGEST);
	if let Some(a) = assertion {
		req = req.header(sa_auth::HEADER, a);
	}
	req.body(body).unwrap()
}

#[tokio::test]
async fn costly_requests_are_refused_before_the_body_is_read() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let panel = panel(&db).await;
	let quick = std::time::Duration::from_secs(5);
	let assertion = conductor.source("aquafix-site", SourceKind::Site, &["aquafix"], "POST", INGEST, b"{}");

	let app = app(panel.clone(), &conductor);
	let res = tokio::time::timeout(quick, app.clone().oneshot(post(None, stalled())))
		.await
		.expect("answered without the body")
		.unwrap();
	assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

	let limits = panel_server::http::Limits {
		max_concurrent: 1,
		body_timeout: std::time::Duration::from_millis(200),
		..panel_server::http::Limits::default()
	};
	let app = app_with(panel, &conductor, limits, panel::booking::PushSources::default());
	let res = tokio::time::timeout(quick, app.clone().oneshot(post(Some(&assertion), stalled()))).await.unwrap().unwrap();
	assert_eq!(res.status(), StatusCode::REQUEST_TIMEOUT, "a body too slow to arrive");

	// One slow request holds the only slot; the next is shed, not queued.
	let (tx, rx) = tokio::sync::oneshot::channel::<()>();
	let held = futures::stream::once(async move {
		rx.await.expect("released below");
		Ok::<_, std::io::Error>(b"{}".to_vec())
	});
	let first = app.clone().oneshot(post(Some(&assertion), Body::from_stream(held)));
	let second = async {
		tokio::time::sleep(std::time::Duration::from_millis(50)).await;
		let res = app.clone().oneshot(post(Some(&assertion), Body::from("{}"))).await.unwrap();
		tx.send(()).unwrap();
		res
	};
	let (first, second) = tokio::join!(first, second);
	assert_eq!(second.status(), StatusCode::SERVICE_UNAVAILABLE);
	assert_eq!(first.unwrap().status(), StatusCode::BAD_REQUEST, "the held one goes on to be judged");
}

/// A bot looks a lead up by the ref the customer brought, under its key's assertion over an
/// empty body: only its own brand's, only a bot's key; what the customer asked for, never who
/// they are.
#[tokio::test]
async fn a_bot_looks_a_lead_up_by_its_ref() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let panel = panel(&db).await;
	let now = Timestamp::now();
	let earlier = now - jiff::SignedDuration::from_mins(5);
	let mut priced = messenger_lead(now, "aquafix", "L-2", "telegram", "AQ-7K3F");
	priced["properties"]["flow"] = json!("fixed");
	priced["properties"]["quotedCents"] = json!(9900);
	priced["properties"]["pricingValidFrom"] = json!("2026-10-01");
	let leads = [messenger_lead(earlier, "aquafix", "L-1", "whatsapp", "AQ-7K3F"), priced];
	let got = panel
		.ingest(&grant("aquafix-site", SourceKind::Site, &["aquafix"]), &batch("aquafix-site", &leads), now)
		.await
		.unwrap();
	assert!(got.iter().all(|v| v.outcome == panel::Outcome::Accepted { unregistered: false }), "{got:?}");
	let app = app(panel, &conductor);
	let bot = As::source("aquafix-tg", SourceKind::Bot, &["aquafix"]);
	let uri = "/api/ingest/v1/leads/by-ref/aquafix/AQ-7K3F";
	let look_up = |who: As, uri: &'static str| {
		let (app, conductor) = (app.clone(), &conductor);
		async move { send(&app, conductor, &who, Method::GET, uri, None, &[]).await }
	};

	let (status, body) = look_up(bot.clone(), uri).await;
	assert_eq!(status, StatusCode::OK, "{body}");
	let created = body["created_at"].as_str().unwrap().to_owned();
	assert_eq!(
		body,
		json!({
			"lead_id": "L-2", "channel": "telegram", "stage": "created", "created_at": created, "message_ref": "AQ-7K3F",
			"need": "fuite sous l'évier", "locality": "Royat", "quoted_cents": 9900, "flow": "fixed",
		}),
		"the newest lead with the ref; no phone, no name"
	);
	let dump = body.to_string();
	assert!(!dump.contains("+336") && !dump.contains("Dupont"), "{dump}");

	let (status, body) = look_up(bot.clone(), "/api/ingest/v1/leads/by-ref/aquafix/AQ-0000").await;
	assert_eq!((status, body["error"].as_str()), (StatusCode::NOT_FOUND, Some("no lead of the brand carries that ref")));
	let (status, _) = look_up(bot.clone(), "/api/ingest/v1/leads/by-ref/aquafix/aq-7k3f").await;
	assert_eq!(status, StatusCode::BAD_REQUEST, "a ref is as the landing made it");
	let (status, body) = look_up(bot.clone(), "/api/ingest/v1/leads/by-ref/vifnet/AQ-7K3F").await;
	assert_eq!((status, body["error"].as_str()), (StatusCode::FORBIDDEN, Some("this key is not for that brand")));
	let (status, body) = look_up(site(), uri).await;
	assert_eq!((status, body["error"].as_str()), (StatusCode::FORBIDDEN, Some("only a bot's key looks a lead up by its ref")));
	let (status, _) = look_up(bot.clone(), "/api/ingest/v1/leads/by-ref/aquafix/AQ-7K3F?locale=fr").await;
	assert_eq!(status, StatusCode::OK, "the conductor checked the query with the MAC; the assertion binds the path");
	let over_a_body = conductor.source("aquafix-tg", SourceKind::Bot, &["aquafix"], "GET", uri, b"x");
	assert_eq!(with_assertion(&app, Method::GET, uri, Some(over_a_body), Vec::new()).await.0, StatusCode::UNAUTHORIZED);
	let elsewhere = conductor.source("aquafix-tg", SourceKind::Bot, &["aquafix"], "GET", "/api/ingest/v1/leads/by-ref/aquafix/AQ-0000", b"");
	assert_eq!(
		with_assertion(&app, Method::GET, uri, Some(elsewhere), Vec::new()).await.0,
		StatusCode::UNAUTHORIZED,
		"an assertion opens the one path it was made for"
	);

	// What it said, once the customer wrote.
	let wrote = event(
		"lead.messaged",
		now,
		"bot",
		json!({"brandId": "aquafix"}),
		json!({"channel": "telegram", "messageRef": "AQ-7K3F"}),
	);
	let (status, body) = raw(&app, &conductor, &bot, Method::POST, INGEST, &[], batch("aquafix-tg", &[wrote])).await;
	let body = json_of(&body);
	assert_eq!((status, body["results"][0]["status"].as_str()), (StatusCode::MULTI_STATUS, Some("accepted")), "{body}");
	let (_, body) = look_up(bot, uri).await;
	assert_eq!(body["messaged_channel"], "telegram");
	assert!(body["messaged_at"].is_string());
}

/// `/gmbs` counts, per linked place, the leads of the last week and those waiting too long,
/// however old.
#[tokio::test]
async fn a_gmb_says_how_many_leads_came_and_how_many_wait() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let panel = panel(&db).await;
	let now = Timestamp::now();
	let ago = |d: jiff::SignedDuration| now - d;
	let lead = |at: Timestamp, id: &str| messenger_lead(at, "aquafix", id, "whatsapp", "AQ-7K3F");
	let contacted = event(
		"lead.contacted",
		ago(jiff::SignedDuration::from_hours(24 * 20)),
		"panel",
		json!({"brandId": "aquafix", "locationId": "royat", "leadId": "L-old"}),
		json!({}),
	);
	let events = [
		lead(ago(jiff::SignedDuration::from_hours(2)), "L-late"),
		lead(ago(jiff::SignedDuration::from_mins(5)), "L-fresh"),
		lead(ago(jiff::SignedDuration::from_hours(24 * 21)), "L-old"),
	];
	let got = panel
		.ingest(&grant("aquafix-site", SourceKind::Site, &["aquafix"]), &batch("aquafix-site", &events), now)
		.await
		.unwrap();
	assert!(got.iter().all(|v| v.outcome == panel::Outcome::Accepted { unregistered: false }), "{got:?}");
	let got = panel
		.ingest(&grant("aquafix-panel", SourceKind::Panel, &["aquafix"]), &batch("aquafix-panel", &[contacted]), now)
		.await
		.unwrap();
	assert!(got.iter().all(|v| v.outcome == panel::Outcome::Accepted { unregistered: false }), "{got:?}");
	let app = app(panel, &conductor);
	let admin = As::user(panel::testing::admin());

	let (status, body) = send(&app, &conductor, &admin, Method::POST, "/api/v1/places", Some(json!({"brand": "aquafix", "slug": "royat"})), &[]).await;
	assert_eq!(status, StatusCode::CREATED, "{body}");
	let (status, body) = send(&app, &conductor, &admin, Method::PUT, "/api/v1/places/aquafix/royat/gmb", Some(json!({"gmb_id": 7})), &[]).await;
	assert_eq!(status, StatusCode::OK, "{body}");

	let (status, body) = send(&app, &conductor, &admin, Method::GET, "/api/v1/gmbs", None, &[]).await;
	assert_eq!(status, StatusCode::OK, "{body}");
	assert_eq!(body, json!({"7": {"brand": "aquafix", "location": "royat", "leads_7d": 2, "overdue": 1}}));
}
