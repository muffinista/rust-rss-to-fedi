-- Add migration script here
ALTER TABLE followers ADD CONSTRAINT feed_and_actor UNIQUE (feed_id, actor);