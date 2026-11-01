//! For tests only (feature `testing`): the real router, called as the conductor calls it —
//! every request carries the assertion it would sign for it.

use axum::{
	Router,
	body::{Body, to_bytes},
	http::{Method, Request, StatusCode, header},
};
use panel::{Panel, booking::PushSources, testing::Conductor};
use panel_core::notify::Locale;
use sa_auth::{PermissionSet, SourceKind};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use crate::http::{self, Limits, Sites};

/// sites_panel as `serve` builds it, trusting `conductor`.
pub fn app(panel: Panel, conductor: &Conductor) -> Router {
	app_with(panel, conductor, Limits::default(), PushSources::default())
}

pub fn app_with(panel: Panel, conductor: &Conductor, limits: Limits, hooks: PushSources) -> Router {
	let sites = Sites {
		panel,
		keys: conductor.keys(),
		locale: Locale::Ru,
		hooks,
		mfe: None,
	};
	http::router(sites, limits)
}

/// What a request is signed as.
#[derive(Clone)]
pub enum As {
	/// A signed-in user holding these permissions.
	User(Uuid, PermissionSet),
	/// A source key, over the request's body.
	Source {
		key_id: String,
		kind: SourceKind,
		brands: Vec<String>,
	},
	Conductor,
	/// No assertion at all.
	Nobody,
}

impl As {
	/// A fresh user holding `permissions`.
	pub fn user(permissions: PermissionSet) -> Self {
		Self::User(Uuid::now_v7(), permissions)
	}

	pub fn source(key_id: &str, kind: SourceKind, brands: &[&str]) -> Self {
		Self::Source {
			key_id: key_id.to_owned(),
			kind,
			brands: brands.iter().map(|b| (*b).to_owned()).collect(),
		}
	}
}

/// One request: `uri` signed as `who` (the assertion binds its path without the query), with
/// `headers` and a raw `body`.
pub async fn raw(app: &Router, conductor: &Conductor, who: &As, method: Method, uri: &str, headers: &[(&str, &str)], body: Vec<u8>) -> (StatusCode, Vec<u8>) {
	let path = uri.split_once('?').map_or(uri, |(p, _)| p);
	let mut req = Request::builder().method(method.clone()).uri(uri);
	let assertion = match who {
		As::User(sub, permissions) => Some(conductor.user(*sub, permissions, method.as_str(), path)),
		As::Source { key_id, kind, brands } => {
			let brands: Vec<&str> = brands.iter().map(String::as_str).collect();
			Some(conductor.source(key_id, *kind, &brands, method.as_str(), path, &body))
		}
		As::Conductor => Some(conductor.conductor(method.as_str(), path)),
		As::Nobody => None,
	};
	if let Some(a) = assertion {
		req = req.header(sa_auth::HEADER, a);
	}
	for (k, v) in headers {
		req = req.header(*k, *v);
	}
	let res = app.clone().oneshot(req.body(Body::from(body)).unwrap()).await.unwrap();
	let status = res.status();
	(status, to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec())
}

/// [`raw`] with a JSON body (or none) and a JSON answer (`null` when it is not JSON).
pub async fn send(app: &Router, conductor: &Conductor, who: &As, method: Method, uri: &str, body: Option<Value>, headers: &[(&str, &str)]) -> (StatusCode, Value) {
	let mut headers = headers.to_vec();
	let bytes = match body {
		Some(b) => {
			headers.push((header::CONTENT_TYPE.as_str(), "application/json"));
			b.to_string().into_bytes()
		}
		None => Vec::new(),
	};
	let (status, bytes) = raw(app, conductor, who, method, uri, &headers, bytes).await;
	(status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// A signed-in user's calls, through one helper per method.
pub struct Client<'a> {
	pub app: &'a Router,
	pub conductor: &'a Conductor,
	pub who: As,
}

impl Client<'_> {
	pub async fn get(&self, uri: &str) -> (StatusCode, Value) {
		send(self.app, self.conductor, &self.who, Method::GET, uri, None, &[]).await
	}

	pub async fn post(&self, uri: &str, body: Value) -> (StatusCode, Value) {
		send(self.app, self.conductor, &self.who, Method::POST, uri, Some(body), &[]).await
	}

	pub async fn put(&self, uri: &str, body: Value) -> (StatusCode, Value) {
		send(self.app, self.conductor, &self.who, Method::PUT, uri, Some(body), &[]).await
	}

	pub async fn delete(&self, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
		send(self.app, self.conductor, &self.who, Method::DELETE, uri, body, &[]).await
	}

	pub async fn send(&self, method: Method, uri: &str, body: Option<Value>, headers: &[(&str, &str)]) -> (StatusCode, Value) {
		send(self.app, self.conductor, &self.who, method, uri, body, headers).await
	}
}

/// What `sa:<alias>` holds, as the catalog says.
pub fn alias(name: &str) -> PermissionSet {
	sa_auth::Catalog::collect("sa", 0).aliases[name].iter().cloned().collect()
}
