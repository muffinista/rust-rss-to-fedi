-- Add migration script here
-- Your SQL goes here
CREATE TABLE enclosures (
  id SERIAL PRIMARY KEY,
  item_id INTEGER NOT NULL,
  url VARCHAR NOT NULL,
  content_type VARCHAR,
  size INTEGER NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

