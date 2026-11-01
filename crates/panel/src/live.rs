//! What changed, told to whoever is watching: the bus the conductor's change feed
//! (`/api/conductor/changes`) reads.
//!
//! The panel runs as one process (one pod, one SQLite writer), so the bus is in-process: a
//! `tokio::sync::broadcast` channel. Every write that a screen reads publishes here, from the
//! engine, after its transaction commits — never before, so a client told "changed" and
//! reading at once finds the change. What publishes:
//!
//! ```text
//! Panel::journal        every event journaled and projected: ingest, the operator's actions,
//!                       a landing's experiments,
//!                       an admin's experiment settings, the booking adapters
//!                                                   → leads | lead | experiments | bookings
//! Panel::rebuild_…      the projections replaced whole                 → resync
//! place::edit, register a place's settings, withdrawn, registered      → places
//! pricing               a brand's model saved or removed, its locales  → pricing
//! invoices              a row stored, read from banking, dropped, paid or ended → invoices
//! ```
//!
//! Publishing never waits and never fails a write: with nobody subscribed the message is
//! simply dropped, and a subscriber that falls behind by more than the channel holds is told
//! so ([`broadcast::error::RecvError::Lagged`]) instead of the channel growing.
//!
//! A command run in another process (`sites_panel place set`, a rebuild from the CLI)
//! publishes on its own bus, which nobody listens to: the feed does not see it.

use jiff::Timestamp;
use panel_core::{fact::Fact, ids::BrandId, lead::Recorded};
use sa_auth::{Analysis, Work};
use tokio::sync::broadcast;

/// How many messages a subscriber may fall behind before it is told to read everything
/// afresh. A batch of ingest is at most 500 events; most moments carry a handful.
pub const DEFAULT_CAPACITY: usize = 1024;

/// What a change is about: which of the screens' reads it may have changed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Topic {
	/// A new lead (its `lead.created`).
	Leads,
	/// Something about one lead: a stage, a call, a payment.
	Lead,
	/// A place's settings, withdrawn or restored, or registered.
	Places,
	/// A brand's pricing: its model saved or removed, or its locales.
	Pricing,
	/// A brand's experiments: what its landing declared, or what an admin set over it.
	Experiments,
	/// The providers' bookings without a lead: one came, changed, or was joined to a lead.
	Bookings,
	/// The invoices asked of banking: one issued, read, dropped, paid or ended.
	Invoices,
}

impl Topic {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Leads => "leads",
			Self::Lead => "lead",
			Self::Places => "places",
			Self::Pricing => "pricing",
			Self::Experiments => "experiments",
			Self::Bookings => "bookings",
			Self::Invoices => "invoices",
		}
	}
}

/// Something committed that a screen reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
	pub topic: Topic,
	pub brand: Option<BrandId>,
	/// The lead, or the place's slug; `None` when the topic has no single subject.
	pub id: Option<String>,
	/// When it was committed (the writer's clock).
	pub at: Timestamp,
}

impl Change {
	/// The permission of the section that reads what changed: whoever holds it may be told,
	/// for every brand (no permission is narrower than the whole of a section).
	pub fn permission(&self) -> &'static str {
		match self.topic {
			Topic::Leads | Topic::Lead | Topic::Places | Topic::Pricing | Topic::Bookings | Topic::Invoices => Work::Read.as_str(),
			Topic::Experiments => Analysis::Read.as_str(),
		}
	}

	/// What journaling `event` changed; `None` for what no screen reads.
	pub fn of_event(event: &Recorded, at: Timestamp) -> Option<Self> {
		let lead = || event.subject.lead_id.as_ref().map(|l| l.as_str().to_owned());
		let (topic, id) = match &event.fact {
			Fact::LeadCreated { .. } => (Topic::Leads, lead()),
			Fact::LeadMessaged { .. }
			| Fact::LeadContacted { .. }
			| Fact::LeadQuoted { .. }
			| Fact::JobWon
			| Fact::LeadLost { .. }
			| Fact::JobCompleted
			| Fact::PaymentReceived { .. }
			| Fact::CallAttempted
			| Fact::CallLogged { .. }
			| Fact::BookingRequested { .. }
			| Fact::BookingCreated { .. }
			| Fact::BookingCanceled { .. }
			| Fact::BookingSet { .. }
			| Fact::BookingStatusChanged(_)
			| Fact::BookingCleared
			| Fact::BookingAttached { .. } => (Topic::Lead, lead()),
			Fact::ExperimentsDeclared(_) => (Topic::Experiments, None),
			Fact::ExperimentConfigured { patch, .. } => (Topic::Experiments, Some(patch.key.clone())),
			Fact::RetiredCount => return None,
		};
		Some(Self {
			topic,
			brand: Some(event.subject.brand_id.clone()),
			id,
			at,
		})
	}
}

