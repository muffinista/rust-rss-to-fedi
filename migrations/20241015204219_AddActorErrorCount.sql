-- Add migration script here
ALTER TABLE actors ADD error_count INT NOT NULL DEFAULT 0;

CREATE INDEX actors_with_errors ON actors(error_count);

