//! Invoices end to end: the real router behind the conductor's assertion, the real gRPC client, and a
//! stand-in banking speaking the vendored `scope.v1` contract on a local port.

use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use axum::{
	Extension, Router,
	http::{Method, StatusCode},
};
use panel::testing::{Conductor, TestDb, panel};
use panel_contracts::scope::v1::{
	self as pb,
	invoice_service_server::{InvoiceService, InvoiceServiceServer},
	money_event::Kind,
	money_events_server::{MoneyEvents, MoneyEventsServer},
	scope_auth_service_server::{ScopeAuthService, ScopeAuthServiceServer},
	scope_ledger_service_server::{ScopeLedgerService, ScopeLedgerServiceServer},
};
use panel_server::{
	scope_ledger::ScopeLedger,
	testing::{As, Client, alias},
};
use serde_json::{Value, json};
use tonic::{Request as GrpcRequest, Response as GrpcResponse, Status, transport::server::TcpIncoming};

const PAID_AT: i64 = 1_793_000_000;

// ── the stand-in banking ─────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Bank {
	/// `false`: every call is PERMISSION_DENIED `not_granted`, as for a client without the grant.
	granted: bool,
	mints: u32,
	opens: u32,
	creates: u32,
	gets: u32,
	/// The next CreateInvoice is created, but its answer is lost (UNAVAILABLE).
	lose_next_answer: bool,
	/// By idempotency key.
	invoices: HashMap<String, pb::Invoice>,
	events: Vec<pb::MoneyEvent>,
}

#[derive(Clone, Default)]
struct Fake(Arc<Mutex<Bank>>);

impl Fake {
	fn granted() -> Self {
		let fake = Self::default();
		fake.bank().granted = true;
		fake
	}

	fn bank(&self) -> std::sync::MutexGuard<'_, Bank> {
		self.0.lock().unwrap()
	}

	/// The bearer the last mint handed out, and the grant: what banking checks on every call.
	fn admit<T>(&self, r: &GrpcRequest<T>) -> Result<(), Status> {
		let bank = self.bank();
		let expected = format!("Bearer tok-{}", bank.mints);
		if r.metadata().get("authorization").and_then(|v| v.to_str().ok()) != Some(expected.as_str()) {
			return Err(Status::unauthenticated("token"));
		}
		if !bank.granted {
			return Err(Status::permission_denied("not_granted"));
		}
		Ok(())
	}

	fn set_state(&self, state: pb::InvoiceState) {
		let mut bank = self.bank();
		let invoice = bank.invoices.values_mut().next().expect("an invoice was issued");
		invoice.state = state.into();
		if state == pb::InvoiceState::Paid {
			invoice.paid_by_auth_subject = "payer-subject".into();
			invoice.paid_at = PAID_AT;
		}
	}

	fn push_event(&self, event_id: &str, kind: Kind, ref_id: &str) {
		let mut bank = self.bank();
		let position = bank.events.len() as i64 + 1;
		bank.events.push(pb::MoneyEvent {
			event_id: event_id.into(),
			position,
			scope: "sa".into(),
			occurred_at: PAID_AT,
			kind: kind.into(),
			ref_id: ref_id.into(),
			auth_subject: "payer-subject".into(),
			..pb::MoneyEvent::default()
		});
	}
}

#[tonic::async_trait]
impl ScopeAuthService for Fake {
	async fn mint_service_token(&self, _: GrpcRequest<pb::MintServiceTokenRequest>) -> Result<GrpcResponse<pb::MintServiceTokenResponse>, Status> {
		let mut bank = self.bank();
		bank.mints += 1;
		Ok(GrpcResponse::new(pb::MintServiceTokenResponse {
			token: format!("tok-{}", bank.mints),
			expires_at: jiff::Timestamp::now().as_second() + 300,
		}))
	}
}

#[tonic::async_trait]
impl ScopeLedgerService for Fake {
	async fn open_account(&self, r: GrpcRequest<pb::OpenAccountRequest>) -> Result<GrpcResponse<pb::ScopeAccount>, Status> {
		self.admit(&r)?;
		self.bank().opens += 1;
		let r = r.into_inner();
		assert_eq!((r.scope.as_str(), r.label.as_str()), ("sa", "sa:invoices"));
		Ok(GrpcResponse::new(pb::ScopeAccount {
			scope: r.scope,
			label: r.label,
			balance: "0".into(),
			owner_client: "sa-panel".into(),
			..pb::ScopeAccount::default()
		}))
	}
}

