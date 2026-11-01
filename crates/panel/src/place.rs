//! A place's live settings, as the panel's editors and the CLI change them and the sites read
//! them (`panel_core::place`). Every change is one write transaction: the current settings
//! read, the caller's view of them checked, the new ones written and the change journaled.

use eyre::WrapErr;
use jiff::Timestamp;
use panel_core::{
	ids::{BrandId, LocationId},
	place::{Change, ChangeKind, Editor, FieldErrors, PlaceSettings},
};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
	Panel,
	store::places::{self, By, StoredPlace},
};

/// A place as the editor shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceView {
	pub brand: BrandId,
	pub slug: LocationId,
	pub withdrawn: bool,
	/// The conductor's GMB this place is the landing of.
	pub gmb_id: Option<i64>,
	/// Only the fields set; empty when none (the site serves its baked config).
	pub settings: PlaceSettings,
	/// `None` while the settings were never set: what a first edit must expect.
	pub updated_at: Option<Timestamp>,
	pub updated_by: Option<String>,
}

/// What a site is answered for a place.
#[derive(Clone, Debug, PartialEq)]
pub enum Live {
	/// Its settings, empty when it has none or the panel does not know it.
	Settings(PlaceSettings),
	/// An admin took it off the sites.
	Withdrawn,
}

/// What the editor saw last, for optimistic concurrency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Expected {
	/// Whatever is there now: the CLI, which reads and writes in one transaction.
	Any,
	/// The `updated_at` the editor read; `None` for settings never set.
	At(Option<Timestamp>),
}

/// Why a change to a place was not made.
#[derive(Debug, thiserror::Error)]
pub enum PlaceError {
	/// No such change of this place.
	#[error("not found")]
	NotFound,
	/// The settings changed since the editor read them.
	#[error("the settings changed since they were read")]
	Conflict,
	/// Another place is already the landing of that GMB.
	#[error("the GMB is linked to another place")]
	GmbTaken,
	#[error("invalid settings: {0:?}")]
	Invalid(FieldErrors),
	#[error(transparent)]
	Internal(#[from] eyre::Report),
}

/// A timestamp at the database's precision, so what the editor is given back compares equal.
pub(crate) fn micros(t: Timestamp) -> eyre::Result<Timestamp> {
	Timestamp::from_microsecond(t.as_microsecond()).wrap_err("a timestamp")
}

pub(crate) fn new_change_id(now: Timestamp) -> Uuid {
	let nanos = u32::try_from(now.subsec_nanosecond().rem_euclid(1_000_000_000)).unwrap_or(0);
	let secs = u64::try_from(now.as_second()).unwrap_or(0);
	Uuid::new_v7(uuid::Timestamp::from_unix(uuid::NoContext, secs, nanos))
}

pub(crate) fn by(editor: &Editor) -> By<'_> {
	By {
		label: editor.label(),
		user: editor.user_id(),
	}
}

fn view(brand: &BrandId, slug: &LocationId, stored: StoredPlace) -> PlaceView {
	let (settings, updated_at, updated_by) = match stored.settings {
		Some(s) => (s.settings, Some(s.updated_at), Some(s.updated_by)),
		None => (PlaceSettings::default(), None, None),
	};
	PlaceView {
		brand: brand.clone(),
		slug: slug.clone(),
		withdrawn: stored.withdrawn,
		gmb_id: stored.gmb_id,
		settings,
		updated_at,
		updated_by,
	}
}

/// What a change does to a place, once its current state is read.
enum Edit {
	Settings { to: PlaceSettings, kind: ChangeKind, reverts: Option<Uuid> },
	Withdrawn(bool),
}

fn place_changed(brand: &BrandId, slug: &LocationId, at: Timestamp) -> crate::live::Change {
	crate::live::Change {
		topic: crate::live::Topic::Places,
		brand: Some(brand.clone()),
		id: Some(slug.as_str().to_owned()),
		at,
	}
}

