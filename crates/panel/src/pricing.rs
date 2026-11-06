//! A brand's pricing, as the panel's editor and the CLI change it and the sites read it
//! (`panel_core::pricing`). Every change is one write transaction: the current model and the
//! brand's locales read, the caller's view checked, the new model checked against those locales,
//! written and journaled; then the live topic `pricing`.

use eyre::WrapErr;
use jiff::Timestamp;
use panel_core::{
	ids::BrandId,
	place::Editor,
	pricing::{self, PricingInputs, PricingModel, Problem},
};
use serde_json::Value;
use uuid::Uuid;

use crate::{
	Panel,
	place::{Expected, by, micros, new_change_id},
	store::pricing::{self as stored, ChangeRecord, StoredPricing},
};

/// How many changes the history shows.
pub const HISTORY: u32 = 50;

/// A brand's pricing as the editor shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct PricingView {
	pub brand: BrandId,
	/// The languages every label must be in; fr and en unless set.
	pub locales: Vec<String>,
	/// The model as saved; `None` when never set or removed (the sites keep their baked one).
	pub model: Option<Value>,
	/// `None` while never set: what a first save must expect. A removal has a time too.
	pub updated_at: Option<Timestamp>,
	pub updated_by: Option<String>,
}

/// One entry of the history, in short: what the editor lists.
#[derive(Clone, Debug, PartialEq)]
pub struct PricingChange {
	pub id: Uuid,
	pub at: Timestamp,
	pub by: String,
	pub kind: ChangeKind,
	/// The saved model's `validFrom`; `None` for a removal.
	pub valid_from: Option<String>,
	/// How many needs the saved model prices; `None` for a removal.
	pub needs: Option<usize>,
	/// The model the change left; `None` for a removal.
	pub model: Option<Value>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeKind {
	Set,
	Remove,
}

impl ChangeKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Set => "set",
			Self::Remove => "remove",
		}
	}

	fn parse(raw: &str) -> Option<Self> {
		[Self::Set, Self::Remove].into_iter().find(|k| k.as_str() == raw)
	}
}

/// Why a change to a brand's pricing was not made.
#[derive(Debug, thiserror::Error)]
pub enum PricingError {
	/// The pricing changed since the editor read it; what it is now.
	#[error("the pricing changed since it was read")]
	Stale(Box<PricingView>),
	/// Every reason the model is not one the brand's sites can show (never empty).
	#[error("invalid pricing model: {0:?}")]
	Invalid(Vec<Problem>),
	#[error(transparent)]
	Internal(#[from] eyre::Report),
}

fn view(brand: &BrandId, s: StoredPricing) -> PricingView {
	let (model, updated_at, updated_by) = match s.current {
		Some(c) => (c.model, Some(c.updated_at), Some(c.updated_by)),
		None => (None, None, None),
	};
	PricingView {
		brand: brand.clone(),
		locales: s.locales.unwrap_or_else(pricing::default_locales),
		model,
		updated_at,
		updated_by,
	}
}

fn pricing_changed(brand: &BrandId, at: Timestamp) -> crate::live::Change {
	crate::live::Change {
		topic: crate::live::Topic::Pricing,
		brand: Some(brand.clone()),
		id: None,
		at,
	}
}

impl Panel {
	/// Every brand the panel knows, with its pricing.
	pub async fn all_pricing(&self) -> eyre::Result<Vec<PricingView>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		Ok(stored::all(&mut conn).await?.into_iter().map(|(brand, s)| view(&brand, s)).collect())
	}

