//! `google_calendars`: each brand's consented calendar, its refresh token sealed.

use eyre::WrapErr;
use jiff::Timestamp;
use sa_core::ids::BrandId;
use sqlx::SqliteConnection;

use super::{from_db, to_db};

/// A brand's consent as stored: the token still sealed.
pub struct Stored {
	pub brand: BrandId,
	pub calendar_id: String,
	pub refresh_sealed: Vec<u8>,
	pub data_key_fp: Vec<u8>,
	pub connected_at: Timestamp,
	pub connected_by: String,
}

/// The brand's consent, replacing the one before.
pub async fn put(conn: &mut SqliteConnection, s: &Stored) -> eyre::Result<()> {
	sqlx::query(
		"INSERT INTO google_calendars (brand_id, calendar_id, refresh_sealed, data_key_fp, connected_at, connected_by) VALUES ($1, $2, $3, $4, $5, $6) \
		 ON CONFLICT (brand_id) DO UPDATE SET calendar_id = excluded.calendar_id, refresh_sealed = excluded.refresh_sealed, \
		 data_key_fp = excluded.data_key_fp, connected_at = excluded.connected_at, connected_by = excluded.connected_by",
	)
	.bind(s.brand.as_str())
	.bind(&s.calendar_id)
	.bind(&s.refresh_sealed)
	.bind(&s.data_key_fp)
	.bind(to_db(s.connected_at))
	.bind(&s.connected_by)
	.execute(&mut *conn)
	.await
	.wrap_err_with(|| format!("storing {}'s Google Calendar", s.brand))?;
	Ok(())
}

/// Every brand's consent, by brand.
pub async fn all(conn: &mut SqliteConnection) -> eyre::Result<Vec<Stored>> {
	type Row = (String, String, Vec<u8>, Vec<u8>, i64, String);
	let rows: Vec<Row> = sqlx::query_as("SELECT brand_id, calendar_id, refresh_sealed, data_key_fp, connected_at, connected_by FROM google_calendars ORDER BY brand_id")
		.fetch_all(&mut *conn)
		.await
		.wrap_err("reading the Google Calendars")?;
	rows.into_iter()
		.map(|(brand, calendar_id, refresh_sealed, data_key_fp, at, connected_by)| {
			Ok(Stored {
				brand: BrandId::parse(&brand).wrap_err("a stored Google Calendar's brand")?,
				calendar_id,
				refresh_sealed,
				data_key_fp,
				connected_at: from_db(at)?,
				connected_by,
			})
		})
		.collect()
}
