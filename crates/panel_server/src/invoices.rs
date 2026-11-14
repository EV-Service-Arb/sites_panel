//! Invoices over HTTP (`panel::invoices`): an operator issues one through banking's ScopeLedger
//! and opens its pay page for the customer; the payment is confirmed in the background
//! ([`crate::scope_ledger::run`]).

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::Method};
use ev_lib::api::{JsonSchema, Upserted};
use jiff::Timestamp;
use panel::{
	Panel,
	invoices::{Invoice, InvoiceError, LedgerError, NewInvoice},
	operator::Actor,
};
use panel_core::invoice::{CURRENCY, InvoiceState};
use sa_auth::Invoices;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
	api::{ApiError, ApiResult, Body, Ids, RequiredIdempotencyKey, recorded},
	gate::Caller,
	http::{ApiRoute, Gate, Section},
	scope_ledger::ScopeLedger,
};

/// The ledger, when the panel is configured for one (`SCOPE_LEDGER_*`).
type MaybeLedger = Option<Extension<Arc<ScopeLedger>>>;

pub(crate) fn routes() -> Vec<ApiRoute> {
	use Method as M;
	let work = Gate::section(Section::Work);
	vec![
		ApiRoute::new(M::GET, "/invoices", work, list_invoices),
		ApiRoute::new(M::GET, "/invoices/{id}", work, get_invoice),
		ApiRoute::new(M::POST, "/invoices", work.may(Invoices::Create), create_invoice),
	]
}

fn ledger(l: MaybeLedger) -> ApiResult<Arc<ScopeLedger>> {
	l.map(|Extension(l)| l).ok_or_else(|| ApiError::Unavailable("invoices_unavailable".into()))
}

impl From<InvoiceError> for ApiError {
	fn from(e: InvoiceError) -> Self {
		match e {
			InvoiceError::Invalid(e) => Self::BadRequest(e.0),
			InvoiceError::Conflict(m) => Self::Conflict(m.to_owned()),
			InvoiceError::NotFound => Self::NotFound,
			InvoiceError::Ledger(e) => e.into(),
			InvoiceError::Internal(e) => Self::Internal(e),
		}
	}
}

impl From<LedgerError> for ApiError {
	fn from(e: LedgerError) -> Self {
		match e {
			// A grant banking has not given the panel is configuration, not the operator's doing.
			LedgerError::Refused(reason) if reason == "not_granted" => Self::Unavailable(reason),
			LedgerError::Refused(reason) | LedgerError::Conflict(reason) => Self::Conflict(reason),
			LedgerError::Invalid(said) => Self::BadRequest(said),
			// Every invoice the panel asks of banking is one it stored: banking not knowing it
			// is banking's state gone wrong, not the caller's 404.
			LedgerError::NotFound => Self::Internal(eyre::eyre!("the ledger does not know an invoice the panel stored")),
			LedgerError::Unavailable(said) => {
				tracing::warn!(reason = said, "the ScopeLedger could not be reached");
				Self::Unavailable("ledger_unavailable".into())
			}
			LedgerError::Protocol(said) => Self::Internal(eyre::eyre!("the ScopeLedger broke its contract: {said}")),
		}
	}
}

/// An invoice the panel asked banking for.
#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Invoice")]
struct InvoiceDto {
	id: Uuid,
	/// Banking's; null while `creating`.
	invoice_id: Option<String>,
	/// Canonical decimal USDT.
	amount: String,
	/// USDT.
	currency: &'static str,
	memo: String,
	state: InvoiceState,
	/// Banking's page where the payer pays; null while `creating`.
	pay_url: Option<String>,
	/// The concierge user who issued it.
	created_by: Uuid,
	created_at: Timestamp,
	expires_at: Timestamp,
	paid_at: Option<Timestamp>,
	/// The payer's `auth_subject` at banking.
	paid_by: Option<String>,
}