	/// A brand's pricing; a brand the panel does not know has none, in the default locales.
	pub async fn pricing(&self, brand: &BrandId) -> eyre::Result<PricingView> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		Ok(view(brand, stored::pricing(&mut conn, brand).await?))
	}

	/// What a site is answered: the model, in its wire shape, when there is one its sites can
	/// show today. One that no longer passes (the rules or the brand's locales changed since
	/// it was saved) is `None`, as the site would refuse it anyway, and logged.
	pub async fn live_pricing(&self, brand: &BrandId) -> eyre::Result<Option<Value>> {
		let v = self.pricing(brand).await?;
		let Some(model) = v.model else { return Ok(None) };
		match PricingModel::parse_for(&model, &v.locales) {
			Ok(m) => Ok(Some(m.to_json())),
			Err(problems) => {
				let first = problems.first().map(ToString::to_string).unwrap_or_default();
				tracing::warn!(%brand, problem = first, "the saved pricing model does not pass today: the sites keep their baked one");
				Ok(None)
			}
		}
	}

	/// What `need` costs under `model` (a draft, not saved) for these answers — exactly as the
	/// brand's sites would price it, or why they would refuse the model.
	pub async fn preview_price(&self, brand: &BrandId, model: &Value, need: &str, answers: &PricingInputs) -> Result<Option<i64>, PricingError> {
		let locales = self.pricing(brand).await?.locales;
		let model = PricingModel::parse_for(model, &locales).map_err(PricingError::Invalid)?;
		Ok(pricing::price_of(&model, need, answers))
	}

	/// The brand's last [`HISTORY`] changes, newest first.
	pub async fn pricing_history(&self, brand: &BrandId) -> eyre::Result<Vec<PricingChange>> {
		let mut conn = self.store.pool().acquire().await.wrap_err("a connection")?;
		stored::changes(&mut conn, brand, HISTORY)
			.await?
			.into_iter()
			.map(|c| {
				let kind = ChangeKind::parse(&c.kind).ok_or_else(|| eyre::eyre!("stored pricing change kind {:?}", c.kind))?;
				let after = c.after.as_ref();
				Ok(PricingChange {
					id: c.id,
					at: c.at,
					by: c.by,
					kind,
					valid_from: after.and_then(|m| m.get("validFrom")).and_then(Value::as_str).map(str::to_owned),
					needs: after.and_then(|m| m.get("needs")).and_then(Value::as_object).map(serde_json::Map::len),
					model: c.after,
				})
			})
			.collect()
	}

	/// Saves `model` as the brand's, whole: checked against the brand's locales, stored in
	/// its wire shape. Saving the model there is changes nothing and writes nothing.
	pub async fn set_pricing(&self, editor: &Editor, brand: &BrandId, model: &Value, expected: Expected, now: Timestamp) -> Result<PricingView, PricingError> {
		self.change_pricing(editor, brand, expected, now, |current| {
			let model = PricingModel::parse_for(model, &current.locales).map_err(PricingError::Invalid)?;
			Ok(Some(model.to_json()))
		})
		.await
	}

	/// Takes the brand's model off its sites: they go back to their baked one.
	pub async fn remove_pricing(&self, editor: &Editor, brand: &BrandId, expected: Expected, now: Timestamp) -> Result<PricingView, PricingError> {
		self.change_pricing(editor, brand, expected, now, |_| Ok(None)).await
	}

	/// One change: the current pricing read under the write lock, `expected` checked against
	/// it, the new model (`None`: removed) written and journaled — or nothing at all when it
	/// would change nothing.
	async fn change_pricing(
		&self,
		editor: &Editor,
		brand: &BrandId,
		expected: Expected,
		now: Timestamp,
		to: impl FnOnce(&PricingView) -> Result<Option<Value>, PricingError>,
	) -> Result<PricingView, PricingError> {
		let now = micros(now)?;
		let mut tx = self.store.begin_write().await?;
		let current = view(brand, stored::pricing(&mut tx, brand).await?);
		if let Expected::At(seen) = expected
			&& seen.map(micros).transpose()? != current.updated_at
		{
			return Err(PricingError::Stale(Box::new(current)));
		}
		let after = to(&current)?;
		if after == current.model {
			return Ok(current);
		}
		// Strictly after the last write, so `updated_at` tells every version apart even when two
		// land within a microsecond.
		let at = match current.updated_at {
			Some(last) if last >= now => last.checked_add(jiff::SignedDuration::from_micros(1)).wrap_err("the next updated_at")?,
			_ => now,
		};
		stored::write(&mut tx, brand, after.as_ref(), at, by(editor)).await?;
		let kind = if after.is_some() { ChangeKind::Set } else { ChangeKind::Remove };
		let change = ChangeRecord {
			id: new_change_id(now),
			at,
			kind: kind.as_str().to_owned(),
			by: editor.label().to_owned(),
			before: current.model,
			after,
		};
		stored::insert_change(&mut tx, brand, &change, editor.user_id()).await?;
		let saved = view(brand, stored::pricing(&mut tx, brand).await?);
		tx.commit().await.wrap_err("committing a brand's pricing")?;
		tracing::info!(by = editor.label(), %brand, kind = kind.as_str(), "pricing changed");
		self.live.changed(pricing_changed(brand, at));
		Ok(saved)
	}

	/// Sets the locales a brand's sites speak (every label of its model must be in each). The
	/// saved model is not checked again here: one missing a new locale stops being served
	/// ([`Panel::live_pricing`]) until it is saved with the labels.
	pub async fn set_brand_locales(&self, editor: &Editor, brand: &BrandId, locales: &[String], now: Timestamp) -> eyre::Result<PricingView> {
		let now = micros(now)?;
		let mut tx = self.store.begin_write().await?;
		let current = view(brand, stored::pricing(&mut tx, brand).await?);
		if current.locales == locales {
			return Ok(current);
		}
		stored::set_locales(&mut tx, brand, locales, now, by(editor)).await?;
		let saved = view(brand, stored::pricing(&mut tx, brand).await?);
		tx.commit().await.wrap_err("committing a brand's locales")?;
		tracing::info!(by = editor.label(), %brand, locales = locales.join(","), "brand locales changed");
		self.live.changed(pricing_changed(brand, now));
		Ok(saved)
	}
}
