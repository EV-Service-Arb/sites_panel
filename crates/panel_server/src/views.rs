//! Where a person can be on the sites' pages, relative to the host's base: each view's href, its
//! parse and the document's `x-views` (`ev_lib::views`), from which the front end routes and the
//! conductor nests them under its own.

use ev_lib::views::Views;
use panel_core::ids::{LeadId, LocationId};
use sa_auth::{Analysis, Work};
use sa_core::ids::BrandId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::LeadsQuery;

#[derive(Clone, Debug, PartialEq, Views)]
pub(crate) enum View {
	/// Where the base leads: the leads for who works them, else the funnel.
	#[view("/")]
	Start,
	/// The leads, filtered, and one of them open.
	#[view("/leads", needs = Work::Read, key = 'l')]
	Leads {
		#[view(flatten)]
		filter: LeadsQuery,
		lead: Option<(BrandId, LeadId)>,
	},
	/// The places, their settings and history; one of them open.
	#[view("/places", needs = Work::Read, key = 'p')]
	Places { brand: Option<BrandId>, location: Option<LocationId> },
	/// The brands' price lists; one brand's open.
	#[view("/pricing", needs = Work::Read, key = 'r')]
	Pricing { brand: Option<BrandId> },
	/// The invoices asked of banking, and a new one.
	#[view("/invoices", needs = Work::Read, key = 'i')]
	Invoices,
	/// The funnel over a period, of every brand or one.
	#[view("/overview", needs = Analysis::Read, key = 'o')]
	Overview { period: Option<Period>, brand: Option<BrandId> },
	/// The landings' experiments and their overrides.
	#[view("/experiments", needs = Analysis::Read, key = 'e')]
	Experiments,
}

/// The days the overview's funnel spans, ending today; absent, the funnel's own window.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub(crate) enum Period {
	#[serde(rename = "7")]
	Week,
	#[serde(rename = "30")]
	Month,
	#[serde(rename = "90")]
	Quarter,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn every_view_parses_back_from_its_href() {
		let lead = (BrandId::parse("aquafix").unwrap(), LeadId::parse("L-1").unwrap());
		let filter: LeadsQuery = serde_json::from_value(serde_json::json!({"stage": "created", "overdue": true, "created_from": "2026-10-01", "channel": "whatsapp"})).unwrap();
		for view in [
			View::Start,
			View::Leads { filter, lead: Some(lead) },
			View::Places {
				brand: Some(BrandId::parse("vifnet").unwrap()),
				location: Some(LocationId::parse("royat").unwrap()),
			},
			View::Pricing { brand: None },
			View::Invoices,
			View::Overview {
				period: Some(Period::Quarter),
				brand: None,
			},
			View::Experiments,
		] {
			assert_eq!(View::parse(&view.href()).unwrap(), view, "{}", view.href());
		}
	}
}