impl From<Invoice> for InvoiceDto {
	fn from(i: Invoice) -> Self {
		Self {
			id: i.id,
			invoice_id: i.invoice_id,
			amount: i.amount,
			currency: CURRENCY,
			memo: i.memo,
			state: i.state,
			pay_url: i.pay_url,
			created_by: i.created_by,
			created_at: i.created_at,
			expires_at: i.expires_at,
			paid_at: i.paid.as_ref().map(|p| p.at),
			paid_by: i.paid.map(|p| p.by),
		}
	}
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "NewInvoice")]
struct CreateBody {
	/// Decimal USDT as a string, a JSON number would round it: whole USDT up to 999 999 999, at
	/// most 6 decimals.
	amount: String,
	/// At most 140 characters; banking refuses links.
	#[serde(default)]
	memo: String,
	/// 1 to 30.
	expires_in_days: u32,
}

/// Issues an invoice through banking's ScopeLedger, payable on its pay page.
///
/// Stored `creating` with its terms, then asked of banking: a refusal drops it, a lost answer
/// keeps it for the retry, which is answered 200 with the same invoice.
///
/// The Idempotency-Key is required: a lost answer retried without one would bill twice. 400 for
/// what was typed or banking found wrong with it; 409 for a key used for other terms, or
/// banking's reason (`invoice_not_open`, `scope_frozen`, …); 503 `invoices_unavailable` without
/// a ledger configured, banking's `not_granted`, or `ledger_unavailable`.
async fn create_invoice(
	State(panel): State<Panel>,
	Extension(caller): Extension<Caller>,
	l: MaybeLedger,
	RequiredIdempotencyKey(key): RequiredIdempotencyKey,
	Body(b): Body<CreateBody>,
) -> ApiResult<Upserted<InvoiceDto>> {
	let ledger = ledger(l)?;
	let new = NewInvoice {
		amount: b.amount,
		memo: b.memo,
		expires_in_days: b.expires_in_days,
	};
	let done = panel.issue_invoice_once(&*ledger, Actor(caller.user_id), new, &key, Timestamp::now()).await?;
	Ok(recorded(done, InvoiceDto::from))
}

#[derive(JsonSchema, Serialize)]
#[schemars(rename = "Invoices")]
struct InvoicesDto {
	invoices: Vec<InvoiceDto>,
}

/// The last 500 invoices, newest first, as the panel last stored them: banking is not asked.
async fn list_invoices(State(panel): State<Panel>) -> ApiResult<Json<InvoicesDto>> {
	Ok(Json(InvoicesDto {
		invoices: panel.invoices().await?.into_iter().map(InvoiceDto::from).collect(),
	}))
}

/// `{id}` of an invoice: the panel's.
#[derive(Deserialize, JsonSchema)]
struct InvoicePath {
	id: Uuid,
}

/// One invoice, as banking has it now; a final one (paid, cancelled, expired) as stored.
async fn get_invoice(State(panel): State<Panel>, l: MaybeLedger, Ids(InvoicePath { id }): Ids<InvoicePath>) -> ApiResult<Json<InvoiceDto>> {
	let ledger = ledger(l)?;
	Ok(Json(panel.refresh_invoice(&*ledger, id).await?.into()))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn status(e: impl Into<ApiError>) -> u16 {
		axum::response::IntoResponse::into_response(e.into()).status().as_u16()
	}

	#[test]
	fn banking_s_answers_map_to_the_contract() {
		assert_eq!(status(LedgerError::Refused("not_granted".into())), 503);
		assert_eq!(status(LedgerError::Refused("scope_frozen".into())), 409);
		assert_eq!(status(LedgerError::Conflict("idempotency_conflict".into())), 409);
		assert_eq!(status(LedgerError::Invalid("memo has a link".into())), 400);
		assert_eq!(status(LedgerError::Unavailable("down".into())), 503);
		assert_eq!(status(InvoiceError::NotFound), 404);
	}
}
