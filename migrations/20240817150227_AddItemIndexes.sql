-- Add migration script here
-- web-1  | [2024-08-17T14:57:02Z INFO  sqlx::query] slow statement: execution time exceeded alert threshold summary="SELECT * FROM items …" db.statement="\n\nSELECT\n  *\nFROM\n  items\nWHERE\n  feed_id = $1\nORDER by\n  created_at DESC,\n  id ASC\nLIMIT\n  $2\n" rows_affected=10 rows_returned=10 elapsed=372.38138ms elapsed_secs=0.37238138 slow_threshold=200ms


CREATE INDEX items_for_feed ON items (feed_id, created_at, id);
