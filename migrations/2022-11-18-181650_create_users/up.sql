-- Your SQL goes here
CREATE TABLE users (
  id INT PRIMARY KEY,
  email VARCHAR NOT NULL,
  login_token VARCHAR NOT NULL,
  login_expires_at DATETIME NOT NULL,
  access_token VARCHAR,
  access_expires_at DATETIME
)
