//! Durable tenant-scoped local storage.

#[path = "migrate.rs"]
mod migration;

pub use migration::{MigrationError, CURRENT_SCHEMA_VERSION};

use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 4] = b"LXAS";
const STORE_FILE: &str = "store.bin";
const TEMP_FILE: &str = "store.bin.tmp";

/// Migrates a store through every supported forward-only schema step.
pub fn migrate(root: &Path) -> Result<(), MigrationError> {
    migration::migrate_store(root)
}

/// A validated tenant identifier that must be present in every storage key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TenantId(String);

impl TenantId {
    /// Creates a non-empty bounded tenant identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, StoreError> {
        let value = value.into();
        if value.is_empty() || value.len() > 255 || value.as_bytes().contains(&0) {
            return Err(StoreError::InvalidTenant);
        }
        Ok(Self(value))
    }

    /// Returns the tenant identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Every durable object category owned by the daemon.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ObjectKind {
    Identity = 1,
    Session = 2,
    Capability = 3,
    Budget = 4,
    Policy = 5,
    PreparedActivity = 6,
    Outbox = 7,
    Receipt = 8,
    Subscription = 9,
    Audit = 10,
    Idempotency = 11,
    Configuration = 12,
}

impl ObjectKind {
    fn from_byte(value: u8) -> Result<Self, StoreError> {
        match value {
            1 => Ok(Self::Identity),
            2 => Ok(Self::Session),
            3 => Ok(Self::Capability),
            4 => Ok(Self::Budget),
            5 => Ok(Self::Policy),
            6 => Ok(Self::PreparedActivity),
            7 => Ok(Self::Outbox),
            8 => Ok(Self::Receipt),
            9 => Ok(Self::Subscription),
            10 => Ok(Self::Audit),
            11 => Ok(Self::Idempotency),
            12 => Ok(Self::Configuration),
            _ => Err(StoreError::Corrupt("unknown object kind")),
        }
    }
}

/// A key that cannot be constructed without a tenant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TenantKey {
    tenant: TenantId,
    kind: ObjectKind,
    object_id: Vec<u8>,
}

impl TenantKey {
    /// Creates a tenant-scoped key.
    pub fn new(
        tenant: TenantId,
        kind: ObjectKind,
        object_id: impl Into<Vec<u8>>,
    ) -> Result<Self, StoreError> {
        let object_id = object_id.into();
        if object_id.is_empty() || object_id.len() > 4096 {
            return Err(StoreError::InvalidObjectId);
        }
        Ok(Self {
            tenant,
            kind,
            object_id,
        })
    }

    /// Returns the tenant owning this key.
    #[must_use]
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }

    /// Returns the object category.
    #[must_use]
    pub const fn kind(&self) -> ObjectKind {
        self.kind
    }

    /// Returns the opaque object identifier.
    #[must_use]
    pub fn object_id(&self) -> &[u8] {
        &self.object_id
    }
}

/// Declares whether bytes are a daemon-local artefact or a cache of core output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StorageClass {
    LocalOnly = 1,
    CoreProducedCache = 2,
}

impl StorageClass {
    fn from_byte(value: u8) -> Result<Self, StoreError> {
        match value {
            1 => Ok(Self::LocalOnly),
            2 => Ok(Self::CoreProducedCache),
            _ => Err(StoreError::Corrupt("unknown storage class")),
        }
    }
}

/// Stored bytes with their authority classification inseparably attached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredValue {
    class: StorageClass,
    bytes: Vec<u8>,
}

impl StoredValue {
    /// Returns whether these bytes are local or a cache of core output.
    #[must_use]
    pub const fn class(&self) -> StorageClass {
        self.class
    }

