-- Add migration script here
ALTER TABLE feeds
ADD COLUMN last_post_at TIMESTAMP WITH TIME ZONE;

update feeds set last_post_at = (SELECT MAX(created_at) from items where items.feed_id = feeds.id)
WHERE feeds.id IN (SELECT DISTINCT feed_id FROM items);
