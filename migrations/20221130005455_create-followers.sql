-- Add migration script here
-- Your SQL goes here
CREATE TABLE followers (
  id INTEGER PRIMARY KEY,
  feed_id INTEGER NOT NULL,
  actor VARCHAR NOT NULL
);

