-- Add migration script here

ALTER TABLE users ADD CONSTRAINT email UNIQUE (email);


ALTER TABLE feeds ADD CONSTRAINT name UNIQUE (name);
