-- Add migration script here
-- Your SQL goes here
CREATE TABLE feeds (
  id SERIAL PRIMARY KEY,
  user_id INTEGER NOT NULL,
  name VARCHAR NOT NULL,
  url VARCHAR NOT NULL,
  error VARCHAR,
  public_key VARCHAR NOT NULL,
  private_key VARCHAR NOT NULL,
  image_url VARCHAR,
  icon_url VARCHAR,

  title VARCHAR,
  description VARCHAR,
  site_url VARCHAR,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
  refreshed_at TIMESTAMP WITH TIME ZONE NOT NULL
);

