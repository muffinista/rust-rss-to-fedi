-- Add migration script here
ALTER TABLE users ADD COLUMN username VARCHAR NULL;
ALTER TABLE actors ADD COLUMN username VARCHAR NULL;
