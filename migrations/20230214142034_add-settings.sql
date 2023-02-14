-- Add migration script here
CREATE TABLE settings (
  name VARCHAR PRIMARY KEY NOT NULL,
  value VARCHAR NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

INSERT INTO settings (name, value, created_at, updated_at) VALUES ('signups_enabled', 'true', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
