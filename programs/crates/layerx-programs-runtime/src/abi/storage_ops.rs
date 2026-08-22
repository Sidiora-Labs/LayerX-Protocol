//! Namespaced storage operations exposed by the ABI transaction.

use crate::meter::Meter;
use crate::storage::{metered_bytes, StorageNamespace};

use super::capability::CapabilityKey;
use super::{Abi, AbiError};

/// Frozen selector used by candidate namespace-aware storage operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum StorageSelector {
    Principal = 1,
    Shared = 2,
}

impl TryFrom<i32> for StorageSelector {
    type Error = AbiError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Principal),
            2 => Ok(Self::Shared),
            _ => Err(AbiError::InvalidEncoding),
        }
    }
}

impl Abi {
    /// Reads one value from the current program/principal namespace.
    ///
    /// # Errors
    ///
    /// Refuses missing authority, invalid keys, or meter exhaustion.
    pub fn storage_read(
        &mut self,
        meter: &mut Meter,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, AbiError> {
        self.storage_read_selected(meter, StorageSelector::Principal, key)
    }

    /// Reads from one host-fixed namespace selected by the candidate ABI.
    ///
    /// # Errors
    ///
    /// Refuses a missing scope-specific grant, invalid key, or exhausted meter.
    pub fn storage_read_selected(
        &mut self,
        meter: &mut Meter,
        selector: StorageSelector,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, AbiError> {
        let (capability, namespace) = self.storage_access(selector, false);
        self.authorization.capabilities().grant(&capability)?;
        let value = self.storage.read(namespace, key)?;
        meter.charge_storage_read(metered_bytes(key, value.as_deref())?)?;
        Ok(value)
    }

    /// Stages one value in the current program/principal namespace.
    ///
    /// # Errors
    ///
    /// Refuses missing authority, invalid bounds, or meter exhaustion.
    pub fn storage_write(
        &mut self,
        meter: &mut Meter,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), AbiError> {
        self.storage_write_selected(meter, StorageSelector::Principal, key, value)
    }

    /// Writes to one host-fixed namespace selected by the candidate ABI.
    ///
    /// # Errors
    ///
    /// Refuses a missing scope-specific grant, invalid bounds, or exhausted meter.
    pub fn storage_write_selected(
        &mut self,
        meter: &mut Meter,
        selector: StorageSelector,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), AbiError> {
        let (capability, namespace) = self.storage_access(selector, true);
        self.authorization.capabilities().grant(&capability)?;
        let bytes = metered_bytes(key, Some(value))?;
        meter.charge_storage_write(bytes)?;
        self.storage.write(namespace, key, value)?;
        Ok(())
    }

    /// Stages deletion in the current program/principal namespace.
    ///
    /// # Errors
    ///
    /// Refuses missing authority, invalid keys, or meter exhaustion.
    pub fn storage_delete(&mut self, meter: &mut Meter, key: &[u8]) -> Result<(), AbiError> {
        self.storage_delete_selected(meter, StorageSelector::Principal, key)
    }

    /// Deletes from one host-fixed namespace selected by the candidate ABI.
    ///
    /// # Errors
    ///
    /// Refuses a missing scope-specific grant, invalid key, or exhausted meter.
    pub fn storage_delete_selected(
        &mut self,
        meter: &mut Meter,
        selector: StorageSelector,
        key: &[u8],
    ) -> Result<(), AbiError> {
        let (capability, namespace) = self.storage_access(selector, true);
        self.authorization.capabilities().grant(&capability)?;
        meter.charge_storage_write(metered_bytes(key, None)?)?;
        self.storage.delete(namespace, key)?;
        Ok(())
    }

    fn storage_access(
        &self,
        selector: StorageSelector,
        write: bool,
    ) -> (CapabilityKey, StorageNamespace) {
        match (selector, write) {
            (StorageSelector::Principal, false) => {
                (CapabilityKey::StorageRead, self.principal_namespace())
            }
            (StorageSelector::Principal, true) => {
                (CapabilityKey::StorageWrite, self.principal_namespace())
            }
            (StorageSelector::Shared, false) => {
                (CapabilityKey::SharedStorageRead, self.shared_namespace())
            }
            (StorageSelector::Shared, true) => {
                (CapabilityKey::SharedStorageWrite, self.shared_namespace())
            }
        }
    }
}
