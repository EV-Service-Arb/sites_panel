//! What the conductor's Telegram is told of the sites (docs/ARCHITECTURE.md, Notifications):
//! the rules' names, what a message says and the buttons under it. Who gets it, when and how is
//! the conductor's.

use std::fmt;

use jiff::SignedDuration;

use crate::fact::LeadSuspect;

/// What a note is about; the conductor holds who may get each and whether it is on.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Rule {
	/// A lead came in: brand, location, what is needed and the phone, with buttons.
	NewLead,
	/// A lead has waited past [`crate::funnel::CONTACT_SLA`] for its first contact: once.
	ContactOverdue,
	/// A payment was recorded.
	PaymentReceived,
	/// A slot was booked, moved or canceled — by the customer through a provider, or by an
	/// operator.
	Booked,
}

impl Rule {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::NewLead => "new_lead",
			Self::ContactOverdue => "contact_overdue",
			Self::PaymentReceived => "payment_received",
			Self::Booked => "booked",
		}
	}
}

impl fmt::Display for Rule {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(self.as_str())
	}
}

/// The language of the messages (one per deployment, `TELEGRAM_LOCALE`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Locale {
	Ru,
	En,
}

impl std::str::FromStr for Locale {
	type Err = crate::Invalid;

	fn from_str(s: &str) -> Result<Self, crate::Invalid> {
		match s.trim() {
			"ru" => Ok(Self::Ru),
			"en" => Ok(Self::En),
			other => Err(crate::Invalid::new(format!("TELEGRAM_LOCALE {other:?} is not one of ru, en"))),
		}
	}
}

/// A button under a lead's message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Button {
	/// "Взял": the lead is contacted (`lead.contacted`).
	Take,
	/// "Не дозвонился": a call attempted and not answered (`call.attempted` + `call.logged`).
	NoAnswer,
}

impl Button {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Take => "take",
			Self::NoAnswer => "no_answer",
		}
	}

	/// The button [`Self::as_str`] names.
	pub fn from_name(name: &str) -> Option<Self> {
		[Self::Take, Self::NoAnswer].into_iter().find(|b| b.as_str() == name)
	}

	pub fn label(self, locale: Locale) -> &'static str {
		match (self, locale) {
			(Self::Take, Locale::Ru) => "Взял",
			(Self::Take, Locale::En) => "Taken",
			(Self::NoAnswer, Locale::Ru) => "Не дозвонился",
			(Self::NoAnswer, Locale::En) => "No answer",
		}
	}

	/// What the message says once pressed, before who pressed it: `"{pressed}: {name}"`.
	pub fn pressed(self, locale: Locale) -> &'static str {
		match (self, locale) {
			(Self::Take, Locale::Ru) => "Взял",
			(Self::Take, Locale::En) => "Taken by",
			(Self::NoAnswer, Locale::Ru) => "Не дозвонился",
			(Self::NoAnswer, Locale::En) => "No answer",
		}
	}

	/// The buttons a message keeps after this one was pressed: none once taken; "Take" still,
	/// after a call nobody answered.
	pub fn leaves(self) -> &'static [Button] {
		match self {
			Self::Take => &[],
			Self::NoAnswer => &[Self::Take],
		}
	}
}

/// A lead as a message shows it. The PII fields are set only for a role that sees PII.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LeadNote {
	pub brand: String,
	pub location: Option<String>,
	pub name: Option<String>,
	pub need: Option<String>,
	pub phone: Option<String>,
}

/// What a message is about.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Note {
	NewLead(LeadNote),
	/// A new lead the landing's antispam doubted: told under [`Rule::NewLead`] too, since it
	/// may be a person, but headed so nobody takes it for an ordinary one.
	SuspectLead {
		lead: LeadNote,
		suspect: LeadSuspect,
	},
	ContactOverdue {
		lead: LeadNote,
		waiting: SignedDuration,
	},
	PaymentReceived {
		brand: String,
		lead: String,
		billed: i64,
		commission: i64,
		currency: String,
	},
	/// A slot booked, moved or canceled. `lead` is `None` for a provider's booking no lead was
	/// found for ("without a lead"); `slot` is already in the place's time.
	Booking {
		change: BookingChange,
		brand: String,
		lead: Option<LeadNote>,
		slot: Option<String>,
		provider: String,
	},
}

/// What became of a slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BookingChange {
	Booked,
	Moved,
	Canceled,
}