#[tonic::async_trait]
impl InvoiceService for Fake {
	async fn create_invoice(&self, r: GrpcRequest<pb::CreateInvoiceRequest>) -> Result<GrpcResponse<pb::Invoice>, Status> {
		self.admit(&r)?;
		let r = r.into_inner();
		let mut bank = self.bank();
		bank.creates += 1;
		let n = bank.invoices.len();
		let invoice = bank
			.invoices
			.entry(r.idempotency_key.clone())
			.or_insert_with(|| pb::Invoice {
				invoice_id: format!("inv-{n}"),
				scope: r.scope.clone(),
				to_label: r.to_label.clone(),
				amount: r.amount.clone(),
				memo: r.memo.clone(),
				state: pb::InvoiceState::Open.into(),
				expires_at: r.expires_at,
				pay_url: format!("https://evinvest.ltd/pay/inv-{n}"),
				subject_ref: r.subject_ref.clone(),
				..pb::Invoice::default()
			})
			.clone();
		if (invoice.amount.as_str(), invoice.memo.as_str(), invoice.expires_at) != (r.amount.as_str(), r.memo.as_str(), r.expires_at) {
			return Err(Status::already_exists("idempotency_conflict"));
		}
		if std::mem::take(&mut bank.lose_next_answer) {
			return Err(Status::unavailable("the answer was lost"));
		}
		Ok(GrpcResponse::new(invoice))
	}

	async fn get_invoice(&self, r: GrpcRequest<pb::GetInvoiceRequest>) -> Result<GrpcResponse<pb::Invoice>, Status> {
		self.admit(&r)?;
		let r = r.into_inner();
		let mut bank = self.bank();
		bank.gets += 1;
		let found = bank.invoices.values().find(|i| i.invoice_id == r.invoice_id).cloned();
		found.map(GrpcResponse::new).ok_or_else(|| Status::not_found("invoice_not_found"))
	}
}

#[tonic::async_trait]
impl MoneyEvents for Fake {
	async fn pull(&self, r: GrpcRequest<pb::PullRequest>) -> Result<GrpcResponse<pb::PullResponse>, Status> {
		self.admit(&r)?;
		let r = r.into_inner();
		let bank = self.bank();
		let events: Vec<pb::MoneyEvent> = bank.events.iter().filter(|e| e.position > r.after_position).take(r.limit as usize).cloned().collect();
		let next_position = events.last().map_or(r.after_position, |e| e.position);
		Ok(GrpcResponse::new(pb::PullResponse { events, next_position }))
	}
}

/// Serves the fake on a free port for the life of the test; a client of it.
async fn serve_fake(fake: Fake) -> Arc<ScopeLedger> {
	let incoming = TcpIncoming::bind("127.0.0.1:0".parse().unwrap()).unwrap();
	let addr = incoming.local_addr().unwrap();
	let server = tonic::transport::Server::builder()
		.add_service(ScopeAuthServiceServer::new(fake.clone()))
		.add_service(ScopeLedgerServiceServer::new(fake.clone()))
		.add_service(InvoiceServiceServer::new(fake.clone()))
		.add_service(MoneyEventsServer::new(fake))
		.serve_with_incoming(incoming);
	// Dropped with the test's runtime; a failure shows up as UNAVAILABLE in the assertions.
	let _server = tokio::spawn(server);
	let channel = tonic::transport::Endpoint::from_shared(format!("http://{addr}")).unwrap().connect_lazy();
	Arc::new(ScopeLedger::over_channel(channel))
}

// ── the panel ────────────────────────────────────────────────────────────────────────────

fn app(panel: panel::Panel, conductor: &Conductor, ledger: Option<Arc<ScopeLedger>>) -> Router {
	let app = panel_server::testing::app(panel, conductor);
	match ledger {
		Some(l) => app.layer(Extension(l)),
		None => app,
	}
}

fn user<'a>(app: &'a Router, conductor: &'a Conductor, who: &str) -> Client<'a> {
	Client {
		app,
		conductor,
		who: As::user(alias(who)),
	}
}

async fn issue(b: &Client<'_>, key: Option<&str>, body: Value) -> (StatusCode, Value) {
	let headers: Vec<(&str, &str)> = key.map(|k| ("idempotency-key", k)).into_iter().collect();
	b.send(Method::POST, "/api/v1/invoices", Some(body), &headers).await
}

fn terms() -> Value {
	json!({"amount": "12.50", "memo": "Window cleaning, 3 rooms", "expires_in_days": 7})
}

