-- Add migration script here
ALTER TABLE feeds ADD error_count INT NOT NULL DEFAULT 0;