    /// Returns the exact stored bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Errors from the durable store.
#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Migration(MigrationError),
    InvalidTenant,
    InvalidObjectId,
    Corrupt(&'static str),
    SizeOverflow,
}

impl Display for StoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "store I/O failure: {error}"),
            Self::Migration(error) => write!(formatter, "store migration failure: {error}"),
            Self::InvalidTenant => formatter.write_str("invalid tenant identifier"),
            Self::InvalidObjectId => formatter.write_str("invalid object identifier"),
            Self::Corrupt(reason) => write!(formatter, "corrupt store: {reason}"),
            Self::SizeOverflow => formatter.write_str("store value exceeds encoding bounds"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<MigrationError> for StoreError {
    fn from(value: MigrationError) -> Self {
        Self::Migration(value)
    }
}

/// File-backed daemon store. All access requires a [`TenantKey`].
#[derive(Debug)]
pub struct Store {
    root: PathBuf,
    entries: BTreeMap<TenantKey, StoredValue>,
}

impl Store {
    /// Opens a store, migrating supported older schemas before decoding it.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, StoreError> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        migrate(&root)?;
        let path = root.join(STORE_FILE);
        let entries = if path.exists() {
            decode(&fs::read(path)?)?
        } else {
            BTreeMap::new()
        };
        Ok(Self { root, entries })
    }

    /// Reads an object. There is deliberately no unscoped read API.
    #[must_use]
    pub fn get(&self, key: &TenantKey) -> Option<&StoredValue> {
        self.entries.get(key)
    }

    /// Lists tenant-scoped object identifiers in deterministic key order.
    #[must_use]
    pub fn list_object_ids(&self, tenant: &TenantId, kind: ObjectKind) -> Vec<Vec<u8>> {
        self.entries
            .keys()
            .filter(|key| key.tenant == *tenant && key.kind == kind)
            .map(|key| key.object_id.clone())
            .collect()
    }

    /// Persists daemon-local bytes.
    pub fn put_local(&mut self, key: TenantKey, bytes: Vec<u8>) -> Result<(), StoreError> {
        self.put(key, StorageClass::LocalOnly, bytes)
    }

    /// Persists exact bytes produced by the core as a rebuildable cache.
    pub fn put_core_cache(&mut self, key: TenantKey, bytes: Vec<u8>) -> Result<(), StoreError> {
        self.put(key, StorageClass::CoreProducedCache, bytes)
    }

    /// Removes one daemon-local object with rollback on persistence failure.
    pub fn remove_local(&mut self, key: &TenantKey) -> Result<bool, StoreError> {
        let Some(previous) = self.entries.remove(key) else {
            return Ok(false);
        };
        if previous.class != StorageClass::LocalOnly {
            self.entries.insert(key.clone(), previous);
            return Err(StoreError::Corrupt(
                "attempted to remove core-produced cache",
            ));
        }
        if let Err(error) = self.persist() {
            self.entries.insert(key.clone(), previous);
            return Err(error);
        }
        Ok(true)
    }

    /// Atomically records the three local records required before transmission.
    pub fn record_submission(
        &mut self,
        tenant: TenantId,
        idempotency_key: Vec<u8>,
        signed_bytes: Vec<u8>,
        outbox_state: Vec<u8>,
    ) -> Result<(), StoreError> {
        let prepared_key = TenantKey::new(
            tenant.clone(),
            ObjectKind::PreparedActivity,
            idempotency_key.clone(),
        )?;
        let outbox_key =
            TenantKey::new(tenant.clone(), ObjectKind::Outbox, idempotency_key.clone())?;
        let idempotency_record = TenantKey::new(tenant, ObjectKind::Idempotency, idempotency_key)?;

        let before = self.entries.clone();
        self.entries.insert(
            prepared_key,
            StoredValue {
                class: StorageClass::LocalOnly,
                bytes: signed_bytes,
            },
        );
        self.entries.insert(
            outbox_key,
            StoredValue {
                class: StorageClass::LocalOnly,
                bytes: outbox_state,
            },
        );
        self.entries.insert(
            idempotency_record,
            StoredValue {
                class: StorageClass::LocalOnly,
                bytes: b"pending".to_vec(),
            },
        );
        if let Err(error) = self.persist() {
            self.entries = before;
            return Err(error);
        }
        Ok(())
    }

    /// Restores cache entries obtained again from the core after local deletion.
    pub fn restore_core_cache<I>(&mut self, entries: I) -> Result<(), StoreError>
    where
        I: IntoIterator<Item = (TenantKey, Vec<u8>)>,
    {
        let before = self.entries.clone();
        for (key, bytes) in entries {
            self.entries.insert(
                key,
                StoredValue {
                    class: StorageClass::CoreProducedCache,
                    bytes,
                },
            );
        }
        if let Err(error) = self.persist() {
            self.entries = before;
            return Err(error);
        }
        Ok(())
    }

    /// Atomically records byte-identical core receipt indexes and local
    /// verification metadata, rejecting any conflicting existing index.
    pub fn record_verified_receipt(
        &mut self,
        receipt_keys: &[TenantKey],
        receipt_bytes: &[u8],
        metadata_key: TenantKey,
        metadata: Vec<u8>,
    ) -> Result<(), StoreError> {
        if receipt_keys.is_empty()
            || receipt_keys
                .iter()
                .any(|key| key.kind != ObjectKind::Receipt)
            || metadata_key.kind != ObjectKind::Configuration
        {
            return Err(StoreError::Corrupt("invalid verified receipt record"));
        }
        for key in receipt_keys {
            if self
                .entries
                .get(key)
                .is_some_and(|value| value.bytes != receipt_bytes)
            {
                return Err(StoreError::Corrupt("conflicting receipt index"));
            }
        }
        if self
            .entries
            .get(&metadata_key)
            .is_some_and(|value| value.bytes != metadata)
        {
            return Err(StoreError::Corrupt("conflicting receipt metadata"));
        }

        let before = self.entries.clone();
        for key in receipt_keys {
            self.entries.insert(
                key.clone(),
                StoredValue {
                    class: StorageClass::CoreProducedCache,
                    bytes: receipt_bytes.to_vec(),
                },
            );
        }
        self.entries.insert(
            metadata_key,
            StoredValue {
                class: StorageClass::LocalOnly,
                bytes: metadata,
            },
        );
        if let Err(error) = self.persist() {
            self.entries = before;
            return Err(error);
        }
        Ok(())
    }

    fn put(
        &mut self,
        key: TenantKey,
        class: StorageClass,
        bytes: Vec<u8>,
    ) -> Result<(), StoreError> {
        let previous = self
            .entries
            .insert(key.clone(), StoredValue { class, bytes });
        if let Err(error) = self.persist() {
            match previous {
                Some(value) => {
                    self.entries.insert(key, value);
                }
                None => {
                    self.entries.remove(&key);
                }
            }
            return Err(error);
        }
        Ok(())
    }

    fn persist(&self) -> Result<(), StoreError> {
        let bytes = encode(&self.entries)?;
        let temp_path = self.root.join(TEMP_FILE);
        let final_path = self.root.join(STORE_FILE);
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(temp_path, final_path)?;
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }
}

