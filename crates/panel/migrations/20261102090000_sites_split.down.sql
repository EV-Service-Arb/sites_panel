-- no-transaction
-- Back to the schema before the split, the conductor's tables empty: their rows went with the
-- conductor's copy. `events.key_id` references `sources` again; with no keys there, the events a
-- key wrote reference nothing until the keys are put back.

PRAGMA foreign_keys = OFF;

BEGIN IMMEDIATE;

DROP TABLE notifications;
DROP INDEX fanout_by_time;
ALTER TABLE fanout RENAME TO telegram_fanout;
CREATE INDEX telegram_fanout_by_time ON telegram_fanout (fanned_at);

CREATE TABLE sources (
	key_id TEXT PRIMARY KEY CHECK (length(key_id) BETWEEN 1 AND 64 AND key_id GLOB '[a-z0-9]*' AND key_id NOT GLOB '*[^a-z0-9_-]*'),
	kind TEXT NOT NULL CHECK (kind IN ('site', 'review_archive', 'gbp', 'posthog', 'panel', 'sheet', 'telephony', 'bot')),
	-- A JSON array of brand ids, never empty.
	brand_ids TEXT NOT NULL CHECK (json_valid(brand_ids) AND json_type(brand_ids) = 'array' AND json_array_length(brand_ids) > 0),
	-- The HMAC secret, sealed under PANEL_DATA_KEY with the key id as associated data.
	secret_sealed BLOB NOT NULL,
	-- Which data key sealed it (a fingerprint, not the key), so a rotated key fails by name.
	data_key_fp BLOB NOT NULL CHECK (length(data_key_fp) = 32),
	created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000000 AS INTEGER)),
	revoked_at INTEGER
) STRICT;

CREATE TRIGGER sources_no_delete BEFORE DELETE ON sources
BEGIN
	SELECT RAISE(ABORT, 'sources are never deleted: DELETE refused');
END;

CREATE TRIGGER sources_only_revoked BEFORE UPDATE ON sources
WHEN NEW.key_id IS NOT OLD.key_id OR NEW.kind IS NOT OLD.kind OR NEW.brand_ids IS NOT OLD.brand_ids
	OR NEW.secret_sealed IS NOT OLD.secret_sealed OR NEW.data_key_fp IS NOT OLD.data_key_fp OR NEW.created_at IS NOT OLD.created_at
BEGIN
	SELECT RAISE(ABORT, 'a source only ever gets revoked: UPDATE refused');
END;

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
	-- The key that signed it; NULL for events the panel writes itself.
	key_id TEXT REFERENCES sources (key_id),
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

CREATE TABLE sessions (
	-- SHA-256 of the session cookie's value: a dump does not hand out sessions.
	id_hash BLOB PRIMARY KEY CHECK (length(id_hash) = 32),
	-- The concierge user id, the tokens' `sub`.
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	-- The relying party's access and refresh tokens, sealed under PANEL_DATA_KEY
	-- (XChaCha20-Poly1305, the row's id_hash bound as associated data).
	access_sealed BLOB NOT NULL,
	access_expires_at INTEGER NOT NULL,
	refresh_sealed BLOB NOT NULL,
	data_key_fp BLOB NOT NULL CHECK (length(data_key_fp) = 32),
	created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000000 AS INTEGER)),
	-- The refresh family's deadline: past it the session is over, whatever the cookie says.
	expires_at INTEGER NOT NULL,
	-- Whoever set this (and it is in the future) is rotating the session's tokens: the others
	-- wait for the result instead of presenting the same refresh token a second time.
	rotating_until INTEGER,
	-- The last request the user made with the session.
	last_seen_at INTEGER
) STRICT;

CREATE INDEX sessions_by_expiry ON sessions (expires_at);
-- Sign-out closes every session of the user.
CREATE INDEX sessions_by_user ON sessions (user_id);

CREATE TABLE consumed_states (
	state_hash BLOB PRIMARY KEY CHECK (length(state_hash) = 32),
	expires_at INTEGER NOT NULL
) STRICT;

CREATE INDEX consumed_states_by_expiry ON consumed_states (expires_at);

