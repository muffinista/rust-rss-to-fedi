-- Your SQL goes here
CREATE TABLE users (
  id INT PRIMARY KEY,
  email VARCHAR NOT NULL,
  body TEXT NOT NULL,
  published BOOLEAN NOT NULL DEFAULT FALSE
)
