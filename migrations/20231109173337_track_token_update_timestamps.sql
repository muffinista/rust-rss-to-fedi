-- Add migration script here
ALTER TABLE users 
ADD COLUMN login_token_updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
ADD COLUMN access_token_updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW();

