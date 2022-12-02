-- Add migration script here
-- Your SQL goes here
CREATE TABLE feeds (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL,
  name VARCHAR NOT NULL,
  url VARCHAR NOT NULL
);

--   login_token_expires_at DATETIME
--   access_expires_at DATETIME

