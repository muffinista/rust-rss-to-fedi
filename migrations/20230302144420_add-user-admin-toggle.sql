-- Add migration script here
ALTER TABLE users ADD COLUMN admin boolean NOT NULL DEFAULT false;
