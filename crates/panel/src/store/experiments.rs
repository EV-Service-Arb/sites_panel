//! `experiments`: each brand's experiments folded from its `experiments.declared` and
//! `experiment.configured` events ([`panel_core::experiment::fold`]), recomputed whole in the
//! transaction that journals one, and by the rebuild.

use eyre::WrapErr;
use panel_core::{
	experiment::{self, Declaration, ExperimentKey, Happened, Override, State, Variant},
	fact::Fact,
};
use sa_core::ids::BrandId;
use sqlx::{SqliteConnection, types::Json};

use super::{events, from_db, projections::registered, to_db};

/// Recomputes a brand's experiments from all its events about them. Inside a write
/// transaction, as a lead's recompute is.
pub async fn recompute(conn: &mut SqliteConnection, brand: &BrandId) -> eyre::Result<()> {
	let happened: Vec<_> = events::of_experiments(conn, brand)
		.await?
		.into_iter()
		.filter_map(|stored| {
			let r = registered(stored)?;
			let h = match r.fact {
				Fact::ExperimentsDeclared(d) => Happened::Declared(d),
				Fact::ExperimentConfigured { patch, by } => Happened::Configured { patch, by },
				// The query selects these two types only.
				_ => return None,
			};
			Some((r.occurred_at, r.id, h))
		})
		.collect();
	sqlx::query("DELETE FROM experiments WHERE brand_id = $1")
		.bind(brand.as_str())
		.execute(&mut *conn)
		.await
		.wrap_err("clearing a brand's experiments")?;
	for (key, s) in experiment::fold(&happened) {
		let o = s.over.as_ref();
		sqlx::query(
			"INSERT INTO experiments (brand_id, key, variants, declared_weights, declared_enabled, declared_holdout, summary, declared_at, \
			 first_declared_at, override_enabled, override_weights, override_holdout, changed_by, changed_at, weights_changed_at, retired) \
			 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)",
		)
		.bind(brand.as_str())
		.bind(key.as_str())
		.bind(Json(&s.declared.variants))
		.bind(Json(&s.declared.weights))
		.bind(s.declared.enabled)
		.bind(s.declared.holdout)
		.bind(s.declared.summary.as_deref())
		.bind(to_db(s.declared_at))
		.bind(to_db(s.first_declared_at))
		.bind(o.and_then(|o| o.enabled))
		.bind(o.and_then(|o| o.weights.as_ref()).map(Json))
		.bind(o.and_then(|o| o.holdout))
		.bind(o.map(|o| o.changed_by.as_str()))
		.bind(o.map(|o| to_db(o.changed_at)))
		.bind(s.weights_changed_at.map(to_db))
		.bind(s.retired)
		.execute(&mut *conn)
		.await
		.wrap_err_with(|| format!("writing experiment {brand}/{key}"))?;
	}
	Ok(())
}

#[derive(sqlx::FromRow)]
struct Row {
	brand_id: String,
	key: String,
	variants: Json<Vec<Variant>>,
	declared_weights: Json<Vec<f64>>,
	declared_enabled: bool,
	declared_holdout: Option<f64>,
	summary: Option<String>,
	declared_at: i64,
	first_declared_at: i64,
	override_enabled: Option<bool>,
	override_weights: Option<Json<Vec<f64>>>,
	override_holdout: Option<f64>,
	changed_by: Option<String>,
	changed_at: Option<i64>,
	weights_changed_at: Option<i64>,
	retired: bool,
}

impl TryFrom<Row> for (BrandId, State) {
	type Error = eyre::Report;

	fn try_from(r: Row) -> eyre::Result<Self> {
		let brand = BrandId::parse(&r.brand_id).wrap_err_with(|| format!("stored brand of experiment {}", r.key))?;
		let over = match (r.changed_by, r.changed_at) {
			(Some(by), Some(at)) => Some(Override {
				enabled: r.override_enabled,
				weights: r.override_weights.map(|w| w.0),
				holdout: r.override_holdout,
				changed_by: by,
				changed_at: from_db(at)?,
			}),
			_ => None,
		};
		let state = State {
			// Written from a checked declaration; read back as is.
			declared: Declaration {
				key: ExperimentKey::parse(&r.key).wrap_err("a stored experiment's key")?,
				variants: r.variants.0,
				weights: r.declared_weights.0,
				enabled: r.declared_enabled,
				holdout: r.declared_holdout,
				summary: r.summary,
			},
			declared_at: from_db(r.declared_at)?,
			first_declared_at: from_db(r.first_declared_at)?,
			over,
			weights_changed_at: r.weights_changed_at.map(from_db).transpose()?,
			retired: r.retired,
		};
		Ok((brand, state))
	}
}

/// The experiments of one brand, or of every brand; by brand, then key.
pub async fn list(conn: &mut SqliteConnection, brand: Option<&BrandId>) -> eyre::Result<Vec<(BrandId, State)>> {
	sqlx::query_as::<_, Row>(
		"SELECT brand_id, key, variants, declared_weights, declared_enabled, declared_holdout, summary, declared_at, first_declared_at, \
		 override_enabled, override_weights, override_holdout, changed_by, changed_at, weights_changed_at, retired \
		 FROM experiments WHERE $1 IS NULL OR brand_id = $1 ORDER BY brand_id, key",
	)
	.bind(brand.map(BrandId::as_str))
	.fetch_all(&mut *conn)
	.await
	.wrap_err("reading the experiments")?
	.into_iter()
	.map(<(BrandId, State)>::try_from)
	.collect()
}
