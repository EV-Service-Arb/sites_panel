//! The PostHog client against a mock capture API served in-process (axum on a local port —
//! never the network): the batch's shape, and which answers are retried and which refused.

use std::sync::{Arc, Mutex};

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use jiff::Timestamp;
use panel::capture::{Captured, Capturer, SendError};
use panel_server::capture::CaptureApi;
use serde_json::{Value, json};

#[derive(Clone, Default)]
struct Mock {
	status: Arc<Mutex<Option<StatusCode>>>,
	bodies: Arc<Mutex<Vec<Value>>>,
}

async fn batch(State(m): State<Mock>, Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
	m.bodies.lock().unwrap().push(body);
	let status = m.status.lock().unwrap().unwrap_or(StatusCode::OK);
	(status, Json(json!({"status": "Ok"})))
}

async fn serve(m: Mock) -> String {
	let app = Router::new().route("/batch/", post(batch)).with_state(m);
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
	let addr = listener.local_addr().unwrap();
	// Dropped with the test's runtime.
	let _server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
	format!("http://{addr}")
}

#[tokio::test]
async fn a_batch_as_posthog_takes_it() {
	let mock = Mock::default();
	let api = CaptureApi::new(&format!("{}/", serve(mock.clone()).await), "phc_test").unwrap();
	let id = uuid::Uuid::now_v7();
	let one = Captured {
		uuid: id,
		event: "sa_lead_created".into(),
		distinct_id: "sa-lead:aquafix:L-1".into(),
		properties: json!({"brand_id": "aquafix", "channel": "form"}),
		timestamp: "2026-10-04T10:00:00Z".parse::<Timestamp>().unwrap(),
	};
	api.send(std::slice::from_ref(&one)).await.unwrap();
	let body = mock.bodies.lock().unwrap()[0].clone();
	assert_eq!(
		body,
		json!({"api_key": "phc_test", "batch": [{
			"event": "sa_lead_created",
			"distinct_id": "sa-lead:aquafix:L-1",
			"uuid": id.to_string(),
			"timestamp": "2026-10-04T10:00:00Z",
			"properties": {"brand_id": "aquafix", "channel": "form", "$lib": "sa-panel"},
		}]})
	);
	for (status, retry) in [
		(StatusCode::SERVICE_UNAVAILABLE, true),
		(StatusCode::TOO_MANY_REQUESTS, true),
		(StatusCode::BAD_REQUEST, false),
		(StatusCode::UNAUTHORIZED, false),
	] {
		*mock.status.lock().unwrap() = Some(status);
		let got = api.send(std::slice::from_ref(&one)).await.unwrap_err();
		assert_eq!(matches!(got, SendError::Retry(_)), retry, "{status}: {got}");
	}
	let nobody = CaptureApi::new("http://127.0.0.1:1", "phc_test").unwrap();
	assert!(matches!(nobody.send(&[one]).await, Err(SendError::Retry(_))), "no answer is retried");
}
