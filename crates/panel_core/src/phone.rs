//! A phone number as the funnel reads it: kitstart's `normalizePhone`
//! (EV-Service-Arb/sites_tools `ts/kitstart/src/core/phone.ts`), ported rule for rule, so a number the site stored and
//! the same number typed into a booking form compare equal.
//!
//! One difference: kitstart runs NFKC first; here only what NFKC does to a phone number is
//! done — full-width digits and `＋` read as ASCII, and every Unicode space (no-break ones
//! included) is a space.

/// The national number's length, after the country code, for the plans kitstart reads
/// closely; any other plan keeps E.164's 8 to 15 digits.
const NATIONAL_LENGTH: [(&str, usize, usize); 7] = [("1", 10, 10), ("32", 8, 9), ("34", 9, 9), ("39", 6, 11), ("41", 9, 9), ("44", 9, 10), ("49", 7, 13)];

fn fits_plan(digits: &str) -> bool {
	for code in [&digits[..1], &digits[..2.min(digits.len())]] {
		if let Some((_, min, max)) = NATIONAL_LENGTH.iter().find(|(c, ..)| *c == code) {
			let national = digits.len() - code.len();
			return (*min..=*max).contains(&national);
		}
	}
	true
}

/// What NFKC makes of the characters a phone number may be typed with.
fn fold(c: char) -> char {
	match c {
		'\u{FF10}'..='\u{FF19}' => char::from(b'0' + (c as u32 - 0xff10) as u8),
		'\u{FF0B}' => '+',
		c if c.is_whitespace() => ' ',
		c => c,
	}
}

fn all_digits(s: &str) -> bool {
	!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// E.164 when `raw` is a number someone can be reached on, otherwise `None`: French numbers
/// in every way they are typed (`06 12 34 56 78`, `+33 (0)6 …`, `0033 6 …`, `6 12 34 56 78`)
/// become `+33612345678`; any other plausible international number is compacted; a run of
/// one digit is a refusal to give a number, not one.
pub fn normalize(raw: &str) -> Option<String> {
	let folded: String = raw.chars().map(fold).collect();
	let mut s: String = folded.replace("(0)", "").chars().filter(|c| !matches!(c, ' ' | '.' | '-' | '(' | ')' | '/')).collect();
	if let Some(rest) = s.strip_prefix("00") {
		s = format!("+{rest}");
	}
	let e164 = if let Some(rest) = s.strip_prefix("+33") {
		// `+33` then an optional trunk zero, then nine digits from 1-9.
		let national = rest.strip_prefix('0').filter(|n| n.len() == 9).unwrap_or(rest);
		(national.len() == 9 && all_digits(national) && !national.starts_with('0')).then(|| format!("+33{national}"))?
	} else if let Some(trunk) = s.strip_prefix('0').filter(|t| t.len() == 9 && all_digits(t) && !t.starts_with('0')) {
		format!("+33{trunk}")
	} else if s.len() == 9 && all_digits(&s) && !s.starts_with('0') {
		format!("+33{s}")
	} else {
		let digits = s.strip_prefix('+')?;
		let plausible = (8..=15).contains(&digits.len()) && all_digits(digits) && !digits.starts_with('0') && fits_plan(digits);
		plausible.then(|| s.clone())?
	};
	let subscriber = e164.strip_prefix("+33").unwrap_or(&e164[1..]);
	let first = subscriber.as_bytes()[0];
	if subscriber.len() > 1 && subscriber.bytes().all(|b| b == first) {
		return None;
	}
	Some(e164)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn as_kitstart_reads_them() {
		for (raw, want) in [
			("06 12 34 56 78", Some("+33612345678")),
			("+33 (0)6 12 34 56 78", Some("+33612345678")),
			("0033 6 12 34 56 78", Some("+33612345678")),
			("6 12 34 56 78", Some("+33612345678")),
			("+33612345678", Some("+33612345678")),
			("06.12.34.56.78", Some("+33612345678")),
			("０６\u{a0}１２ ３４ ５６ ７８", Some("+33612345678")),
			("+44 7911 123456", Some("+447911123456")),
			("+1 415 555 0100", Some("+14155550100")),
			("+12345678", None),
			("00 00 00 00 00", None),
			("06 66 66 66 66", None),
			("+33 012345678", None),
			("12345", None),
			("phone", None),
			("", None),
			("+33 6 12 34 56 7", None),
		] {
			assert_eq!(normalize(raw).as_deref(), want, "{raw:?}");
		}
	}
}
