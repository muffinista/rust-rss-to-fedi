-- Add migration script here
DELETE FROM actors;
ALTER TABLE actors ADD COLUMN inbox_url VARCHAR NOT NULL;