impl Note {
	pub fn rule(&self) -> Rule {
		match self {
			Self::NewLead(_) | Self::SuspectLead { .. } => Rule::NewLead,
			Self::ContactOverdue { .. } => Rule::ContactOverdue,
			Self::PaymentReceived { .. } => Rule::PaymentReceived,
			Self::Booking { .. } => Rule::Booked,
		}
	}

	/// The buttons under it: the lead's two actions for the lead rules, none otherwise.
	pub fn buttons(&self) -> &'static [Button] {
		match self {
			Self::NewLead(_) | Self::SuspectLead { .. } | Self::ContactOverdue { .. } => &[Button::Take, Button::NoAnswer],
			Self::PaymentReceived { .. } | Self::Booking { .. } => &[],
		}
	}

	/// Plain text (sent without a parse mode, so nothing a customer typed is markup).
	pub fn text(&self, locale: Locale) -> String {
		self.render(locale).text
	}

	/// The text and its `code` entities: what a customer typed goes on one line of its own
	/// field, cleaned ([`one_line`]) and bounded, inside a `code` entity, so it can neither
	/// fake a line of ours ("Взял: …") nor turn into a link.
	pub fn render(&self, locale: Locale) -> Rendered {
		let ru = locale == Locale::Ru;
		let mut out = Rendered::default();
		match self {
			Self::NewLead(lead) => lead_text(&mut out, if ru { "Новая заявка" } else { "New lead" }, lead, locale),
			Self::SuspectLead { lead, suspect } => {
				let head = match (suspect, ru) {
					(LeadSuspect::RateLimited, true) => "Подозрительная заявка (антиспам: слишком много отправок)",
					(LeadSuspect::RateLimited, false) => "Suspect lead (antispam: too many submissions)",
					(LeadSuspect::TooFast, true) => "Подозрительная заявка (антиспам: форма заполнена слишком быстро)",
					(LeadSuspect::TooFast, false) => "Suspect lead (antispam: form filled too fast)",
				};
				lead_text(&mut out, head, lead, locale);
			}
			Self::ContactOverdue { lead, waiting } => {
				let mins = waiting.as_mins().max(0);
				let head = if ru {
					format!("Заявка ждёт звонка {mins} мин")
				} else {
					format!("Lead waiting for a call for {mins} min")
				};
				lead_text(&mut out, &head, lead, locale);
			}
			Self::PaymentReceived {
				brand,
				lead,
				billed,
				commission,
				currency,
			} => {
				let (billed, commission) = (money(*billed), money(*commission));
				out.push(if ru { "Оплата получена\n" } else { "Payment received\n" });
				out.push(&format!("{brand} · "));
				out.code(lead);
				if ru {
					out.push(&format!("\nСумма: {billed} {currency}\nКомиссия: {commission} {currency}"));
				} else {
					out.push(&format!("\nBilled: {billed} {currency}\nCommission: {commission} {currency}"));
				}
			}
			Self::Booking {
				change,
				brand,
				lead,
				slot,
				provider,
			} => {
				let head = match (change, ru) {
					(BookingChange::Booked, true) => "Бронь",
					(BookingChange::Booked, false) => "Slot booked",
					(BookingChange::Moved, true) => "Бронь перенесена",
					(BookingChange::Moved, false) => "Booking moved",
					(BookingChange::Canceled, true) => "Бронь отменена",
					(BookingChange::Canceled, false) => "Booking canceled",
				};
				let head = match (slot, lead.is_none(), ru) {
					(Some(slot), false, _) => format!("{head}: {slot}"),
					(Some(slot), true, true) => format!("{head} без заявки: {slot}"),
					(Some(slot), true, false) => format!("{head}, no lead: {slot}"),
					(None, false, _) => head.to_owned(),
					(None, true, true) => format!("{head} без заявки"),
					(None, true, false) => format!("{head}, no lead"),
				};
				match lead {
					Some(lead) => lead_text(&mut out, &head, lead, locale),
					None => out.push(&format!("{head}\n{brand}")),
				}
				out.push(&format!("\n{} {provider}", if ru { "Через:" } else { "Via:" }));
			}
		}
		out.clamp(MAX_MESSAGE);
		out
	}
}

/// A message's length, in characters, whatever went into it: well under Telegram's 4096.
pub const MAX_MESSAGE: usize = 3500;