impl Change {
	/// What journaling `event` changed, once projected: as [`Self::of_event`], but a booking
	/// event tells every lead it changed (a provider's booking may leave one lead and join
	/// another), and the list of bookings without a lead when that changed — and nothing of a
	/// lead it did not change.
	pub fn of_applied(event: &Recorded, applied: &crate::store::projections::Applied, at: Timestamp) -> Vec<Self> {
		let lead = |l: &panel_core::ids::LeadId| Self {
			topic: Topic::Lead,
			brand: Some(event.subject.brand_id.clone()),
			id: Some(l.as_str().to_owned()),
			at,
		};
		match &event.fact {
			Fact::BookingCreated { .. } | Fact::BookingCanceled { .. } | Fact::BookingAttached { .. } => {
				let mut out: Vec<Self> = applied.leads.iter().map(lead).collect();
				if applied.unmatched {
					out.push(Self {
						topic: Topic::Bookings,
						brand: Some(event.subject.brand_id.clone()),
						id: None,
						at,
					});
				}
				out
			}
			_ => Self::of_event(event, at).into_iter().collect(),
		}
	}
}

/// What travels on the bus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Signal {
	Changed(Change),
	/// Too much changed to say what: read everything again.
	Resync,
	/// The process is stopping: the feed ends, so the graceful shutdown is not held open.
	GoingAway,
}

/// The in-process bus. Cloning it gives another handle on the same channel.
#[derive(Clone, Debug)]
pub struct Bus {
	tx: broadcast::Sender<Signal>,
}

impl Default for Bus {
	fn default() -> Self {
		Self::new(DEFAULT_CAPACITY)
	}
}

impl Bus {
	/// A bus whose subscribers may fall `capacity` messages behind (at least 1).
	pub fn new(capacity: usize) -> Self {
		Self {
			tx: broadcast::channel(capacity.max(1)).0,
		}
	}

	pub fn subscribe(&self) -> broadcast::Receiver<Signal> {
		self.tx.subscribe()
	}

	/// Tells every subscriber; never waits.
	pub fn publish(&self, signal: Signal) {
		// `Err` means nobody is subscribed, and then there is nobody to tell: the write it
		// follows has committed either way.
		let _nobody_listening = self.tx.send(signal);
	}

	pub fn changed(&self, change: Change) {
		self.publish(Signal::Changed(change));
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn each_topic_is_told_under_the_permission_of_its_read() {
		let change = |topic| Change {
			topic,
			brand: None,
			id: None,
			at: Timestamp::UNIX_EPOCH,
		};
		for topic in [Topic::Leads, Topic::Lead, Topic::Places, Topic::Pricing, Topic::Bookings, Topic::Invoices] {
			assert_eq!(change(topic).permission(), "sa:sites:work:read", "{topic:?}");
		}
		assert_eq!(change(Topic::Experiments).permission(), "sa:sites:analysis:read");
	}

	#[tokio::test]
	async fn publishing_with_nobody_listening_is_fine() {
		let bus = Bus::new(2);
		bus.publish(Signal::Resync);
		let mut rx = bus.subscribe();
		bus.publish(Signal::Resync);
		assert_eq!(rx.recv().await.unwrap(), Signal::Resync);
	}

	#[tokio::test]
	async fn a_subscriber_behind_is_told_it_lagged() {
		let bus = Bus::new(2);
		let mut rx = bus.subscribe();
		for _ in 0..5 {
			bus.publish(Signal::Resync);
		}
		assert!(matches!(rx.recv().await, Err(broadcast::error::RecvError::Lagged(3))));
	}
}
