CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- replaced as a whole on every catalog refresh
CREATE TABLE items (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    max_rank INTEGER,
    subtypes TEXT NOT NULL
);

-- no foreign key to items, the catalog gets wiped on refresh
CREATE TABLE filters (
    id INTEGER PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 1,
    item_id TEXT NOT NULL,
    intent TEXT NOT NULL,
    price_limit INTEGER NOT NULL,
    platform TEXT NOT NULL,
    crossplay INTEGER NOT NULL,
    rank_min INTEGER,
    rank_max INTEGER,
    subtype TEXT,
    min_status TEXT NOT NULL,
    cooldown_secs INTEGER NOT NULL DEFAULT 60
);
