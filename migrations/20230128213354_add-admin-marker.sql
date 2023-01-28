-- Add migration script here
ALTER TABLE feeds ADD COLUMN admin BOOLEAN NOT NULL DEFAULT FALSE;