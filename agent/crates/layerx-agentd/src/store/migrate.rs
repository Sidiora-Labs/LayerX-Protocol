//! Forward-only store schema migrations.

use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

const MAGIC: &[u8; 4] = b"LXAS";
const STORE_FILE: &str = "store.bin";
const MIGRATION_TEMP_FILE: &str = "store.bin.migrating";

/// Current durable store schema.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;
const OLDEST_SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Migration refusal and I/O failures.
#[derive(Debug)]
pub enum MigrationError {
    Io(io::Error),
    InvalidHeader,
    NewerSchema { found: u32, supported: u32 },
    UnsupportedOldSchema(u32),
    CorruptV1,
    SizeOverflow,
}

impl Display for MigrationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "migration I/O failure: {error}"),
            Self::InvalidHeader => formatter.write_str("invalid store header"),
            Self::NewerSchema { found, supported } => write!(
                formatter,
                "store schema {found} is newer than supported schema {supported}"
            ),
            Self::UnsupportedOldSchema(version) => {
                write!(formatter, "store schema {version} is no longer supported")
            }
            Self::CorruptV1 => formatter.write_str("corrupt schema version 1 store"),
            Self::SizeOverflow => formatter.write_str("migrated store exceeds encoding bounds"),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<io::Error> for MigrationError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// Migrates a store forward without deriving any protocol fact locally.
pub(super) fn migrate_store(root: &Path) -> Result<(), MigrationError> {
    let path = root.join(STORE_FILE);
    if !path.exists() {
        return Ok(());
    }
    let bytes = fs::read(&path)?;
    let version = schema_version(&bytes)?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::NewerSchema {
            found: version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    if version < OLDEST_SUPPORTED_SCHEMA_VERSION {
        return Err(MigrationError::UnsupportedOldSchema(version));
    }
    if version == CURRENT_SCHEMA_VERSION {
        return Ok(());
    }

    let migrated = match version {
        1 => migrate_v1_to_v2(&bytes)?,
        _ => return Err(MigrationError::UnsupportedOldSchema(version)),
    };
    let temp_path = root.join(MIGRATION_TEMP_FILE);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp_path)?;
    file.write_all(&migrated)?;
    file.sync_all()?;
    fs::rename(temp_path, path)?;
    File::open(root)?.sync_all()?;
    Ok(())
}

fn schema_version(bytes: &[u8]) -> Result<u32, MigrationError> {
    if bytes.get(..4) != Some(MAGIC) {
        return Err(MigrationError::InvalidHeader);
    }
    let encoded = bytes.get(4..8).ok_or(MigrationError::InvalidHeader)?;
    let mut value = [0_u8; 4];
    value.copy_from_slice(encoded);
    Ok(u32::from_be_bytes(value))
}

// V1 entries were tenant, kind, object id and value. V2 inserts the explicit
// authority class after kind. Existing data is local-only unless it is a
// core-produced identity, budget mirror, or receipt.
fn migrate_v1_to_v2(bytes: &[u8]) -> Result<Vec<u8>, MigrationError> {
    let mut offset = 8_usize;
    let count = read_u32(bytes, &mut offset)?;
    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&CURRENT_SCHEMA_VERSION.to_be_bytes());
    output.extend_from_slice(&count.to_be_bytes());
    for _ in 0..count {
        copy_sized(bytes, &mut offset, &mut output)?;
        let kind = *bytes.get(offset).ok_or(MigrationError::CorruptV1)?;
        offset = offset.checked_add(1).ok_or(MigrationError::SizeOverflow)?;
        output.push(kind);
        output.push(if matches!(kind, 1 | 4 | 8) { 2 } else { 1 });
        copy_sized(bytes, &mut offset, &mut output)?;
        copy_sized(bytes, &mut offset, &mut output)?;
    }
    if offset != bytes.len() {
        return Err(MigrationError::CorruptV1);
    }
    Ok(output)
}

fn copy_sized(
    input: &[u8],
    offset: &mut usize,
    output: &mut Vec<u8>,
) -> Result<(), MigrationError> {
    let length = read_u32(input, offset)?;
    output.extend_from_slice(&length.to_be_bytes());
    let length = usize::try_from(length).map_err(|_| MigrationError::SizeOverflow)?;
    let end = offset
        .checked_add(length)
        .ok_or(MigrationError::SizeOverflow)?;
    let value = input.get(*offset..end).ok_or(MigrationError::CorruptV1)?;
    output.extend_from_slice(value);
    *offset = end;
    Ok(())
}

fn read_u32(input: &[u8], offset: &mut usize) -> Result<u32, MigrationError> {
    let end = offset.checked_add(4).ok_or(MigrationError::SizeOverflow)?;
    let encoded = input.get(*offset..end).ok_or(MigrationError::CorruptV1)?;
    let mut value = [0_u8; 4];
    value.copy_from_slice(encoded);
    *offset = end;
    Ok(u32::from_be_bytes(value))
}
