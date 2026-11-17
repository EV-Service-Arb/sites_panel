-- A brand's Google Calendar, as its owner consented to it through the editor: the calendar read
-- and the refresh token, sealed under PANEL_DATA_KEY (the brand as associated data), the key's
-- fingerprint beside it. Replaced by the next consent.
CREATE TABLE google_calendars (
	brand_id TEXT PRIMARY KEY,
	calendar_id TEXT NOT NULL CHECK (length(calendar_id) BETWEEN 1 AND 1024),
	refresh_sealed BLOB NOT NULL,
	data_key_fp BLOB NOT NULL,
	connected_at INTEGER NOT NULL,
	connected_by TEXT NOT NULL
) STRICT;
