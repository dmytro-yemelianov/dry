CREATE TABLE licenses (id TEXT PRIMARY KEY, email TEXT NOT NULL, licensee TEXT NOT NULL, tier TEXT NOT NULL, expires_unix INTEGER NOT NULL, order_id TEXT, revoked INTEGER DEFAULT 0, created_at TEXT DEFAULT (datetime('now')));
CREATE UNIQUE INDEX idx_licenses_order_id ON licenses(order_id) WHERE order_id IS NOT NULL;
