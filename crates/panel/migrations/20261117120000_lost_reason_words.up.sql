-- A lead's lost reason is one of the panel's words (LostReason): a journaled or projected one
-- outside them stops the migration, for a person to look at, before the typed fold refuses it.
CREATE TEMP TABLE lost_reasons (
	reason TEXT NOT NULL CHECK (reason IN ('too_expensive', 'no_answer', 'went_elsewhere', 'out_of_area', 'not_needed', 'spam', 'other'))
);
INSERT INTO lost_reasons SELECT json_extract(properties, '$.reason') FROM events WHERE type = 'lead.lost';
INSERT INTO lost_reasons SELECT lost_reason FROM leads WHERE lost_reason IS NOT NULL;
DROP TABLE lost_reasons;