#[tokio::test]
async fn an_invoice_is_issued_once_per_key_and_read_back() {
	let db = TestDb::create().await;
	let fake = Fake::granted();
	let ledger = serve_fake(fake.clone()).await;
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor, Some(ledger));
	let b = user(&app, &conductor, "sa:admin");

	let (status, e) = issue(&b, None, terms()).await;
	assert_eq!((status, e["error"].as_str()), (StatusCode::BAD_REQUEST, Some("an invoice needs an Idempotency-Key")));
	for bad in [
		json!({"amount": "-1", "expires_in_days": 7}),
		json!({"amount": "1", "expires_in_days": 31}),
		json!({"amount": "1", "memo": "a\nb", "expires_in_days": 7}),
		json!({"amount": 12.5, "expires_in_days": 7}),
	] {
		assert_eq!(issue(&b, Some("k-bad"), bad.clone()).await.0, StatusCode::BAD_REQUEST, "{bad}");
	}
	assert_eq!(fake.bank().creates, 0, "nothing invalid reaches banking");

	let (status, first) = issue(&b, Some("k-1"), terms()).await;
	assert_eq!(status, StatusCode::CREATED, "{first}");
	assert_eq!(first["invoice_id"], "inv-0");
	assert_eq!(first["amount"], "12.5", "canonical");
	assert_eq!(first["currency"], "USDT");
	assert_eq!(first["state"], "open");
	assert_eq!(first["pay_url"], "https://evinvest.ltd/pay/inv-0");
	assert_eq!((first["paid_at"].clone(), first["paid_by"].clone()), (Value::Null, Value::Null));
	let created: jiff::Timestamp = first["created_at"].as_str().unwrap().parse().unwrap();
	let expires: jiff::Timestamp = first["expires_at"].as_str().unwrap().parse().unwrap();
	assert_eq!(expires.duration_since(created), jiff::SignedDuration::from_hours(7 * 24));
	let sent = fake.bank().invoices.values().next().unwrap().clone();
	assert_eq!(sent.subject_ref, format!("sa_invoice:{}", first["id"].as_str().unwrap()));
	assert_eq!(sent.expires_at, expires.as_second());

	assert_eq!(issue(&b, Some("k-1"), terms()).await, (StatusCode::OK, first.clone()), "a retry is the first answer");
	let other = json!({"amount": "13", "memo": "Window cleaning, 3 rooms", "expires_in_days": 7});
	let (status, e) = issue(&b, Some("k-1"), other).await;
	assert_eq!((status, e["error"].as_str()), (StatusCode::CONFLICT, Some("this Idempotency-Key was used for another invoice")));

	assert_eq!(b.get("/api/v1/invoices").await, (StatusCode::OK, json!({"invoices": [first.clone()]})));
	let id = first["id"].as_str().unwrap();
	assert_eq!(b.get(&format!("/api/v1/invoices/{id}")).await, (StatusCode::OK, first.clone()));
	assert_eq!(b.get("/api/v1/invoices/not-a-uuid").await.0, StatusCode::NOT_FOUND);
	assert_eq!(b.get(&format!("/api/v1/invoices/{}", uuid::Uuid::nil())).await.0, StatusCode::NOT_FOUND);

	let bank = fake.bank();
	assert_eq!((bank.invoices.len(), bank.creates, bank.opens), (1, 1, 1), "one invoice, the account opened once");
	assert_eq!(bank.mints, 1, "one token across every call");
}

#[tokio::test]
async fn an_operator_without_the_permission_reads_but_cannot_issue() {
	let db = TestDb::create().await;
	let fake = Fake::granted();
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor, Some(serve_fake(fake.clone()).await));
	let b = user(&app, &conductor, "sa:operator");
	assert_eq!(issue(&b, Some("k-1"), terms()).await.0, StatusCode::FORBIDDEN);
	assert_eq!(b.get("/api/v1/invoices").await, (StatusCode::OK, json!({"invoices": []})));
	assert_eq!(fake.bank().mints, 0);
}

