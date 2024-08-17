-- Add migration script here
CREATE INDEX actors_url on actors(url);
CREATE INDEX actors_inbox_url on actors(inbox_url);
CREATE INDEX actors_public_key_id on actors(public_key_id);
