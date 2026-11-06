//! The Service-Arb panel without I/O: no database, network, clock or randomness. Time
//! comes in as an argument.
//!
//! - [`ids`]: the typed identities events carry;
//! - [`event`]: the envelope of a `sa.funnel.v1` event, and what a signing key may write;
//! - [`fact`]: the registered event types, typed and checked;
//! - [`lead`]: a lead's stage and stage times, folded from its facts;
//! - [`experiment`]: a brand's experiments as its landing declares them and an admin sets them;
//! - [`funnel`]: shares no more precise than the data, and the contact SLA;
//! - [`place`]: a place's live settings (phones, hours, …), checked as the sites read them;
//! - [`pricing`]: a brand's price list, checked and priced to the cent;
//! - [`analytics`]: what PostHog is told of a lead's life, without PII;
//! - [`notify`]: what the conductor's Telegram is handed: the rules' names, the texts, the buttons;
//! - [`booking`]: the booking providers and a place's choice of them, and a lead's booking
//!   folded from its facts; [`phone`]: a phone number as the sites normalize it.

/// A closed set of words on the wire: the enum, its word per variant (`as_str`, serde and
/// ts-rs alike), `ALL` in order, and `parse`, which refuses any other word with `$refused`.
macro_rules! vocabulary {
	($(#[$meta:meta])* pub enum $ty:ident, $refused:literal { $($(#[$vmeta:meta])* $variant:ident => $wire:literal),+ $(,)? }) => {
		#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize, ts_rs::TS)]
		$(#[$meta])*
		pub enum $ty {
			$($(#[$vmeta])* #[serde(rename = $wire)] $variant,)+
		}

		impl $ty {
			pub const ALL: [Self; [$(stringify!($variant)),+].len()] = [$(Self::$variant),+];

			pub fn as_str(self) -> &'static str {
				match self {
					$(Self::$variant => $wire,)+
				}
			}

			pub fn parse(raw: &str) -> Result<Self, $crate::Invalid> {
				Self::ALL.into_iter().find(|v| v.as_str() == raw).ok_or_else(|| $crate::Invalid::new($refused))
			}
		}

		impl std::fmt::Display for $ty {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.write_str(self.as_str())
			}
		}
	};
}

pub mod analytics;
pub mod booking;
pub mod event;
pub mod experiment;
pub mod fact;
pub mod funnel;
pub mod ids;
pub mod invoice;
pub mod lead;
pub mod notify;
pub mod phone;
pub mod place;
pub mod pricing;

/// JavaScript's `Number.isSafeInteger`, on a JSON number: `100.0` and `1e2` are the integer
/// 100 there, as they are here — the sites read the contract's JSON with JavaScript.
pub(crate) fn safe_integer(v: &serde_json::Value) -> Option<i64> {
	const MAX_SAFE: i64 = (1 << 53) - 1;
	let serde_json::Value::Number(n) = v else { return None };
	if let Some(i) = n.as_i64() {
		return (-MAX_SAFE..=MAX_SAFE).contains(&i).then_some(i);
	}
	if n.is_u64() {
		return None;
	}
	let f = n.as_f64()?;
	// A float within ±2^53 with no fraction is exactly an integer, so the cast is exact.
	(f.fract() == 0.0 && f.abs() <= MAX_SAFE as f64).then_some(f as i64)
}

/// Why an event (or a part of one) cannot be accepted. The message is what the source is
/// told, so it names the field and never echoes a secret or PII.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct Invalid(pub String);

impl Invalid {
	pub fn new(msg: impl Into<String>) -> Self {
		Self(msg.into())
	}
}

impl From<sa_auth::UnknownKind> for Invalid {
	fn from(e: sa_auth::UnknownKind) -> Self {
		Self(e.to_string())
	}
}
