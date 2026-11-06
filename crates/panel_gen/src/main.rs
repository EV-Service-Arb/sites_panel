//! `nix run .#gen`: the TS the front end and the sites would otherwise re-type from the Rust —
//! the panel's own mirror in `frontend/src`, and the generated half of `@evinvest/sites-contract`
//! in `ts/sites-contract/src/generated`. The output is committed; the `generated` pre-commit
//! hook re-runs this and re-stages it.

mod wire;

use std::path::Path;

use ev_lib::ts_gen::Ts;
use panel::experiment::LiveExperiments;
use panel_core::{
	booking::{self, BookingMatch, BookingStatus, DayPart, PageProvider, Provider},
	event::may_write,
	experiment,
	fact::{self, LeadChannel, LeadFlow, LeadSuspect, Messenger},
	funnel::CONTACT_SLA,
	lead::Stage,
	place::{self, DayOfWeek, FIELDS, PlaceLive},
	pricing::{self, InputKind, PricingModel},
};
use panel_server::api::{CURRENCIES, IDEMPOTENCY_KEY, Me};
use sa_auth::{Catalog, Service, SourceKind};

/// `export const NAME = [...] as const;`: the words of a vocabulary whose type ts-rs declares.
fn words(name: &'static str, items: impl IntoIterator<Item = &'static str>) -> Ts {
	Ts::Value {
		name,
		value: items.into_iter().collect::<Vec<_>>().into(),
	}
}

fn value(name: &'static str, v: impl serde::Serialize) -> Ts {
	Ts::Value {
		name,
		value: serde_json::to_value(v).expect("a constant is plain data"),
	}
}

fn main() {
	let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
	contract(&root.join("ts/sites-contract/src/generated"));
	frontend(&root.join("frontend/src"));
}

fn contract(dir: &Path) {
	Ts::write(
		&dir.join("contract.ts"),
		&[
			Ts::types::<(PricingModel, PlaceLive, LiveExperiments)>(),
			Ts::Value {
				name: "PRICING_FORMAT",
				value: pricing::FORMAT.into(),
			},
			Ts::Const {
				name: "PRICING_CURRENCY",
				value: pricing::CURRENCY,
			},
			value("PRICING_LIMITS", pricing::LIMITS),
			Ts::Union {
				name: "INPUT_KINDS",
				ty: "PricingInputKind",
				items: InputKind::ALL.map(InputKind::as_str).into(),
			},
			Ts::Union {
				name: "PRICING_CODES",
				ty: "PricingCode",
				items: pricing::Code::ALL.map(pricing::Code::as_str).into(),
			},
			words("BOOKING_PROVIDERS", Provider::ALL.map(Provider::as_str)),
			words("PAGE_PROVIDERS", PageProvider::ALL.map(PageProvider::as_str)),
			value("BOOKING_LIMITS", booking::LIMITS),
			Ts::Union {
				name: "BOOKING_CODES",
				ty: "BookingCode",
				items: booking::Code::ALL.map(booking::Code::as_str).into(),
			},
			Ts::Union {
				name: "BOOKING_STATUSES",
				ty: "BookingStatus",
				items: BookingStatus::ALL.map(BookingStatus::as_str).into(),
			},
			Ts::Union {
				name: "DAY_PARTS",
				ty: "DayPart",
				items: DayPart::ALL.map(DayPart::as_str).into(),
			},
			words("DAYS", DayOfWeek::ALL.map(DayOfWeek::as_str)),
			words("MESSENGERS", Messenger::ALL.map(Messenger::as_str)),
			Ts::Union {
				name: "PLACE_FIELDS",
				ty: "PlaceField",
				items: FIELDS.into(),
			},
			value("PLACE_LIMITS", place::LIMITS),
			Ts::Union {
				name: "PLACE_CODES",
				ty: "PlaceCode",
				items: place::Code::ALL.map(place::Code::as_str).into(),
			},
			value("EXPERIMENT_LIMITS", experiment::LIMITS),
			value("LEAD_LIMITS", fact::LIMITS),
			Ts::Union {
				name: "LEAD_CHANNELS",
				ty: "LeadChannel",
				items: LeadChannel::ALL.map(LeadChannel::as_str).into(),
			},
			Ts::Union {
				name: "SITE_CHANNELS",
				ty: "SiteChannel",
				items: LeadChannel::SITE.map(LeadChannel::as_str).into(),
			},
			Ts::Union {
				name: "LEAD_SUSPECTS",
				ty: "LeadSuspect",
				items: LeadSuspect::ALL.map(LeadSuspect::as_str).into(),
			},
			Ts::Union {
				name: "LEAD_FLOWS",
				ty: "LeadFlow",
				items: LeadFlow::ALL.map(LeadFlow::as_str).into(),
			},
		],
	);
	Ts::write(
		&dir.join("wire.ts"),
		&[
			Ts::Const {
				name: "SCHEMA",
				value: panel_contracts::SCHEMA,
			},
			wire::types(),
			Ts::Union {
				name: "SITE_EVENTS",
				ty: "SiteEventKey",
				items: panel::wire::REGISTERED
					.iter()
					.filter(|(ty, ..)| may_write(SourceKind::Site, ty))
					.map(|(ty, version, _)| &*format!("{ty}@{version}").leak()) // Ts::Union takes &'static str; one-shot generator
					.collect(),
			},
		],
	);
}

/// The panel's own: what only its front end reads.
fn frontend(root: &Path) {
	let catalog = Catalog::collect("sa", 0);
	Ts::write(
		&root.join("entities/lead/model/generated.ts"),
		&[
			Ts::Union {
				name: "STAGES",
				ty: "Stage",
				items: Stage::ALL.map(Stage::as_str).into(),
			},
			Ts::Union {
				name: "MANUAL_CHANNELS",
				ty: "ManualChannel",
				items: LeadChannel::MANUAL.map(LeadChannel::as_str).into(),
			},
			Ts::Union {
				name: "BOOKING_MATCHES",
				ty: "BookingMatch",
				items: BookingMatch::ALL.map(BookingMatch::as_str).into(),
			},
			Ts::Value {
				name: "CONTACT_SLA_SECONDS",
				value: CONTACT_SLA.as_secs().into(),
			},
		],
	);
	Ts::write(
		&root.join("shared/config/generated.ts"),
		&[Ts::Union {
			name: "CURRENCIES",
			ty: "Currency",
			items: CURRENCIES.into(),
		}],
	);
	Ts::write(
		&root.join("shared/api/generated.ts"),
		&[
			Ts::Const {
				name: "IDEMPOTENCY_HEADER",
				value: IDEMPOTENCY_KEY,
			},
			Ts::Union {
				name: "PERMISSIONS",
				ty: "Permission",
				items: catalog
					.permissions
					.iter()
					.filter(|p| p.starts_with(Service::Sites.prefix()))
					.map(|p| &*p.clone().leak()) // Ts::Union takes &'static str; one-shot generator
					.collect(),
			},
			Ts::types::<Me>(),
		],
	);
}
