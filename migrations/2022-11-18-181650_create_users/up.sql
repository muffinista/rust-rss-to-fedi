-- Your SQL goes here
CREATE TABLE users (
  id INT PRIMARY KEY NOT NULL,
  email VARCHAR NOT NULL,
  login_token VARCHAR NOT NULL,
  access_token VARCHAR
);

--   login_token_expires_at DATETIME
--   access_expires_at DATETIME

