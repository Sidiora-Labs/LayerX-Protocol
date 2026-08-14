PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projection_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    watermark INTEGER NOT NULL,
    stale INTEGER NOT NULL CHECK (stale IN (0, 1))
) STRICT;
INSERT OR IGNORE INTO projection_meta(singleton, watermark, stale)
VALUES (1, -1, 0);

CREATE TABLE IF NOT EXISTS balances (
    account_id BLOB NOT NULL CHECK (length(account_id) = 32),
    asset_id BLOB NOT NULL CHECK (length(asset_id) = 32),
    amount BLOB NOT NULL CHECK (length(amount) = 16),
    PRIMARY KEY (account_id, asset_id)
) STRICT, WITHOUT ROWID;
CREATE VIEW IF NOT EXISTS balance_view AS
SELECT account_id, asset_id, amount FROM balances;

CREATE TABLE IF NOT EXISTS receipts (
    activity_id BLOB PRIMARY KEY CHECK (length(activity_id) = 32),
    idempotency_key BLOB NOT NULL UNIQUE CHECK (length(idempotency_key) = 32),
    global_sequence INTEGER NOT NULL UNIQUE,
    result_code INTEGER NOT NULL,
    canonical_receipt BLOB NOT NULL
) STRICT, WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS receipts_by_idempotency
ON receipts(idempotency_key);

CREATE TABLE IF NOT EXISTS module_index (
    module_id INTEGER NOT NULL,
    secondary_key BLOB NOT NULL,
    activity_id BLOB NOT NULL CHECK (length(activity_id) = 32),
    PRIMARY KEY (module_id, secondary_key, activity_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS agent_queries (
    agent_id BLOB NOT NULL CHECK (length(agent_id) = 32),
    query_key TEXT NOT NULL,
    query_value BLOB NOT NULL,
    PRIMARY KEY (agent_id, query_key)
) STRICT, WITHOUT ROWID;
