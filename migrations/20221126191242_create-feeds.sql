-- Add migration script here
-- Your SQL goes here
CREATE TABLE feeds (
  id INTEGER PRIMARY KEY,
  user_id INTEGER NOT NULL,
  name VARCHAR NOT NULL,
  url VARCHAR NOT NULL,
  public_key VARCHAR NOT NULL,
  private_key VARCHAR NOT NULL,
  image_url VARCHAR,
  icon_url VARCHAR,

  title VARCHAR,
  description VARCHAR,
  site_url VARCHAR,
  created_at DATETIME NOT NULL,
  updated_at DATETIME NOT NULL

);

