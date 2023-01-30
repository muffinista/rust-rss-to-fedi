-- Your SQL goes here
CREATE TABLE actors (
  url VARCHAR PRIMARY KEY,
  error VARCHAR,
  public_key_id VARCHAR NOT NULL,
  public_key VARCHAR NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
  refreshed_at TIMESTAMP WITH TIME ZONE NOT NULL
);

