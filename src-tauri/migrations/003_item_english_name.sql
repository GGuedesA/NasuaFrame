ALTER TABLE items ADD COLUMN name_en TEXT NOT NULL DEFAULT '';
UPDATE items SET name_en = name;
