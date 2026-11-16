//! What committed, told to whoever is watching: the bus the conductor's change feed
//! (`/api/conductor/changes`) reads.
//!
//! A topic is a table, `sites.<table>`: the store's pool tells each table a committed write
//! touched once its connection is released (`ev_lib::sqlite`), whoever wrote it — ingest, an
//! operator, an admin, the booking pull, the invoices' confirmation, a migration. A rolled-back
//! write tells nothing. A read's answer names the tables it read (`x-sa-reads`), so a screen
//! re-reads exactly what a change may have touched.
//!
//! The panel runs as one process (one pod, one SQLite writer), so the bus is in-process: a
//! `tokio::sync::broadcast` channel. Publishing never waits and never fails a write; a
//! subscriber that falls behind by more than the channel holds is told so
//! ([`broadcast::error::RecvError::Lagged`]) instead of the channel growing. A command run in
//! another process (`sites_panel rebuild-projections`) tells its own bus, which nobody hears.

use std::sync::Arc;

use tokio::sync::{broadcast, watch};

/// How many messages a subscriber may fall behind before it is told to read everything
/// afresh. A batch of ingest is at most 500 events; most moments carry a handful.
pub const DEFAULT_CAPACITY: usize = 1024;

/// The in-process bus: each table a committed write touched, and whether the process is
/// stopping. Cloning it gives another handle on the same channels.
#[derive(Clone, Debug)]
pub struct Bus {
	tx: broadcast::Sender<String>,
	going: Arc<watch::Sender<bool>>,
}

impl Default for Bus {
	fn default() -> Self {
		Self::new(DEFAULT_CAPACITY)
	}
}

impl Bus {
	/// A bus whose subscribers may fall `capacity` topics behind (at least 1).
	pub fn new(capacity: usize) -> Self {
		Self {
			tx: broadcast::channel(capacity.max(1)).0,
			going: Arc::new(watch::channel(false).0),
		}
	}

	pub fn subscribe(&self) -> broadcast::Receiver<String> {
		self.tx.subscribe()
	}

	/// Tells every subscriber; never waits.
	pub fn publish(&self, topic: &str) {
		// `Err` means nobody is subscribed, and then there is nobody to tell: the write it
		// follows has committed either way.
		let _nobody_listening = self.tx.send(topic.to_owned());
	}

	/// The process is stopping: every feed ends, so the graceful shutdown is not held open by
	/// a response that never would.
	pub fn going_away(&self) {
		self.going.send_replace(true);
	}

	/// Resolves once [`Self::going_away`] was called.
	pub fn gone(&self) -> impl Future<Output = ()> + Send + 'static {
		let mut going = self.going.subscribe();
		async move {
			// `Err`: every handle on the bus is dropped, and nothing can be told any more either.
			let _bus_dropped = going.wait_for(|gone| *gone).await;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn publishing_with_nobody_listening_is_fine() {
		let bus = Bus::new(2);
		bus.publish("sites.leads");
		let mut rx = bus.subscribe();
		bus.publish("sites.places");
		assert_eq!(rx.recv().await.unwrap(), "sites.places");
	}

	#[tokio::test]
	async fn a_subscriber_behind_is_told_it_lagged() {
		let bus = Bus::new(2);
		let mut rx = bus.subscribe();
		for _ in 0..5 {
			bus.publish("sites.leads");
		}
		assert!(matches!(rx.recv().await, Err(broadcast::error::RecvError::Lagged(3))));
	}
}
