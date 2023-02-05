-- Add migration script here
CREATE TABLE blocked_domains (
  name VARCHAR PRIMARY KEY,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);
