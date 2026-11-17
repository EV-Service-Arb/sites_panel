//! The `/api/v1` operations as the conductor pulls them, against the human overview of them in
//! docs/ARCHITECTURE.md.

use std::collections::BTreeSet;

use axum::{
	Router,
	body::{Body, to_bytes},
	http::{Request, StatusCode, header},
};
use panel::testing::{Conductor, TestDb, panel};
use panel_server::testing::{As, app, send};
use serde_json::Value;
use tower::ServiceExt;

const DOCUMENT: &str = "/api/v1/openapi.json";

/// `{a,b}` in a path is two paths; `{id}` is a parameter.
fn expand(path: &str) -> Vec<String> {
	let mut at = 0;
	while let Some(open) = path[at..].find('{').map(|i| at + i) {
		let close = open + path[open..].find('}').expect("a brace closes");
		if path[open..close].contains(',') {
			return path[open + 1..close]
				.split(',')
				.flat_map(|alt| expand(&format!("{}{alt}{}", &path[..open], &path[close + 1..])))
				.collect();
		}
		at = close;
	}
	vec![path.to_owned()]
}

/// The document as the conductor asks for it, `if_none_match` its ETag when it has a copy.
async fn pull(app: &Router, conductor: &Conductor, if_none_match: Option<&str>) -> (StatusCode, Option<String>, Vec<u8>) {
	let mut req = Request::get(DOCUMENT).header(sa_auth::HEADER, conductor.conductor("GET", DOCUMENT));
	if let Some(etag) = if_none_match {
		req = req.header(header::IF_NONE_MATCH, etag);
	}
	let res = app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
	let etag = res.headers().get(header::ETAG).map(|v| v.to_str().unwrap().to_owned());
	(res.status(), etag, to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec())
}

#[tokio::test]
async fn the_conductor_pulls_the_document_of_every_operation_the_overview_names() {
	let db = TestDb::create().await;
	let conductor = Conductor::new("k1");
	let app = app(panel(&db).await, &conductor);

	let (status, etag, body) = pull(&app, &conductor, None).await;
	assert_eq!(status, StatusCode::OK);
	let etag = etag.expect("the document carries an ETag");
	let doc: Value = serde_json::from_slice(&body).unwrap();
	assert_eq!(pull(&app, &conductor, Some(&etag)).await.0, StatusCode::NOT_MODIFIED, "the copy it has");

	let user = As::user(Vec::<&str>::new().into_iter().collect());
	let (status, as_user) = send(&app, &conductor, &user, axum::http::Method::GET, DOCUMENT, None, &[]).await;
	assert_eq!((status, &as_user), (StatusCode::OK, &doc), "the whole of it, whoever asks");
	assert_eq!(
		send(&app, &conductor, &As::Nobody, axum::http::Method::GET, DOCUMENT, None, &[]).await.0,
		StatusCode::UNAUTHORIZED
	);
	assert_eq!(
		doc["paths"]["/leads"]["post"]["x-permission"], "sa:sites:work:leads:edit",
		"an action's own permission, not only its section's"
	);

	let arch = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/ARCHITECTURE.md")).unwrap();
	let section = &arch[arch.find("## The operator API, `/api/v1`").expect("the section")..];
	let block = &section[section.find("```text\n").expect("its block") + 8..];
	let block = &block[..block.find("```").expect("closed")];
	let overview: BTreeSet<(String, String)> = block
		.lines()
		.flat_map(|l| {
			let mut words = l.split_whitespace();
			let method = words.next().expect("a method").to_owned();
			expand(words.next().expect("a path")).into_iter().map(move |p| (method.clone(), p))
		})
		.collect();
	let table: BTreeSet<(String, String)> = doc["paths"]
		.as_object()
		.unwrap()
		.iter()
		.flat_map(|(path, item)| item.as_object().unwrap().keys().map(move |m| (m.to_uppercase(), path.clone())))
		.collect();
	assert_eq!(overview, table);
}
