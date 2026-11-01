//! Everything the runtime does runs on a freshly migrated database, and the schema itself
//! refuses what the code must never do: the journal is append-only. (On Postgres a runtime role's grants said so; SQLite has no roles, so the tables'
//! triggers do.)

use jiff::{SignedDuration, Timestamp};
use panel::{
	Outcome, Panel,
	operator::{Actor, FunnelBy, LeadQuery, NewLead, Payment, Pii},
	seal::DataKey,
	store::Store,
	testing::{TestDb, batch, event, grant},
};
use panel_core::{
	ids::{BrandId, LocationId},
	notify::Locale,
};
use sa_auth::{PermissionSet, SourceKind};
use serde_json::json;
use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions};

#[tokio::test]
async fn the_runtime_does_its_work_on_a_fresh_database() {
	let db = TestDb::create().await;
	Store::open(db.path()).await.expect("created and migrated");
	Store::open(db.path()).await.expect("again: nothing to do");
	{
		let key = DataKey::from_hex(&DataKey::generate_hex().unwrap()).unwrap();
		let panel = Panel::new(db.store().await, key);
		let ops = grant("aquafix-ops", SourceKind::Panel, &["aquafix"]);
		let now = Timestamp::now();
		let lead = json!({"brandId": "aquafix", "leadId": "L-1"});
		let events = [
			event("lead.created", now - SignedDuration::from_mins(5), "panel", lead.clone(), json!({"channel": "phone_inbound"})),
			event("call.logged", now, "panel", lead.clone(), json!({"outcome": "answered"})),
			event("payment.received", now, "panel", lead, json!({"billed": 100, "commission": 10, "currency": "EUR"})),
		];
		let got = panel.ingest(&ops, &batch("aquafix-ops", &events), now).await.unwrap();
		assert!(got.iter().all(|v| v.outcome == Outcome::Accepted { unregistered: false }), "{got:?}");
		assert_eq!(panel.rebuild_projections().await.unwrap().leads, 1);

		// What an operator does and reads.
		let by = Actor(uuid::Uuid::now_v7());
		let new_lead = || NewLead {
			brand: BrandId::parse("aquafix").unwrap(),
			location: LocationId::parse("paris-11").unwrap(),
			need: "a boiler".into(),
			phone: None,
			channel: panel_core::fact::LeadChannel::PhoneInbound,
		};
		let (lead, _) = panel.create_lead(by, new_lead(), now).await.unwrap();
		let brand = BrandId::parse("aquafix").unwrap();
		panel.attempt_call(by, &brand, &lead, now).await.unwrap();
		assert_eq!(panel.leads(&LeadQuery { limit: 10, ..LeadQuery::default() }, Pii::Reveal, now).await.unwrap().leads.len(), 2);
		assert!(panel.lead_card(&brand, &lead, Pii::Reveal, now).await.unwrap().is_some());
		let today = now.to_zoned(jiff::tz::TimeZone::UTC).date();
		assert_eq!(panel.funnel(today, today, None).await.unwrap().manual, 2);
		let payment = Payment {
			billed: 100,
			commission: 10,
			currency: "EUR".into(),
		};
		panel.record_payment(by, &brand, &lead, payment, now).await.unwrap();
		let slices = panel.funnel_slices(today, today, None, FunnelBy::Location).await.unwrap();
		assert_eq!(slices[0].payments[0].billed, 100);
		assert_eq!(panel.places().await.unwrap().len(), 1);
		assert_eq!(panel.lead_counts(Some(&brand), None, now).await.unwrap().overdue, 0);

		// By a colleague, so that the fan-out has leads of its own to tell.
		let colleague = Actor(uuid::Uuid::now_v7());
		for need in ["a tap", "a sink"] {
			panel.create_lead(colleague, NewLead { need: need.into(), ..new_lead() }, now).await.unwrap();
		}
		let appended = panel.fan_out(now, Locale::Ru).await.unwrap();
		assert!(appended > 0, "new leads and a payment to tell");
		assert_eq!(panel.fan_out(now, Locale::Ru).await.unwrap(), 0, "what was told is not told again");
		assert_eq!(panel.notifications(0, 10).await.unwrap().len(), appended);
		assert_eq!(panel.received_by_key().await.unwrap().len(), 1);

		let pool = panel.store().pool();
		for (sql, refusal) in [
			("DELETE FROM events", "events is append-only"),
			("UPDATE events SET brand_id = 'x'", "events is append-only"),
			("UPDATE events SET received_at = received_at + 1", "events is append-only"),
			("UPDATE leads SET last_event_id = x'00000000000000000000000000000000'", "FOREIGN KEY constraint failed"),
		] {
			let err = sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_err().to_string();
			assert!(err.contains(refusal), "{sql}: {err}");
		}
		sqlx::query("UPDATE events SET status = status, status_reason = 'x'")
			.execute(pool)
			.await
			.expect("status and its reason are the journal's one change");
		let n: i64 = sqlx::query_scalar("SELECT count(*) FROM reporting_leads").fetch_one(pool).await.unwrap();
		assert_eq!(n, 4, "the ingested lead and the three taken by phone");
		pool.close().await;
	}
}

