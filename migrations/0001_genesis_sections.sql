CREATE TABLE genesis_import_sections (
    section_kind INTEGER PRIMARY KEY CHECK (section_kind BETWEEN 1 AND 11),
    item_count INTEGER NOT NULL CHECK (item_count > 0),
    canonical_digest BLOB NOT NULL CHECK (length(canonical_digest) = 32)
) STRICT;

CREATE TABLE genesis_import_asset_totals (
    section_kind INTEGER NOT NULL REFERENCES genesis_import_sections(section_kind),
    asset_id BLOB NOT NULL CHECK (length(asset_id) = 32),
    amount_hi INTEGER NOT NULL CHECK (amount_hi >= 0),
    amount_lo INTEGER NOT NULL CHECK (amount_lo >= 0),
    PRIMARY KEY (section_kind, asset_id)
) STRICT;

CREATE TABLE genesis_historical_commitments (
    commitment_id BLOB PRIMARY KEY CHECK (length(commitment_id) = 32),
    anchored_root BLOB NOT NULL CHECK (length(anchored_root) = 32),
    immutable INTEGER NOT NULL CHECK (immutable = 1),
    re_executable INTEGER NOT NULL CHECK (re_executable = 0)
) STRICT;
