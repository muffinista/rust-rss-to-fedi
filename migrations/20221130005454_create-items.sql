-- Add migration script here
-- Your SQL goes here
CREATE TABLE items (
  id SERIAL PRIMARY KEY,
  feed_id INTEGER NOT NULL,
  guid VARCHAR NOT NULL,
  title VARCHAR,
  content VARCHAR,
  url VARCHAR,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL

);

--   login_token_expires_at DATETIME
--   access_expires_at DATETIME

