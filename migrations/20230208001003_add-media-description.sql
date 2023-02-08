-- Add migration script here
ALTER TABLE enclosures
ADD COLUMN description VARCHAR,
ADD COLUMN credits VARCHAR;
