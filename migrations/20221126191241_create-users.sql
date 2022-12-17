-- Add migration script here
-- Your SQL goes here
CREATE TABLE users (
  id INTEGER PRIMARY KEY,
  email VARCHAR NOT NULL,
  login_token VARCHAR NOT NULL,
  access_token VARCHAR,
  created_at DATETIME NOT NULL,
  updated_at DATETIME NOT NULL
);

--   login_token_expires_at DATETIME
--   access_expires_at DATETIME

