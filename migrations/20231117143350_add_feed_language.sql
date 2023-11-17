-- Add migration script here
ALTER TABLE feeds ADD COLUMN language VARCHAR NOT NULL DEFAULT 'en';

