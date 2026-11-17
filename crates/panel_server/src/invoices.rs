//! Invoices over HTTP (`panel::invoices`): an operator issues one through banking's ScopeLedger
//! and opens its pay page for the customer; the payment is confirmed in the background
//! ([`crate::scope_ledger::run`]).

use std::sync::Arc;

use axum::{Extension, Json, extract::State, http::Method};
use ev_lib::api::{Body, Ids, JsonSchema, Refusal, Refuses, Upserted};
use jiff::Timestamp;
use panel::{
	Panel,
	invoices::{Invoice, InvoiceError, LedgerError, NewInvoice},
	operator::Actor,
};
use panel_core::invoice::{AMOUNT_PATTERN, CURRENCY, InvoiceState, MAX_EXPIRES_IN_DAYS, MAX_MEMO_CHARS};
use sa_auth::{Gate, Invoices, Work};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
	api::{ApiResult, Caller, Code, RequiredIdempotencyKey, recorded},
	http::ApiRoute,
	scope_ledger::ScopeLedger,
};

/// The ledger, when the panel is configured for one (`SCOPE_LEDGER_*`).
type MaybeLedger = Option<Extension<Arc<ScopeLedger>>>;

pub(crate) fn routes() -> Vec<ApiRoute> {
	use Method as M;
	let work = Gate::USER.needs(Work::Read);
	vec![
		ApiRoute::new(M::GET, "/invoices", work, list_invoices),
		ApiRoute::new(M::GET, "/invoices/{id}", work, get_invoice),
		ApiRoute::new(M::POST, "/invoices", Gate::USER.needs(Invoices::Create), create_invoice),
	]
}

fn ledger(l: MaybeLedger) -> ApiResult<Arc<ScopeLedger>> {
	l.map(|Extension(l)| l)
		.ok_or_else(|| Refusal::new(Code::InvoicesUnavailable, "no banking ScopeLedger is configured"))
}

impl Refuses<InvoiceError> for Code {
	fn refusal(e: InvoiceError) -> Refusal<Self> {
		match e {
			InvoiceError::Invalid(e) => Self::refusal(e),
			InvoiceError::KeyTaken => Refusal::new(Self::IdempotencyKeyTaken, e.to_string()),
			InvoiceError::NotFound => Refusal::new(Self::NotFound, "not found"),
			InvoiceError::Ledger(e) => Self::refusal(e),
			InvoiceError::Internal(e) => Self::refusal(e),
		}
	}
}

impl Refuses<LedgerError> for Code {
	fn refusal(e: LedgerError) -> Refusal<Self> {
		match e {
			// Banking's machine reasons: the ones the panel acts on are words of its own; the rest
			// are banking's vocabulary, which grows without the panel.
			LedgerError::Refused(reason) | LedgerError::Conflict(reason) => {
				let code = match reason.as_str() {
					// A grant banking has not given the panel is configuration, not the operator's doing.
					"not_granted" => Self::NotGranted,
					"invoice_not_open" => Self::InvoiceNotOpen,
					"invoice_processing" => Self::InvoiceProcessing,
					"scope_frozen" => Self::ScopeFrozen,
					"idempotency_conflict" => Self::IdempotencyConflict,
					_ => Self::LedgerRefused,
				};
				Refusal::new(code, reason)
			}
			LedgerError::Invalid(said) => Refusal::new(Self::BadRequest, said),
			// Every invoice the panel asks of banking is one it stored: banking not knowing it
			// is banking's state gone wrong, not the caller's 404.
			LedgerError::NotFound => Self::refusal(eyre::eyre!("the ledger does not know an invoice the panel stored")),
			LedgerError::Unavailable(said) => {
				tracing::warn!(reason = said, "the ScopeLedger could not be reached");
				Refusal::new(Self::LedgerUnavailable, "banking could not be reached")
			}
			LedgerError::Protocol(said) => Self::refusal(eyre::eyre!("the ScopeLedger broke its contract: {said}")),
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
	#[schemars(regex(pattern = AMOUNT_PATTERN))]
	amount: String,
	/// Banking refuses links.
	#[serde(default)]
	#[schemars(length(max = MAX_MEMO_CHARS))]
	memo: String,
	#[schemars(range(min = 1, max = MAX_EXPIRES_IN_DAYS))]
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
	caller: Caller,
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
	use serde_json::{Value, json};

	use super::*;

	fn answer(e: impl Into<Refusal<Code>>) -> (u16, Value) {
		let refusal: Refusal<Code> = e.into();
		let body = serde_json::to_value(&refusal).unwrap();
		(axum::response::IntoResponse::into_response(refusal).status().as_u16(), body["code"].clone())
	}

	#[test]
	fn banking_s_answers_map_to_the_contract() {
		assert_eq!(answer(LedgerError::Refused("not_granted".into())), (503, json!("not_granted")));
		assert_eq!(answer(LedgerError::Refused("scope_frozen".into())), (409, json!("scope_frozen")));
		assert_eq!(answer(LedgerError::Refused("a_new_reason".into())), (409, json!("ledger_refused")));
		assert_eq!(answer(LedgerError::Conflict("idempotency_conflict".into())), (409, json!("idempotency_conflict")));
		assert_eq!(answer(LedgerError::Invalid("memo has a link".into())), (400, json!("bad_request")));
		assert_eq!(answer(LedgerError::Unavailable("down".into())), (503, json!("ledger_unavailable")));
		assert_eq!(answer(InvoiceError::NotFound), (404, json!("not_found")));
	}
}
