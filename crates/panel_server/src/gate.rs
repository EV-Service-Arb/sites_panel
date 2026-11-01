//! Who is asking: the conductor's assertion (`sa_auth`, header `x-sa-assertion`) on every route
//! but `/api/internal`, `/api/hooks`, `/mfe` and `/health`. sites trusts nothing else. Any
//! refusal is `401 {"error": "unauthenticated"}`; the log says why.

use std::sync::Arc;

use axum::{
	Json,
	extract::{OriginalUri, Request, State},
	http::{HeaderMap, Method},
	middleware::Next,
	response::{IntoResponse, Response},
};
use panel_core::{event::KeyGrant, ids::BrandId};
use sa_auth::{Keys, PermissionSet, Principal, Service};
use serde_json::json;
use uuid::Uuid;

use crate::http::Section;

/// The signed-in user of an `/api/v1` request, as the conductor vouched for them; put in the
/// request's extensions by `user`.
#[derive(Clone, Debug)]
pub struct Caller {
	pub user_id: Uuid,
	pub email: String,
	/// Their `sa:sites:*` permissions, and no others.
	pub permissions: PermissionSet,
}

pub(crate) fn unauthenticated() -> Response {
	(axum::http::StatusCode::UNAUTHORIZED, Json(json!({ "error": "unauthenticated" }))).into_response()
}

/// The assertion on this request, verified for `method` and `path` (and `body`, for a source).
fn verify(keys: &Keys, headers: &HeaderMap, method: &Method, path: &str, body: Option<&[u8]>) -> Option<Principal> {
	let token = headers.get(sa_auth::HEADER)?.to_str().ok()?;
	match sa_auth::verify(keys, token, Service::Sites, method.as_str(), path, body, jiff::Timestamp::now().as_second()) {
		Ok(a) => Some(a.principal),
		Err(e) => {
			tracing::warn!(%method, path, reason = %e, "an assertion refused");
			None
		}
	}
}

/// The path as the conductor asked for it: a nested router's middleware sees it stripped.
fn asked_path(req: &Request) -> String {
	req.extensions().get::<OriginalUri>().expect("the router records the URI it was asked").path().to_owned()
}

/// The conductor's public keys, and the section a route sits under (`None`: any signed-in user).
#[derive(Clone)]
pub(crate) struct UserGate {
	pub keys: Arc<Keys>,
	pub section: Option<Section>,
}

/// `/api/v1`: a user's assertion, then the route's section (`403` without it).
pub(crate) async fn user(State(gate): State<UserGate>, mut req: Request, next: Next) -> Response {
	let principal = verify(&gate.keys, req.headers(), req.method(), &asked_path(&req), None);
	let Some(Principal::User(user)) = principal else {
		return unauthenticated();
	};
	let Ok(user_id) = Uuid::parse_str(&user.sub) else {
		tracing::warn!(sub = user.sub, "an assertion's sub is not a user id");
		return unauthenticated();
	};
	if gate.section.is_some_and(|s| !s.may(&user.permissions)) {
		return crate::api::ApiError::Forbidden.into_response();
	}
	req.extensions_mut().insert(Caller {
		user_id,
		email: user.email,
		permissions: user.permissions,
	});
	next.run(req).await
}

/// `/api/conductor`: the conductor's own assertion.
pub(crate) async fn conductor(State(keys): State<Arc<Keys>>, req: Request, next: Next) -> Response {
	match verify(&keys, req.headers(), req.method(), &asked_path(&req), None) {
		Some(Principal::Conductor) => next.run(req).await,
		_ => unauthenticated(),
	}
}

/// Ingest: whether the request carries an assertion at all, before its body is read.
pub(crate) fn asserted(headers: &HeaderMap) -> bool {
	headers.contains_key(sa_auth::HEADER)
}

/// Ingest: the key the conductor checked the MAC of over `body`, as what it may write. A kind no
/// key is issued for, or a brand that cannot be one, is refused like a bad assertion.
pub(crate) fn source(keys: &Keys, headers: &HeaderMap, method: &Method, path: &str, body: &[u8]) -> Option<KeyGrant> {
	let Some(Principal::Source(source)) = verify(keys, headers, method, path, Some(body)) else {
		return None;
	};
	if !source.kind.keyed() {
		tracing::warn!(key_id = source.key_id, kind = %source.kind, "a source assertion of a kind no key is issued for");
		return None;
	}
	let brands = match source.brands.iter().map(|b| BrandId::parse(b)).collect::<Result<_, _>>() {
		Ok(b) => b,
		Err(e) => {
			tracing::warn!(key_id = source.key_id, error = %e, "a source assertion names a brand that cannot be one");
			return None;
		}
	};
	Some(KeyGrant {
		key_id: source.key_id,
		kind: source.kind,
		brands,
	})
}
