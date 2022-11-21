CREATE TABLE email_verification_tokens (
    id INT PRIMARY KEY,
    token VARCHAR NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);