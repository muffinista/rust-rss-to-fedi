-- Add migration script here
CREATE TABLE messages (
  id SERIAL PRIMARY KEY,
  username VARCHAR NOT NULL,
  text VARCHAR NOT NULL,
  actor VARCHAR NULL,
  error VARCHAR NULL,
  handled boolean NOT NULL DEFAULT false,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

