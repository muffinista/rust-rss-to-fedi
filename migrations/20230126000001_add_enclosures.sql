-- Add migration script here
-- Your SQL goes here
ALTER TABLE items ADD enclosure_url VARCHAR NULL;
ALTER TABLE items ADD enclosure_content_type VARCHAR NULL;
ALTER TABLE items ADD enclosure_size INTEGER NULL;