/// Longest a customer's field may run in a message, in characters.
pub const MAX_NAME: usize = 80;
pub const MAX_PHONE: usize = 40;
pub const MAX_NEED: usize = 500;

/// A span of a message shown as `code`, in UTF-16 code units (what the Bot API counts).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Entity {
	pub offset: usize,
	pub length: usize,
}

/// A message: its text and the spans of it that are `code`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Rendered {
	pub text: String,
	pub code: Vec<Entity>,
}

impl Rendered {
	fn units(&self) -> usize {
		self.text.encode_utf16().count()
	}

	fn push(&mut self, s: &str) {
		self.text.push_str(s);
	}

	/// `s` as `code`; nothing when it is empty.
	fn code(&mut self, s: &str) {
		let length = s.encode_utf16().count();
		if length > 0 {
			self.code.push(Entity { offset: self.units(), length });
		}
		self.text.push_str(s);
	}

	/// At most `max` characters, the entities cut to what is left.
	fn clamp(&mut self, max: usize) {
		let Some((cut, _)) = self.text.char_indices().nth(max) else { return };
		self.text.truncate(cut);
		let units = self.units();
		self.code.retain_mut(|e| {
			e.length = e.length.min(units.saturating_sub(e.offset));
			e.offset < units && e.length > 0
		});
	}
}