#[tokio::test]
async fn without_a_ledger_or_a_grant_the_answer_is_503() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let unconfigured = app(panel(&db).await, &conductor, None);
	let b = user(&unconfigured, &conductor, "sa:admin");
	assert_eq!(issue(&b, Some("k-1"), terms()).await, (StatusCode::SERVICE_UNAVAILABLE, json!({"error": "invoices_unavailable"})));
	assert_eq!(b.get("/api/v1/invoices").await, (StatusCode::OK, json!({"invoices": []})), "the list needs no banking");

	let fake = Fake::default();
	let ungranted = app(panel(&db).await, &conductor, Some(serve_fake(fake.clone()).await));
	let b = user(&ungranted, &conductor, "sa:admin");
	assert_eq!(issue(&b, Some("k-1"), terms()).await, (StatusCode::SERVICE_UNAVAILABLE, json!({"error": "not_granted"})));
	assert_eq!(b.get("/api/v1/invoices").await, (StatusCode::OK, json!({"invoices": []})), "a refusal leaves no row");

	fake.bank().granted = true;
	assert_eq!(issue(&b, Some("k-1"), terms()).await.0, StatusCode::CREATED, "the key is free again once banking grants");
}

#[tokio::test]
async fn a_lost_answer_is_repaired_by_the_retry_or_the_next_read() {
	let db = TestDb::create().await;
	let fake = Fake::granted();
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor, Some(serve_fake(fake.clone()).await));
	let b = user(&app, &conductor, "sa:admin");

	fake.bank().lose_next_answer = true;
	assert_eq!(issue(&b, Some("k-1"), terms()).await, (StatusCode::SERVICE_UNAVAILABLE, json!({"error": "ledger_unavailable"})));
	let (_, list) = b.get("/api/v1/invoices").await;
	let row = &list["invoices"][0];
	assert_eq!((row["state"].as_str(), row["invoice_id"].clone()), (Some("creating"), Value::Null));

	// The panel missed the answer; banking has the invoice, and the payment comes meanwhile.
	fake.set_state(pb::InvoiceState::Paid);
	let (status, read) = b.get(&format!("/api/v1/invoices/{}", row["id"].as_str().unwrap())).await;
	assert_eq!(status, StatusCode::OK, "{read}");
	assert_eq!((read["invoice_id"].as_str(), read["state"].as_str()), (Some("inv-0"), Some("paid")));
	assert_eq!(read["paid_by"], "payer-subject");
	assert_eq!(fake.bank().invoices.len(), 1, "asked again under the same key: no second invoice");

	assert_eq!(issue(&b, Some("k-1"), terms()).await, (StatusCode::OK, read), "the operator's retry finds it");
}

#[tokio::test]
async fn a_payment_is_confirmed_once_from_the_money_events() {
	let db = TestDb::create().await;
	let fake = Fake::granted();
	let ledger = serve_fake(fake.clone()).await;
	let panel = panel(&db).await;
	let conductor = Conductor::new("k1");
	let app = app(panel.clone(), &conductor, Some(ledger.clone()));
	let b = user(&app, &conductor, "sa:admin");
	let (_, issued) = issue(&b, Some("k-1"), terms()).await;
	let item = format!("/api/v1/invoices/{}", issued["id"].as_str().unwrap());

	fake.set_state(pb::InvoiceState::Processing);
	let (_, read) = b.get(&item).await;
	assert_eq!((read["state"].as_str(), read["paid_at"].clone()), (Some("processing"), Value::Null), "processing is not paid");

	fake.set_state(pb::InvoiceState::Paid);
	fake.push_event("e-0", Kind::InvoiceCreated, "inv-0");
	fake.push_event("e-1", Kind::InvoicePaid, "inv-0");
	fake.push_event("e-1", Kind::InvoicePaid, "inv-0");
	fake.push_event("e-2", Kind::Charged, "hold-9");
	fake.push_event("e-3", Kind::InvoicePaid, "inv-somebody-else");
	let done = panel.confirm_invoices(&*ledger).await.unwrap();
	assert_eq!((done.events, done.paid, done.position), (5, 1, 5), "a redelivered event is taken once");
	let again = panel.confirm_invoices(&*ledger).await.unwrap();
	assert_eq!((again.events, again.paid, again.position), (0, 0, 5));

	let gets = fake.bank().gets;
	let (_, paid) = b.get(&item).await;
	assert_eq!(paid["state"], "paid");
	assert_eq!(paid["paid_at"], jiff::Timestamp::from_second(PAID_AT).unwrap().to_string());
	assert_eq!(paid["paid_by"], "payer-subject");
	assert_eq!(fake.bank().gets, gets, "a paid invoice is final: banking is not asked again");

	// A later cancellation, as a stale event, does not take the payment back.
	fake.push_event("e-4", Kind::InvoiceCancelled, "inv-0");
	panel.confirm_invoices(&*ledger).await.unwrap();
	assert_eq!(b.get(&item).await.1, paid);

	let pool = db.pool().await;
	let payments: i64 = sqlx::query_scalar("SELECT count(*) FROM payments").fetch_one(&pool).await.unwrap();
	assert_eq!(payments, 0, "an invoice is not a lead's payment");
	let event: Option<String> = sqlx::query_scalar("SELECT paid_event_id FROM invoices").fetch_one(&pool).await.unwrap();
	assert_eq!(event.as_deref(), Some("e-1"));
}

