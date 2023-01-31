-- Add migration script here

ALTER TABLE users ADD CONSTRAINT email UNIQUE (email);
ALTER TABLE users ADD CONSTRAINT actor_url UNIQUE (actor_url);

ALTER TABLE feeds ADD CONSTRAINT name UNIQUE (name);