impl Panel {
	/// A place's settings as the editor shows them; an unknown place has none.
	pub async fn place(&self, brand: &BrandId, slug: &LocationId) -> eyre::Result<PlaceView> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		Ok(view(brand, slug, places::place(&mut conn, brand, slug).await?))
	}

	/// What a site is answered: never "withdrawn" for a place the panel does not know, so an
	/// empty database leaves every site on its baked config rather than taking pages down.
	pub async fn live_place(&self, brand: &BrandId, slug: &LocationId) -> eyre::Result<Live> {
		let place = self.place(brand, slug).await?;
		Ok(if place.withdrawn { Live::Withdrawn } else { Live::Settings(place.settings) })
	}

	/// A place's changes, newest first.
	pub async fn place_history(&self, brand: &BrandId, slug: &LocationId) -> eyre::Result<Vec<Change>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		places::changes(&mut conn, brand, slug).await
	}

	/// Registers a place by hand; `true` when it was not known before (journaled), `false`
	/// when it was (nothing written).
	pub async fn register_place(&self, editor: &Editor, brand: &BrandId, slug: &LocationId, now: Timestamp) -> eyre::Result<(PlaceView, bool)> {
		let now = micros(now)?;
		let mut tx = self.store.begin_write().await?;
		let added = places::register(&mut tx, brand, slug, now, by(editor)).await?;
		if added {
			let change = Change {
				id: new_change_id(now),
				at: now,
				kind: ChangeKind::Register,
				by: editor.label().to_owned(),
				before: PlaceSettings::default(),
				after: PlaceSettings::default(),
				reverts: None,
			};
			places::insert_change(&mut tx, brand, slug, &change, editor.user_id()).await?;
		}
		let stored = places::place(&mut tx, brand, slug).await?;
		tx.commit().await.wrap_err("committing a place's registration")?;
		if added {
			tracing::info!(by = editor.label(), %brand, %slug, "place registered");
			self.live.changed(place_changed(brand, slug, now));
		}
		Ok((view(brand, slug, stored), added))
	}

	/// Replaces a place's settings whole.
	pub async fn set_place(&self, editor: &Editor, brand: &BrandId, slug: &LocationId, settings: PlaceSettings, expected: Expected, now: Timestamp) -> Result<PlaceView, PlaceError> {
		self.edit(editor, brand, slug, expected, now, |_| {
			Ok(Edit::Settings {
				to: settings,
				kind: ChangeKind::Set,
				reverts: None,
			})
		})
		.await
	}

	/// Sets some fields and clears others, over the settings as they are when the write
	/// begins: the CLI's `panel place set`.
	pub async fn patch_place(&self, editor: &Editor, brand: &BrandId, slug: &LocationId, set: Map<String, Value>, clear: &[String], now: Timestamp) -> Result<PlaceView, PlaceError> {
		self.edit(editor, brand, slug, Expected::Any, now, |current| {
			Ok(Edit::Settings {
				to: current.settings.patched(set, clear).map_err(PlaceError::Invalid)?,
				kind: ChangeKind::Set,
				reverts: None,
			})
		})
		.await
	}

	/// Puts back the settings a change found (its `before`); journaled as a change of its own.
	pub async fn revert_place(&self, editor: &Editor, brand: &BrandId, slug: &LocationId, change: Uuid, expected: Expected, now: Timestamp) -> Result<PlaceView, PlaceError> {
		let target = {
			let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
			places::change(&mut conn, brand, slug, change).await?.ok_or(PlaceError::NotFound)?
		};
		self.edit(editor, brand, slug, expected, now, |_| {
			Ok(Edit::Settings {
				to: target.before,
				kind: ChangeKind::Revert,
				reverts: Some(change),
			})
		})
		.await
	}

	/// Makes a registered place the landing of a GMB, or of none; the change is not journaled
	/// (`place_changes` holds settings), only published.
	pub async fn link_gmb(&self, brand: &BrandId, slug: &LocationId, gmb_id: Option<i64>, now: Timestamp) -> Result<PlaceView, PlaceError> {
		let mut tx = self.store.begin_write().await?;
		if !places::place(&mut tx, brand, slug).await?.registered {
			return Err(PlaceError::NotFound);
		}
		if !places::set_gmb(&mut tx, brand, slug, gmb_id).await? {
			return Err(PlaceError::GmbTaken);
		}
		let stored = places::place(&mut tx, brand, slug).await?;
		tx.commit().await.wrap_err("committing a place's GMB")?;
		tracing::info!(%brand, %slug, ?gmb_id, "place linked to a GMB");
		self.live.changed(place_changed(brand, slug, micros(now)?));
		Ok(view(brand, slug, stored))
	}

	/// Every place with a GMB: `(gmb_id, brand, slug)`.
	pub async fn gmbs(&self) -> eyre::Result<Vec<(i64, String, String)>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		places::gmbs(&mut conn).await
	}

	/// Takes a place off the sites (`true`) or puts it back (`false`); its settings are kept.
	pub async fn withdraw_place(&self, editor: &Editor, brand: &BrandId, slug: &LocationId, withdrawn: bool, now: Timestamp) -> Result<PlaceView, PlaceError> {
		self.edit(editor, brand, slug, Expected::Any, now, |_| Ok(Edit::Withdrawn(withdrawn))).await
	}

	/// One change: the place registered if it was not, the current state read under the write
	/// lock, `expected` checked against it, the change made and journaled — or nothing at all
	/// when it would change nothing.
	async fn edit(
		&self,
		editor: &Editor,
		brand: &BrandId,
		slug: &LocationId,
		expected: Expected,
		now: Timestamp,
		what: impl FnOnce(&PlaceView) -> Result<Edit, PlaceError>,
	) -> Result<PlaceView, PlaceError> {
		let now = micros(now)?;
		let mut tx = self.store.begin_write().await?;
		let current = view(brand, slug, places::place(&mut tx, brand, slug).await?);
		if let Expected::At(seen) = expected
			&& seen.map(micros).transpose()? != current.updated_at
		{
			return Err(PlaceError::Conflict);
		}
		let edit = what(&current)?;
		let (kind, after, reverts) = match edit {
			Edit::Settings { to, kind, reverts } => {
				if to == current.settings && current.updated_at.is_some() {
					return Ok(current);
				}
				(kind, to, reverts)
			}
			Edit::Withdrawn(w) => {
				if w == current.withdrawn {
					return Ok(current);
				}
				(if w { ChangeKind::Withdraw } else { ChangeKind::Restore }, current.settings.clone(), None)
			}
		};
		places::register(&mut tx, brand, slug, now, by(editor)).await?;
		match kind {
			ChangeKind::Withdraw | ChangeKind::Restore => places::set_withdrawn(&mut tx, brand, slug, kind == ChangeKind::Withdraw).await?,
			ChangeKind::Set | ChangeKind::Revert => {
				// Strictly after the last write, so `updated_at` tells every version apart even
				// when two land within a microsecond.
				let at = match current.updated_at {
					Some(last) if last >= now => last.checked_add(jiff::SignedDuration::from_micros(1)).wrap_err("the next updated_at")?,
					_ => now,
				};
				places::write_settings(&mut tx, brand, slug, &after, at, by(editor)).await?;
			}
			ChangeKind::Register => return Err(PlaceError::Internal(eyre::eyre!("a registration is not an edit"))),
		}
		let change = Change {
			id: new_change_id(now),
			at: now,
			kind,
			by: editor.label().to_owned(),
			before: current.settings,
			after,
			reverts,
		};
		places::insert_change(&mut tx, brand, slug, &change, editor.user_id()).await?;
		let stored = places::place(&mut tx, brand, slug).await?;
		tx.commit().await.wrap_err("committing a place's change")?;
		tracing::info!(by = editor.label(), %brand, %slug, kind = kind.as_str(), "place changed");
		self.live.changed(place_changed(brand, slug, now));
		Ok(view(brand, slug, stored))
	}
}