#[tokio::test]
async fn a_cancelled_or_expired_invoice_ends_so() {
	let db = TestDb::create().await;
	let fake = Fake::granted();
	let ledger = serve_fake(fake.clone()).await;
	let panel = panel(&db).await;
	let conductor = Conductor::new("k1");
	let app = app(panel.clone(), &conductor, Some(ledger.clone()));
	let b = user(&app, &conductor, "sa:admin");
	let (_, one) = issue(&b, Some("k-1"), terms()).await;
	let (_, two) = issue(&b, Some("k-2"), terms()).await;
	fake.push_event("e-1", Kind::InvoiceCancelled, one["invoice_id"].as_str().unwrap());
	fake.push_event("e-2", Kind::InvoiceExpired, two["invoice_id"].as_str().unwrap());
	panel.confirm_invoices(&*ledger).await.unwrap();
	let (_, list) = b.get("/api/v1/invoices").await;
	let states: HashMap<&str, &str> = list["invoices"]
		.as_array()
		.unwrap()
		.iter()
		.map(|i| (i["invoice_id"].as_str().unwrap(), i["state"].as_str().unwrap()))
		.collect();
	assert_eq!(states, HashMap::from([("inv-0", "cancelled"), ("inv-1", "expired")]));
}

/// The `invoices` changes told on the bus since the last call, as `(id named?)`.
fn told(rx: &mut tokio::sync::broadcast::Receiver<panel::live::Signal>) -> Vec<bool> {
	let mut out = Vec::new();
	while let Ok(signal) = rx.try_recv() {
		if let panel::live::Signal::Changed(c) = signal {
			assert_eq!(c.topic, panel::live::Topic::Invoices, "{c:?}");
			assert_eq!(c.brand, None);
			out.push(c.id.is_some());
		}
	}
	out
}

#[tokio::test]
async fn the_screens_are_told_when_an_invoice_row_changes() {
	let db = TestDb::create().await;
	let fake = Fake::granted();
	let ledger = serve_fake(fake.clone()).await;
	let panel = panel(&db).await;
	let mut rx = panel.bus().subscribe();
	let conductor = Conductor::new("k1");
	let app = app(panel.clone(), &conductor, Some(ledger.clone()));
	let b = user(&app, &conductor, "sa:admin");

	let (_, issued) = issue(&b, Some("k-1"), terms()).await;
	assert_eq!(told(&mut rx), [true, true], "stored creating, then as banking issued it");
	issue(&b, Some("k-1"), terms()).await;
	assert_eq!(told(&mut rx), [false; 0], "a retry changes nothing");

	let item = format!("/api/v1/invoices/{}", issued["id"].as_str().unwrap());
	b.get(&item).await;
	assert_eq!(told(&mut rx), [false; 0], "a read banking answers the same");
	fake.set_state(pb::InvoiceState::Processing);
	b.get(&item).await;
	assert_eq!(told(&mut rx), [true], "a read that found it processing");

	panel.confirm_invoices(&*ledger).await.unwrap();
	assert_eq!(told(&mut rx), [false; 0], "no events, nothing told");
	fake.push_event("e-0", Kind::Charged, "hold-9");
	panel.confirm_invoices(&*ledger).await.unwrap();
	assert_eq!(told(&mut rx), [false; 0], "events about nothing the panel holds");
	fake.push_event("e-1", Kind::InvoicePaid, "inv-0");
	panel.confirm_invoices(&*ledger).await.unwrap();
	assert_eq!(told(&mut rx), [false], "paid: one change for the page");

	let (_, two) = issue(&b, Some("k-2"), terms()).await;
	told(&mut rx);
	fake.push_event("e-2", Kind::InvoiceExpired, two["invoice_id"].as_str().unwrap());
	panel.confirm_invoices(&*ledger).await.unwrap();
	assert_eq!(told(&mut rx), [false], "expired");

	fake.bank().granted = false;
	issue(&b, Some("k-3"), terms()).await;
	assert_eq!(told(&mut rx), [true, true], "stored creating, then dropped on the refusal");
}
