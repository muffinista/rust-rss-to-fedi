-- Add migration script here
-- Your SQL goes here
CREATE TABLE items (
  id INTEGER PRIMARY KEY,
  feed_id INTEGER NOT NULL,
  guid VARCHAR NOT NULL,
  title VARCHAR,
  content VARCHAR
);

--   login_token_expires_at DATETIME
--   access_expires_at DATETIME

