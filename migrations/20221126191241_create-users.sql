-- Add migration script here
-- Your SQL goes here
CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  email VARCHAR NOT NULL,
  login_token VARCHAR NOT NULL,
  access_token VARCHAR,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

--   login_token_expires_at DATETIME
--   access_expires_at DATETIME

