-- Forgets which GMB each place is the landing of.
DROP INDEX places_by_gmb;
ALTER TABLE places DROP COLUMN gmb_id;
