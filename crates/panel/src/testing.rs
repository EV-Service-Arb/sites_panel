//! For tests only (feature `testing`): a throwaway database per test, batches as a source
//! writes them, and the conductor's assertions under a test key.
//!
//! Each test gets its own SQLite file in the system's temp directory, removed when the
//! [`TestDb`] is dropped: no server, nothing to set up, so the database tests run everywhere
//! `cargo test` does, CI included. A file, not `:memory:`: an in-memory database is one per
//! connection, and the tests open several pools on one database, as replicas would.

use std::path::{Path, PathBuf};

use jiff::Timestamp;
use panel_core::{event::KeyGrant, ids::BrandId};
use sa_auth::{Assertion, Keys, Principal, Service, Signer, Source, SourceKind, User};
use serde_json::{Value, json};
use sqlx::sqlite::SqlitePoolOptions;
use uuid::Uuid;

use crate::{
	Panel,
	seal::DataKey,
	store::{self, Store},
};

/// A database that lives as long as this value.
pub struct TestDb {
	path: PathBuf,
}

impl TestDb {
	/// A fresh, empty database.
	pub async fn create() -> Self {
		let path = std::env::temp_dir().join(format!("panel_test_{}.db", Uuid::now_v7().simple()));
		Self { path }
	}

	pub fn path(&self) -> &Path {
		&self.path
	}

	/// A store on this database, migrated (opening migrates).
	pub async fn store(&self) -> Store {
		Store::open(&self.path).await.expect("opening the test database")
	}

	/// A plain pool on this database, for assertions the engine has no API for. Migrated
	/// first, so it can be asked for before any store.
	pub async fn pool(&self) -> sqlx::SqlitePool {
		drop(self.store().await);
		SqlitePoolOptions::new()
			.max_connections(2)
			.connect_with(store::options(&self.path))
			.await
			.expect("connecting to the test database")
	}
}

impl Drop for TestDb {
	fn drop(&mut self) {
		// The WAL and its index sit beside the file; a pool still open only keeps them a while.
		for suffix in ["", "-wal", "-shm"] {
			let mut path = self.path.clone().into_os_string();
			path.push(suffix);
			match std::fs::remove_file(&path) {
				Ok(()) => {}
				Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
				Err(e) => eprintln!("could not remove test database {}: {e}", PathBuf::from(path).display()),
			}
		}
	}
}

/// A panel on a fresh store with a fresh data key.
pub async fn panel(db: &TestDb) -> Panel {
	let key = DataKey::from_hex(&DataKey::generate_hex().expect("random")).expect("a fresh key");
	Panel::new(db.store().await, key)
}

/// `{"events": events}`, as a source sends it: each event's `source.id` set to the key id,
/// which is what a source is called.
pub fn batch(key_id: &str, events: &[Value]) -> Vec<u8> {
	let events: Vec<Value> = events
		.iter()
		.cloned()
		.map(|mut e| {
			if let Some(source) = e.get_mut("source").and_then(Value::as_object_mut) {
				source.insert("id".to_owned(), json!(key_id));
			}
			e
		})
		.collect();
	batch_verbatim(&events)
}

/// [`batch`], leaving the events exactly as given.
pub fn batch_verbatim(events: &[Value]) -> Vec<u8> {
	serde_json::to_vec(&json!({ "events": events })).expect("JSON")
}

/// What the conductor's assertion says a key may write.
pub fn grant(key_id: &str, kind: SourceKind, brands: &[&str]) -> KeyGrant {
	KeyGrant {
		key_id: key_id.to_owned(),
		kind,
		brands: brands.iter().map(|b| BrandId::parse(b).expect("a brand id")).collect(),
	}
}

/// The conductor, as far as sites sees it: a signing key, and the assertions it makes.
pub struct Conductor {
	signer: Signer,
}

impl Conductor {
	/// A fresh key under `kid`.
	pub fn new(kid: &str) -> Self {
		use base64::Engine;
		let mut seed = [0u8; 32];
		getrandom::fill(&mut seed).expect("random");
		let signer = format!("{kid}:{}", base64::engine::general_purpose::STANDARD.encode(seed)).parse().expect("a signing key");
		Self { signer }
	}

	/// What sites is configured with to trust this conductor (`PANEL_ASSERTION_KEYS`).
	pub fn keys(&self) -> Keys {
		self.signer.public().parse().expect("its own public key")
	}

	pub fn sign(&self, aud: Service, principal: Principal, method: &str, path: &str) -> String {
		self.signer.sign(&Assertion {
			aud,
			principal,
			method: method.to_owned(),
			path: path.to_owned(),
			exp: Timestamp::now().as_second() + sa_auth::TTL,
		})
	}

	/// A signed-in user `sub` holding `permissions` (the `sa:sites:` ones are kept), for one request.
	pub fn user(&self, sub: Uuid, permissions: &sa_auth::PermissionSet, method: &str, path: &str) -> String {
		let user = User {
			sub: sub.to_string(),
			email: format!("{}@example.com", sub.simple()),
			email_verified: true,
			name: "Olga".to_owned(),
			permissions: permissions.iter().filter(|p| p.starts_with(Service::Sites.prefix())).collect(),
		};
		self.sign(Service::Sites, Principal::User(user), method, path)
	}

	/// A source key the conductor checked the MAC of, over `body`.
	pub fn source(&self, key_id: &str, kind: SourceKind, brands: &[&str], method: &str, path: &str, body: &[u8]) -> String {
		let source = Source::new(key_id.to_owned(), kind, brands.iter().map(|b| (*b).to_owned()).collect(), body);
		self.sign(Service::Sites, Principal::Source(source), method, path)
	}

	/// The conductor itself, reading for its hubs.
	pub fn conductor(&self, method: &str, path: &str) -> String {
		self.sign(Service::Sites, Principal::Conductor, method, path)
	}
}

/// A messenger lead as a landing sends it: `lead.created` with its channel and ref, the
/// customer's need and locality (and a phone and a name, which a lookup must never give back)
/// in its PII.
pub fn messenger_lead(occurred_at: Timestamp, brand: &str, lead: &str, channel: &str, message_ref: &str) -> Value {
	let mut e = event(
		"lead.created",
		occurred_at,
		"site",
		json!({"brandId": brand, "locationId": "royat", "leadId": lead}),
		json!({"channel": channel, "messageRef": message_ref}),
	);
	e["pii"] = json!({"need": "fuite sous l'évier", "locality": "Royat", "phone": "+33612345678", "name": "Jean Dupont"});
	e
}

/// An event as a source writes it, with a fresh id.
pub fn event(r#type: &str, occurred_at: Timestamp, kind: &str, subject: Value, properties: Value) -> Value {
	json!({
		"id": Uuid::now_v7().to_string(),
		"schema": "sa.funnel.v1",
		"type": r#type,
		"typeVersion": 1,
		"occurredAt": occurred_at.to_string(),
		"source": {"kind": kind, "id": "test"},
		"subject": subject,
		"properties": properties,
	})
}

/// What `sa:operator` holds.
pub fn operator() -> sa_auth::PermissionSet {
	sa_auth::SA_OPERATOR.members.iter().copied().collect()
}

/// What `sa:admin` holds.
pub fn admin() -> sa_auth::PermissionSet {
	sa_auth::SA_ADMIN.members.iter().copied().collect()
}
