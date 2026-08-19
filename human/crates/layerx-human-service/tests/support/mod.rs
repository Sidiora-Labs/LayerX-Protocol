use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use layerx_human_service::store::{
    AgentTenantId, PrincipalId, PrincipalStore, RetentionPeriod, RetentionPolicy, RowKey,
    TenancyDigest, TenancyMap,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

pub fn directory(label: &str) -> PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "layerx-human-store-{label}-{}-{sequence}",
        std::process::id()
    ))
}

pub fn principal(name: &str) -> PrincipalId {
    PrincipalId::new(name).unwrap_or_else(|error| panic!("principal: {error}"))
}

pub fn row_key(name: &str) -> RowKey {
    RowKey::new(name).unwrap_or_else(|error| panic!("row key: {error}"))
}

pub fn tenancy(pairs: &[(&str, &str)]) -> TenancyMap {
    let entries = pairs.iter().map(|(name, tenant)| {
        (
            principal(name),
            AgentTenantId::new(*tenant).unwrap_or_else(|error| panic!("tenant: {error}")),
        )
    });
    TenancyMap::new(entries).unwrap_or_else(|error| panic!("tenancy map: {error}"))
}

pub fn retention_uniform(units: u64) -> RetentionPolicy {
    RetentionPolicy {
        journeys: RetentionPeriod::new(units),
        notifications: RetentionPeriod::new(units),
        audit: RetentionPeriod::new(units),
        telemetry: RetentionPeriod::new(units),
        cache: RetentionPeriod::new(units),
    }
}

pub fn install_and_open(
    root: &Path,
    map: &TenancyMap,
    retention: RetentionPolicy,
) -> (PrincipalStore, TenancyDigest) {
    let digest = map
        .install(root)
        .unwrap_or_else(|error| panic!("install tenancy: {error}"));
    let store = PrincipalStore::open(root, retention, digest)
        .unwrap_or_else(|error| panic!("open store: {error}"));
    (store, digest)
}

pub fn version_file_bytes(version: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8);
    bytes.extend_from_slice(b"LXHV");
    bytes.extend_from_slice(&version.to_be_bytes());
    bytes
}