/// A Telegram link and a link token made under roles carry, after the migration, exactly what
/// the role's alias held; and go back to the role on the way down.
#[tokio::test]
async fn telegram_roles_become_permissions_both_ways() {
	const PERMISSIONS: i64 = 20261006090000;
	// The aliases as the migration wrote them, frozen: the permissions have been re-spelled since.
	const OPERATOR_WHEN_WRITTEN: &[&str] = &["sa:analysis:read", "sa:work:leads:edit", "sa:work:pii:see", "sa:work:read"];
	const ADMIN_WHEN_WRITTEN: &[&str] = &[
		"sa:admin:sources:manage",
		"sa:analysis:experiments:edit",
		"sa:analysis:read",
		"sa:playbook:mcp:use",
		"sa:review_archive:archive:operate",
		"sa:review_archive:members:act_as",
		"sa:review_archive:tokens:grant",
		"sa:work:leads:edit",
		"sa:work:pii:see",
		"sa:work:places:edit",
		"sa:work:pricing:edit",
		"sa:work:read",
	];
	let db = TestDb::create().await;
	let migrator = Migrator::new(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations").as_path()).await.unwrap();
	let pool = SqlitePoolOptions::new().max_connections(1).connect_with(panel::store::options(db.path())).await.unwrap();
	migrator.run_to(PERMISSIONS - 1, &pool).await.unwrap();
	let (ann, bob) = (uuid::Uuid::from_u128(1), uuid::Uuid::from_u128(2));
	for (user, chat, role) in [(ann, 7, "operator"), (bob, 8, "admin")] {
		sqlx::query("INSERT INTO telegram_links (user_id, chat_id, linked_at, role, role_checked_at, display_name) VALUES ($1, $2, 0, $3, 5, 'x')")
			.bind(user)
			.bind(chat)
			.bind(role)
			.execute(&pool)
			.await
			.unwrap();
	}
	sqlx::query("INSERT INTO telegram_link_tokens (token_hash, user_id, role, display_name, expires_at) VALUES (zeroblob(32), $1, 'admin', 'x', 9)")
		.bind(bob)
		.execute(&pool)
		.await
		.unwrap();

	migrator.run_to(PERMISSIONS, &pool).await.unwrap();
	let held = |sql: &'static str, user: uuid::Uuid| {
		let pool = pool.clone();
		async move {
			let raw: String = sqlx::query_scalar(sql).bind(user).fetch_one(&pool).await.unwrap();
			serde_json::from_str::<PermissionSet>(&raw).unwrap()
		}
	};
	let alias = |a: &[&str]| a.iter().copied().collect::<PermissionSet>();
	let links = "SELECT permissions FROM telegram_links WHERE user_id = $1";
	assert_eq!(held(links, ann).await, alias(OPERATOR_WHEN_WRITTEN));
	assert_eq!(held(links, bob).await, alias(ADMIN_WHEN_WRITTEN));
	assert_eq!(held("SELECT permissions FROM telegram_link_tokens WHERE user_id = $1", bob).await, alias(ADMIN_WHEN_WRITTEN));
	let checked: i64 = sqlx::query_scalar("SELECT permissions_checked_at FROM telegram_links WHERE user_id = $1")
		.bind(ann)
		.fetch_one(&pool)
		.await
		.unwrap();
	assert_eq!(checked, 5);

	migrator.undo(&pool, PERMISSIONS - 1).await.unwrap();
	let roles: Vec<(uuid::Uuid, String)> = sqlx::query_as("SELECT user_id, role FROM telegram_links ORDER BY user_id").fetch_all(&pool).await.unwrap();
	assert_eq!(roles, [(ann, "operator".to_owned()), (bob, "admin".to_owned())]);
	let token: String = sqlx::query_scalar("SELECT role FROM telegram_link_tokens").fetch_one(&pool).await.unwrap();
	assert_eq!(token, "admin");
}
