-- Add migration script here
ALTER TABLE feeds
ADD listed boolean NOT NULL DEFAULT false,
ADD hashtag VARCHAR NULL,
ADD content_warning VARCHAR NULL,
ADD status_publicity VARCHAR NULL;