/// Whether a character is one a customer's text may not carry into a message as it is: any
/// control character (a line break could start a line that reads as ours), the line and
/// paragraph separators, and the marks that reorder text (bidi overrides and isolates).
fn hidden(c: char) -> bool {
	c.is_control() || matches!(c, '\u{2028}' | '\u{2029}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// A customer's text on one line: the characters `hidden` names made spaces, runs of spaces one,
/// at most `max` characters (the last one "…" when cut).
pub fn one_line(s: &str, max: usize) -> String {
	let spaced: String = s.chars().map(|c| if hidden(c) { ' ' } else { c }).collect();
	let joined = spaced.split_whitespace().collect::<Vec<_>>().join(" ");
	match joined.char_indices().nth(max.saturating_sub(1)) {
		Some((i, _)) if joined.chars().count() > max => format!("{}…", &joined[..i]),
		_ => joined,
	}
}

/// A phone number as a message shows it: digits, `+`, `(`, `)`, `-` and spaces only.
pub fn phone(s: &str) -> String {
	let kept: String = s
		.chars()
		.map(|c| if hidden(c) { ' ' } else { c })
		.filter(|c| c.is_ascii_digit() || matches!(c, '+' | '(' | ')' | '-' | ' '))
		.collect();
	one_line(&kept, MAX_PHONE)
}

fn lead_text(out: &mut Rendered, head: &str, lead: &LeadNote, locale: Locale) {
	let ru = locale == Locale::Ru;
	out.push(head);
	out.push("\n");
	out.push(&lead.brand);
	if let Some(location) = &lead.location {
		out.push(&format!(" · {location}"));
	}
	let clean = |v: &Option<String>, max| v.as_deref().map(|v| one_line(v, max)).filter(|v| !v.is_empty());
	if let Some(name) = clean(&lead.name, MAX_NAME) {
		out.push(if ru { "\nИмя: " } else { "\nName: " });
		out.code(&name);
	}
	if let Some(need) = clean(&lead.need, MAX_NEED) {
		out.push(if ru { "\nНужно: " } else { "\nNeeds: " });
		out.code(&need);
	}
	if let Some(p) = lead.phone.as_deref().map(phone).filter(|p| !p.is_empty()) {
		out.push(if ru { "\nТелефон: " } else { "\nPhone: " });
		out.push(&p);
	}
}

/// Minor units as the amount a person reads: whole when it is whole, else with its cents.
/// Every currency the panel takes has two decimals.
fn money(minor: i64) -> String {
	let sign = if minor < 0 { "-" } else { "" };
	let abs = minor.unsigned_abs();
	if abs.is_multiple_of(100) {
		format!("{sign}{}", abs / 100)
	} else {
		format!("{sign}{}.{:02}", abs / 100, abs % 100)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn texts() {
		let lead = LeadNote {
			brand: "aquafix".into(),
			location: Some("paris-11".into()),
			name: None,
			need: Some("leaking tap".into()),
			phone: Some("+33 6 00 00 00 00".into()),
		};
		assert_eq!(
			Note::NewLead(lead.clone()).text(Locale::Ru),
			"Новая заявка\naquafix · paris-11\nНужно: leaking tap\nТелефон: +33 6 00 00 00 00"
		);
		let withheld = LeadNote {
			need: None,
			phone: None,
			..lead.clone()
		};
		assert_eq!(Note::NewLead(withheld.clone()).text(Locale::En), "New lead\naquafix · paris-11");
		let doubted = Note::SuspectLead {
			lead: withheld,
			suspect: LeadSuspect::TooFast,
		};
		assert_eq!(doubted.text(Locale::En), "Suspect lead (antispam: form filled too fast)\naquafix · paris-11");
		assert_eq!((doubted.rule(), doubted.buttons()), (Rule::NewLead, [Button::Take, Button::NoAnswer].as_slice()));
		let overdue = Note::ContactOverdue {
			lead,
			waiting: SignedDuration::from_mins(47),
		};
		assert!(overdue.text(Locale::Ru).starts_with("Заявка ждёт звонка 47 мин\n"));
		assert_eq!(overdue.buttons(), [Button::Take, Button::NoAnswer]);
		let paid = Note::PaymentReceived {
			brand: "aquafix".into(),
			lead: "L-1".into(),
			billed: 12050,
			commission: 3000,
			currency: "EUR".into(),
		};
		assert_eq!(paid.text(Locale::En), "Payment received\naquafix · L-1\nBilled: 120.50 EUR\nCommission: 30 EUR");
		assert!(paid.buttons().is_empty());
		assert_eq!(money(-5), "-0.05");
		let booked = Note::Booking {
			change: BookingChange::Booked,
			brand: "aquafix".into(),
			lead: Some(LeadNote {
				brand: "aquafix".into(),
				location: Some("royat".into()),
				phone: Some("+33612345678".into()),
				..LeadNote::default()
			}),
			slot: Some("2026-10-06 14:30 (Paris)".into()),
			provider: "google_calendar".into(),
		};
		assert_eq!(
			booked.text(Locale::Ru),
			"Бронь: 2026-10-06 14:30 (Paris)\naquafix · royat\nТелефон: +33612345678\nЧерез: google_calendar"
		);
		assert_eq!((booked.rule(), booked.buttons()), (Rule::Booked, [].as_slice()));
		let orphan = Note::Booking {
			change: BookingChange::Canceled,
			brand: "vifnet".into(),
			lead: None,
			slot: None,
			provider: "google_calendar".into(),
		};
		assert_eq!(orphan.text(Locale::En), "Booking canceled, no lead\nvifnet\nVia: google_calendar");
	}

	#[test]
	fn a_customer_cannot_forge_our_lines_or_links() {
		let lead = LeadNote {
			brand: "aquafix".into(),
			location: None,
			name: Some("Eve\u{202e}\t".into()),
			need: Some(format!("tap\nВзял: Mallory\r\n\u{2028}see https://evil.example {}", "x".repeat(10 * 1024))),
			phone: Some("+33 6 <a>00\n00 https://x".into()),
		};
		let r = Note::NewLead(lead).render(Locale::Ru);
		let lines: Vec<&str> = r.text.lines().collect();
		assert_eq!(lines.len(), 5, "{lines:?}");
		assert!(!lines.iter().any(|l| l.starts_with("Взял")), "no line of ours from the customer");
		assert_eq!(lines[2], "Имя: Eve");
		assert_eq!(lines[4], "Телефон: +33 6 00 00", "digits and phone punctuation only");
		let need = lines[3].strip_prefix("Нужно: ").unwrap();
		assert_eq!(need.chars().count(), MAX_NEED);
		assert!(need.starts_with("tap Взял: Mallory see https://evil.example x") && need.ends_with('…'));
		let units: Vec<u16> = r.text.encode_utf16().collect();
		let spans: Vec<String> = r.code.iter().map(|e| String::from_utf16(&units[e.offset..e.offset + e.length]).unwrap()).collect();
		assert_eq!(spans, ["Eve", need], "the typed fields, exactly, are code");
		assert!(r.text.chars().count() <= MAX_MESSAGE);
		assert_eq!(one_line("  a \u{2066}b\u{2069}  c ", 80), "a b c");
		assert_eq!(one_line("abcdef", 4), "abc…");
		let mut long = Rendered::default();
		long.push("ab");
		long.code("cdef");
		long.clamp(4);
		assert_eq!((long.text.as_str(), long.code.as_slice()), ("abcd", [Entity { offset: 2, length: 2 }].as_slice()));
	}
}
