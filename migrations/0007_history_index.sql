PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS history_index_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    log_end INTEGER NOT NULL,
    record_count INTEGER NOT NULL
) STRICT;
INSERT OR IGNORE INTO history_index_meta(singleton, log_end, record_count)
VALUES (1, 0, 0);

CREATE TABLE IF NOT EXISTS history_records (
    record_offset INTEGER PRIMARY KEY,
    record_kind INTEGER NOT NULL,
    global_sequence INTEGER NOT NULL,
    body_length INTEGER NOT NULL,
    batch_number INTEGER,
    checkpoint_id BLOB CHECK (
        checkpoint_id IS NULL OR length(checkpoint_id) = 32),
    activity_id BLOB CHECK (
        activity_id IS NULL OR length(activity_id) = 32),
    transaction_id BLOB CHECK (
        transaction_id IS NULL OR length(transaction_id) = 32),
    idempotency_key BLOB CHECK (
        idempotency_key IS NULL OR length(idempotency_key) = 32)
) STRICT;
CREATE INDEX IF NOT EXISTS history_by_sequence
ON history_records(global_sequence, record_offset);
CREATE INDEX IF NOT EXISTS history_by_batch
ON history_records(batch_number) WHERE batch_number IS NOT NULL;
CREATE INDEX IF NOT EXISTS history_by_checkpoint
ON history_records(checkpoint_id) WHERE checkpoint_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS history_by_activity
ON history_records(activity_id) WHERE activity_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS history_by_transaction
ON history_records(transaction_id) WHERE transaction_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS history_by_idempotency
ON history_records(idempotency_key) WHERE idempotency_key IS NOT NULL;
