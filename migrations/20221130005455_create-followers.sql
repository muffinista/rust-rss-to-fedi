-- Add migration script here
-- Your SQL goes here
CREATE TABLE followers (
  id SERIAL PRIMARY KEY,
  feed_id INTEGER NOT NULL,
  actor VARCHAR NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

