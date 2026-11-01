-- no-transaction
-- sites_panel keeps the sites' tables; signing in, the access requests, the source keys and the
-- Telegram hub are the conductor's, which starts from a copy of the same file. The journal keeps
-- each event's `key_id`; the keys themselves are the conductor's, so `events` is made again
-- without its reference to `sources` (SQLite cannot drop a foreign key): every row copied as it
-- is, its indexes, both append-only triggers and the view over it made again, foreign keys off
-- outside the one IMMEDIATE transaction (PRAGMA foreign_keys is a no-op inside one).
-- The fan-out's claims stay (what was told is not told again), for the sites' rules only, beside
-- the notes the conductor reads.

PRAGMA foreign_keys = OFF;

BEGIN IMMEDIATE;

DROP TABLE sessions;
DROP TABLE consumed_states;
DROP TABLE access_requests;
DROP TABLE telegram_link_tokens;
DROP TABLE telegram_links;
DROP TABLE telegram_rules;
DROP TABLE telegram_outbox;
DROP TABLE telegram_poller;
DROP TABLE telegram_replies;

DROP VIEW reporting_ingest_daily;

CREATE TABLE events_new (
	id BLOB PRIMARY KEY CHECK (length(id) = 16),
	schema TEXT NOT NULL,
	type TEXT NOT NULL,
	type_version INTEGER NOT NULL CHECK (type_version > 0),
	occurred_at INTEGER NOT NULL,
	received_at INTEGER NOT NULL,
	source_kind TEXT NOT NULL CHECK (source_kind IN ('site', 'review_archive', 'gbp', 'posthog', 'panel', 'sheet', 'telephony', 'booking', 'bot')),
	source_id TEXT NOT NULL,
	-- The key the conductor authenticated it under; NULL for events sites writes itself.
	key_id TEXT,
	brand_id TEXT NOT NULL,
	location_id TEXT,
	lead_id TEXT,
	job_id TEXT,
	properties TEXT NOT NULL CHECK (json_valid(properties)),
	pii_sealed BLOB,
	data_key_fp BLOB CHECK (length(data_key_fp) = 32),
	-- HMAC-SHA256 of the event in canonical form, under a key derived from PANEL_DATA_KEY.
	content_mac BLOB NOT NULL CHECK (length(content_mac) = 32),
	status TEXT NOT NULL CHECK (status IN ('registered', 'unregistered', 'invalid')),
	status_reason TEXT,
	manual INTEGER GENERATED ALWAYS AS (source_kind = 'panel') STORED,
	CHECK ((pii_sealed IS NULL) = (data_key_fp IS NULL))
) STRICT;

INSERT INTO events_new (id, schema, type, type_version, occurred_at, received_at, source_kind, source_id, key_id, brand_id, location_id,
	lead_id, job_id, properties, pii_sealed, data_key_fp, content_mac, status, status_reason)
SELECT id, schema, type, type_version, occurred_at, received_at, source_kind, source_id, key_id, brand_id, location_id,
	lead_id, job_id, properties, pii_sealed, data_key_fp, content_mac, status, status_reason
FROM events;

DROP TABLE events;
ALTER TABLE events_new RENAME TO events;

CREATE INDEX events_by_lead ON events (brand_id, lead_id, occurred_at, id) WHERE lead_id IS NOT NULL;
CREATE INDEX events_in_order ON events (occurred_at, id);
CREATE INDEX events_not_registered ON events (type, type_version) WHERE status <> 'registered';
CREATE INDEX events_by_key ON events (key_id, received_at) WHERE key_id IS NOT NULL;
CREATE INDEX events_of_experiments ON events (brand_id, occurred_at, id)
WHERE type IN ('experiments.declared', 'experiment.configured');

CREATE TRIGGER events_no_delete BEFORE DELETE ON events
BEGIN
	SELECT RAISE(ABORT, 'events is append-only: DELETE refused');
END;

CREATE TRIGGER events_only_status BEFORE UPDATE ON events
WHEN NEW.id IS NOT OLD.id OR NEW.schema IS NOT OLD.schema OR NEW.type IS NOT OLD.type OR NEW.type_version IS NOT OLD.type_version
	OR NEW.occurred_at IS NOT OLD.occurred_at OR NEW.received_at IS NOT OLD.received_at
	OR NEW.source_kind IS NOT OLD.source_kind OR NEW.source_id IS NOT OLD.source_id OR NEW.key_id IS NOT OLD.key_id
	OR NEW.brand_id IS NOT OLD.brand_id OR NEW.location_id IS NOT OLD.location_id OR NEW.lead_id IS NOT OLD.lead_id OR NEW.job_id IS NOT OLD.job_id
	OR NEW.properties IS NOT OLD.properties OR NEW.pii_sealed IS NOT OLD.pii_sealed OR NEW.data_key_fp IS NOT OLD.data_key_fp
	OR NEW.content_mac IS NOT OLD.content_mac
BEGIN
	SELECT RAISE(ABORT, 'events is append-only: UPDATE refused');
END;

CREATE VIEW reporting_ingest_daily AS
SELECT date(received_at / 1000000, 'unixepoch') AS day, brand_id, source_kind, source_id, type, type_version, status,
	count(*) AS events
FROM events
GROUP BY 1, 2, 3, 4, 5, 6, 7;

DROP TABLE sources;

DELETE FROM telegram_fanout WHERE rule NOT IN ('new_lead', 'contact_overdue', 'payment_received', 'booked');
DROP INDEX telegram_fanout_by_time;
ALTER TABLE telegram_fanout RENAME TO fanout;
CREATE INDEX fanout_by_time ON fanout (fanned_at);

-- What the conductor's Telegram reads after its cursor (`seq`, never handed out twice). The
-- texts carry the customer's PII, so they are sealed under PANEL_DATA_KEY; a note waits a day.
CREATE TABLE notifications (
	seq INTEGER PRIMARY KEY AUTOINCREMENT,
	rule TEXT NOT NULL CHECK (rule IN ('new_lead', 'contact_overdue', 'payment_received', 'booked')),
	event_id BLOB NOT NULL CHECK (length(event_id) = 16),
	created_at INTEGER NOT NULL,
	-- The lead the buttons act on.
	brand_id TEXT,
	lead_id TEXT,
	-- A JSON array of button names.
	buttons TEXT NOT NULL CHECK (json_valid(buttons) AND json_type(buttons) = 'array'),
	-- Whoever did what the note tells: they are not told.
	skip_user BLOB CHECK (length(skip_user) = 16),
	-- {text, text_pii}, each {text, code}, sealed with the rule and the event bound.
	texts_sealed BLOB NOT NULL,
	data_key_fp BLOB NOT NULL CHECK (length(data_key_fp) = 32),
	UNIQUE (rule, event_id),
	CHECK ((brand_id IS NULL) = (lead_id IS NULL))
) STRICT;

CREATE INDEX notifications_by_time ON notifications (created_at);

COMMIT;

PRAGMA foreign_keys = ON;
