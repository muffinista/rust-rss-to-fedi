delete from actors where  username is null;
ALTER TABLE actors ALTER COLUMN username SET NOT NULL;

