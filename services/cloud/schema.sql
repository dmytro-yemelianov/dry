CREATE TABLE accounts (id TEXT PRIMARY KEY, email TEXT UNIQUE NOT NULL, created_at TEXT DEFAULT (datetime('now')));
CREATE TABLE tokens (hash TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(id), kind TEXT NOT NULL CHECK (kind IN ('at','key')), label TEXT, created_at TEXT DEFAULT (datetime('now')), revoked INTEGER DEFAULT 0);
CREATE TABLE jobs (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, status TEXT NOT NULL, pack_id TEXT, pack_version TEXT, profile_id TEXT, input_r2 TEXT, report_r2 TEXT, error TEXT, stage TEXT, created_at TEXT DEFAULT (datetime('now')), finished_at TEXT);
CREATE TABLE usage_events (id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, route TEXT NOT NULL, bytes INTEGER DEFAULT 0, at TEXT DEFAULT (datetime('now')));
CREATE INDEX usage_by_account_day ON usage_events (account_id, at);
CREATE TABLE grants (device_code TEXT PRIMARY KEY, granted_at TEXT DEFAULT (datetime('now')));
CREATE TABLE machine_catalog (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    vendor TEXT NOT NULL,
    category TEXT NOT NULL,          -- '3d_printer', 'cnc_mill', 'laser_cutter', 'plasma_waterjet', 'robot_arm'
    firmware_flavor TEXT NOT NULL,   -- 'marlin', 'klipper', 'reprap', 'rs274', 'grbl', 'krl'
    kinematics_type TEXT NOT NULL,   -- 'cartesian', 'corexy', 'delta', 'five_axis', 'robot_6dof'
    bounds_min_x REAL NOT NULL,
    bounds_max_x REAL NOT NULL,
    bounds_min_y REAL NOT NULL,
    bounds_max_y REAL NOT NULL,
    bounds_min_z REAL NOT NULL,
    bounds_max_z REAL NOT NULL,
    max_feedrate REAL NOT NULL,
    max_accel REAL,
    is_official INTEGER DEFAULT 1,
    profile_json TEXT NOT NULL,
    sha256_hash TEXT NOT NULL,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_machines_filter ON machine_catalog(vendor, category, firmware_flavor);