fn encode(entries: &BTreeMap<TenantKey, StoredValue>) -> Result<Vec<u8>, StoreError> {
    let mut output = Vec::new();
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&CURRENT_SCHEMA_VERSION.to_be_bytes());
    push_len(&mut output, entries.len())?;
    for (key, value) in entries {
        push_bytes(&mut output, key.tenant.as_str().as_bytes())?;
        output.push(key.kind as u8);
        output.push(value.class as u8);
        push_bytes(&mut output, &key.object_id)?;
        push_bytes(&mut output, &value.bytes)?;
    }
    Ok(output)
}

fn decode(bytes: &[u8]) -> Result<BTreeMap<TenantKey, StoredValue>, StoreError> {
    let mut decoder = Decoder::new(bytes);
    if decoder.take(4)? != MAGIC {
        return Err(StoreError::Corrupt("invalid magic"));
    }
    if decoder.u32()? != CURRENT_SCHEMA_VERSION {
        return Err(StoreError::Corrupt("store was not migrated"));
    }
    let count = decoder.len()?;
    let mut entries = BTreeMap::new();
    for _ in 0..count {
        let tenant_bytes = decoder.bytes()?;
        let tenant_text = std::str::from_utf8(tenant_bytes)
            .map_err(|_| StoreError::Corrupt("tenant is not UTF-8"))?;
        let tenant = TenantId::new(tenant_text.to_owned())?;
        let kind = ObjectKind::from_byte(decoder.byte()?)?;
        let class = StorageClass::from_byte(decoder.byte()?)?;
        let object_id = decoder.bytes()?.to_vec();
        let value = decoder.bytes()?.to_vec();
        let key = TenantKey::new(tenant, kind, object_id)?;
        if entries
            .insert(
                key,
                StoredValue {
                    class,
                    bytes: value,
                },
            )
            .is_some()
        {
            return Err(StoreError::Corrupt("duplicate key"));
        }
    }
    if !decoder.is_empty() {
        return Err(StoreError::Corrupt("trailing bytes"));
    }
    Ok(entries)
}

fn push_len(output: &mut Vec<u8>, value: usize) -> Result<(), StoreError> {
    let value = u32::try_from(value).map_err(|_| StoreError::SizeOverflow)?;
    output.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

fn push_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), StoreError> {
    push_len(output, bytes.len())?;
    output.extend_from_slice(bytes);
    Ok(())
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], StoreError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(StoreError::Corrupt("length overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(StoreError::Corrupt("truncated value"))?;
        self.offset = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, StoreError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, StoreError> {
        let mut encoded = [0_u8; 4];
        encoded.copy_from_slice(self.take(4)?);
        Ok(u32::from_be_bytes(encoded))
    }

    fn len(&mut self) -> Result<usize, StoreError> {
        usize::try_from(self.u32()?).map_err(|_| StoreError::SizeOverflow)
    }

    fn bytes(&mut self) -> Result<&'a [u8], StoreError> {
        let length = self.len()?;
        self.take(length)
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