CREATE TABLE telegram_link_tokens (
	token_hash BLOB PRIMARY KEY CHECK (length(token_hash) = 32),
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	permissions TEXT NOT NULL CHECK (json_valid(permissions) AND json_type(permissions) = 'array'),
	display_name TEXT NOT NULL,
	expires_at INTEGER NOT NULL,
	issued_at INTEGER
) STRICT;

CREATE INDEX telegram_link_tokens_by_expiry ON telegram_link_tokens (expires_at);

CREATE TABLE telegram_links (
	user_id BLOB PRIMARY KEY CHECK (length(user_id) = 16),
	chat_id INTEGER NOT NULL UNIQUE,
	linked_at INTEGER NOT NULL,
	-- What concierge last confirmed the user holds, and when; NULL once their access is known
	-- lost. Nothing is sent on a confirmation older than the engine's ACCESS_TTL.
	permissions TEXT CHECK (json_valid(permissions) AND json_type(permissions) = 'array'),
	permissions_checked_at INTEGER NOT NULL,
	access_tried_at INTEGER,
	display_name TEXT NOT NULL,
	dead_at INTEGER,
	tg_username TEXT,
	tg_first_name TEXT
) STRICT;

CREATE TABLE telegram_rules (
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	rule TEXT NOT NULL CHECK (rule IN ('new_lead', 'contact_overdue', 'payment_received', 'source_silent', 'booked', 'access_requested')),
	enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
	PRIMARY KEY (user_id, rule)
) STRICT;

CREATE TABLE telegram_outbox (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	chat_id INTEGER NOT NULL,
	rule TEXT NOT NULL,
	event_id BLOB NOT NULL,
	-- The lead the buttons act on.
	brand_id TEXT,
	lead_id TEXT,
	-- A JSON array of button names.
	buttons TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(buttons) AND json_type(buttons) = 'array'),
	text_sealed BLOB,
	data_key_fp BLOB CHECK (length(data_key_fp) = 32),
	state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'sent', 'dead')),
	attempts INTEGER NOT NULL DEFAULT 0,
	next_attempt_at INTEGER NOT NULL,
	-- A sender holds it until then; a lease past its time is taken over.
	leased_until INTEGER,
	-- When the last try started: the pacing counts these.
	attempted_at INTEGER,
	sent_at INTEGER,
	tg_message_id INTEGER,
	last_error TEXT,
	created_at INTEGER NOT NULL,
	UNIQUE (rule, event_id, chat_id),
	CHECK ((state = 'pending') = (text_sealed IS NOT NULL)),
	CHECK ((text_sealed IS NULL) = (data_key_fp IS NULL))
) STRICT;

CREATE INDEX telegram_outbox_due ON telegram_outbox (next_attempt_at, id) WHERE state = 'pending';
CREATE INDEX telegram_outbox_by_attempt ON telegram_outbox (attempted_at);
CREATE INDEX telegram_outbox_by_chat ON telegram_outbox (chat_id, attempted_at);

CREATE TABLE telegram_poller (
	id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
	holder BLOB,
	leased_until INTEGER,
	next_update_id INTEGER NOT NULL DEFAULT 0,
	-- Telegram asked the bot to wait (429): nothing leaves the outbox before this.
	outbox_paused_until INTEGER
) STRICT;

INSERT INTO telegram_poller DEFAULT VALUES;

CREATE TABLE telegram_replies (
	chat_id INTEGER PRIMARY KEY,
	last_at INTEGER NOT NULL
) STRICT;

CREATE TABLE access_requests (
	user_id BLOB NOT NULL CHECK (length(user_id) = 16),
	need TEXT NOT NULL,
	email TEXT NOT NULL,
	name TEXT NOT NULL,
	requested_at INTEGER NOT NULL,
	-- What the admins' message fans out under; a new one each time the request is refreshed.
	event_id BLOB NOT NULL UNIQUE CHECK (length(event_id) = 16),
	PRIMARY KEY (user_id, need)
) STRICT;

CREATE INDEX access_requests_by_time ON access_requests (requested_at);

COMMIT;

PRAGMA foreign_keys = ON;
