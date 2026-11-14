//! Identities. Events are keyed by the UUIDv7 their source issues; everything else a
//! source names (brand, location, lead, job) is an opaque string the panel only checks
//! the shape of.

use std::fmt;

use ev_lib::architecture::Id;
use uuid::Uuid;

use crate::Invalid;

pub struct EventTag;
/// The event's idempotency key, a UUIDv7 issued by the source.
pub type EventId = Id<EventTag, Uuid>;

/// Parses an event id: a UUID, version 7 (time-ordered, so the journal's key follows time).
pub fn parse_event_id(raw: &str) -> Result<EventId, Invalid> {
	let id = Uuid::try_parse(raw).map_err(|_| Invalid::new("id is not a UUID"))?;
	if id.get_version_num() != 7 {
		return Err(Invalid::new(format!("id is a UUIDv{}, not a UUIDv7", id.get_version_num())));
	}
	Ok(EventId::from_raw(id))
}

macro_rules! opaque_id {
	($(#[$doc:meta])* $name:ident, $what:literal, $check:path) => {
		$(#[$doc])*
		#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, schemars::JsonSchema, serde::Serialize)]
		#[serde(try_from = "String")]
		pub struct $name(String);

		impl TryFrom<String> for $name {
			type Error = Invalid;

			fn try_from(raw: String) -> Result<Self, Invalid> {
				Self::parse(&raw)
			}
		}

		impl $name {
			pub fn parse(raw: &str) -> Result<Self, Invalid> {
				if $check(raw) { Ok(Self(raw.to_owned())) } else { Err(Invalid::new(concat!($what, " is not a valid id"))) }
			}

			pub fn as_str(&self) -> &str {
				&self.0
			}
		}

		impl fmt::Display for $name {
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				f.write_str(&self.0)
			}
		}
	};
}

opaque_id!(
	/// A brand: aquafix, vifnet, … Lowercase slug, 1–64 of `[a-z0-9_-]`, starting alphanumeric.
	BrandId,
	"subject.brand_id",
	is_slug
);
opaque_id!(
	/// A location: a GBP listing plus its landing subdomain.
	LocationId,
	"subject.location_id",
	is_opaque
);
opaque_id!(
	/// A lead, unique within its brand.
	LeadId,
	"subject.lead_id",
	is_opaque
);
opaque_id!(
	/// A job won from a lead.
	JobId,
	"subject.job_id",
	is_opaque
);

/// 1–64 of `[a-z0-9_-]`, starting alphanumeric.
pub fn is_slug(s: &str) -> bool {
	(1..=64).contains(&s.len())
		&& s.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
		&& s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// 1–128 of `[A-Za-z0-9._:-]`, starting alphanumeric: what a source's own ids look like.
/// No `@`, `+` or spaces, so an email or a formatted phone number is not taken for an id; and
/// nothing made only of digits, dots and dashes with seven digits or more, which is what a
/// phone number looks like once its `+` and spaces are gone. An id is stored in the clear and
/// shown in reporting, so it must not be a way to carry the customer's number there.
pub fn is_opaque(s: &str) -> bool {
	let charset =
		(1..=128).contains(&s.len()) && s.starts_with(|c: char| c.is_ascii_alphanumeric()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'));
	let phone_like = s.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-')) && s.bytes().filter(u8::is_ascii_digit).count() >= 7;
	charset && !phone_like
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn event_ids_are_v7() {
		assert!(parse_event_id(&Uuid::now_v7().to_string()).is_ok());
		assert_eq!(parse_event_id("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap_err().0, "id is a UUIDv1, not a UUIDv7");
		assert!(parse_event_id("not-a-uuid").is_err());
	}

	#[test]
	fn slugs_and_opaque_ids() {
		assert!(BrandId::parse("aquafix").is_ok());
		for bad in ["", "Aquafix", "-x", "a b", &"a".repeat(65)] {
			assert!(BrandId::parse(bad).is_err(), "{bad:?}");
		}
		for good in ["01J9Z:lead-42.x_1", "123456", "L-33600000000"] {
			assert!(LeadId::parse(good).is_ok(), "{good:?}");
		}
		for bad in ["", "+33600000000", "33600000000", "06-12-34-56-78", "06.12.34.56.78", "a@b.c", "a b", &"a".repeat(129)] {
			assert!(LeadId::parse(bad).is_err(), "{bad:?}");
		}
	}
}
