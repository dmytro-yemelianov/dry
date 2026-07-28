CREATE TABLE accounts (id TEXT PRIMARY KEY, email TEXT UNIQUE NOT NULL, created_at TEXT DEFAULT (datetime('now')));
CREATE TABLE tokens (hash TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(id), kind TEXT NOT NULL CHECK (kind IN ('at','key')), label TEXT, created_at TEXT DEFAULT (datetime('now')), revoked INTEGER DEFAULT 0);
CREATE TABLE jobs (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, status TEXT NOT NULL, pack_id TEXT, pack_version TEXT, input_r2 TEXT, report_r2 TEXT, error TEXT, stage TEXT, created_at TEXT DEFAULT (datetime('now')), finished_at TEXT);
CREATE TABLE usage_events (id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, route TEXT NOT NULL, bytes INTEGER DEFAULT 0, at TEXT DEFAULT (datetime('now')));
CREATE INDEX usage_by_account_day ON usage_events (account_id, at);
