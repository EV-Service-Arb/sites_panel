//! The bus the conductor's change feed reads: each write is told by the tables it touched, once it
//! has committed, and a write refused or rolled back tells nothing.

use std::collections::BTreeSet;

use jiff::Timestamp;
use panel::{
	operator::{Actor, StageMove},
	place::Expected,
	testing::{TestDb, batch, event, grant, panel},
};
use panel_core::{
	ids::{LeadId, LocationId},
	place::{Editor, PlaceSettings},
};
use sa_auth::SourceKind;
use sa_core::ids::BrandId;
use serde_json::json;
use uuid::Uuid;

/// The tables the bus told, drained once it has been quiet a moment: a connection tells its
/// writes when it goes back to the pool, which happens after the call that used it returns.
async fn told(rx: &mut tokio::sync::broadcast::Receiver<String>) -> std::collections::BTreeSet<String> {
	let mut out = std::collections::BTreeSet::new();
	while let Ok(topic) = tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
		out.insert(topic.expect("the test reads as fast as the bus is told"));
	}
	out
}

fn tables(names: &[&str]) -> BTreeSet<String> {
	names.iter().map(|t| format!("sites.{t}")).collect()
}

#[tokio::test]
async fn a_write_tells_the_tables_it_wrote_and_a_refused_one_nothing() {
	let db = TestDb::create().await;
	let panel = panel(&db).await;
	let mut rx = panel.bus().subscribe();
	let site = grant("aquafix-site", SourceKind::Site, &["aquafix"]);
	let now = Timestamp::now();

	told(&mut rx).await;
	let lead = json!({"brandId": "aquafix", "locationId": "royat", "leadId": "L-1"});
	let created = event("lead.created", now, "site", lead, json!({"channel": "form"}));
	panel.ingest(&site, &batch("aquafix-site", &[created]), now).await.unwrap();
	let first = told(&mut rx).await;
	assert!(first.is_superset(&tables(&["events", "leads"])), "{first:?}");

	let refused = event("lead.created", now, "site", json!({"brandId": "vifnet", "leadId": "V-1"}), json!({"channel": "form"}));
	panel.ingest(&site, &batch("aquafix-site", &[refused]), now).await.unwrap();
	assert_eq!(told(&mut rx).await, tables(&[]), "a brand the key may not write: nothing journaled");

	let by = Actor(Uuid::now_v7());
	let aquafix = BrandId::parse("aquafix").unwrap();
	let l1 = LeadId::parse("L-1").unwrap();
	panel.move_lead(by, &aquafix, &l1, StageMove::Contacted { channel: None }, now).await.unwrap();
	let moved = told(&mut rx).await;
	assert!(moved.is_superset(&tables(&["events", "leads"])), "an operator's move goes through the same journal: {moved:?}");

	let slug = LocationId::parse("royat").unwrap();
	let editor = Editor {
		id: Uuid::now_v7(),
		label: "admin@evinvest.ltd".into(),
	};
	panel.register_place(&editor, &aquafix, &slug, now).await.unwrap();
	told(&mut rx).await;
	let phone = PlaceSettings::parse(&json!({"phone": "+33423500640"})).unwrap();
	panel.set_place(&editor, &aquafix, &slug, phone, Expected::Any, now).await.unwrap();
	let placed = told(&mut rx).await;
	assert!(placed.is_superset(&tables(&["place_settings", "place_changes"])), "{placed:?}");
	assert!(!placed.contains("sites.leads"), "a place's change is not a lead's: {placed:?}");

	let stale = Expected::At(Some("2020-01-01T00:00:00Z".parse().unwrap()));
	assert!(panel.set_place(&editor, &aquafix, &slug, PlaceSettings::default(), stale, now).await.is_err());
	assert_eq!(told(&mut rx).await, tables(&[]), "a conflict rolls back, and tells nothing");
}
