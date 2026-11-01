-- Which GMB a place is the landing of. The id is the conductor's (`gmbs.id`), held as opaque:
-- sites never checks it exists. One GMB lands on at most one place.
ALTER TABLE places ADD COLUMN gmb_id INTEGER;

CREATE UNIQUE INDEX places_by_gmb ON places (gmb_id) WHERE gmb_id IS NOT NULL;
