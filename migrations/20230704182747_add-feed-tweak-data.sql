-- Add migration script here
ALTER TABLE feeds ADD tweaked_profile_data boolean NOT NULL DEFAULT false;

UPDATE feeds SET tweaked_profile_data = false;