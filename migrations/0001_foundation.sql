-- Initial durable simulation metadata and append-only event journal.
-- Applied transactionally by society-storage in a later milestone.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_migrations (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS world_metadata (
    singleton       INTEGER PRIMARY KEY CHECK (singleton = 1),
    rules_version   INTEGER NOT NULL,
    world_version   INTEGER NOT NULL CHECK (world_version >= 0),
    virtual_tick    INTEGER NOT NULL CHECK (virtual_tick >= 0),
    seed            INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS event_log (
    sequence        INTEGER PRIMARY KEY AUTOINCREMENT,
    virtual_tick    INTEGER NOT NULL CHECK (virtual_tick >= 0),
    world_version   INTEGER NOT NULL CHECK (world_version > 0),
    event_type      TEXT NOT NULL,
    payload_json    TEXT NOT NULL,
    event_hash      TEXT NOT NULL,
    UNIQUE (world_version)
);

CREATE INDEX IF NOT EXISTS event_log_virtual_tick_idx ON event_log (virtual_tick, sequence);

CREATE TABLE IF NOT EXISTS world_snapshots (
    sequence        INTEGER PRIMARY KEY AUTOINCREMENT,
    format_version  INTEGER NOT NULL,
    virtual_tick    INTEGER NOT NULL CHECK (virtual_tick >= 0),
    world_version   INTEGER NOT NULL CHECK (world_version >= 0),
    payload_json    TEXT NOT NULL
);

INSERT OR IGNORE INTO schema_migrations (version) VALUES (1);
