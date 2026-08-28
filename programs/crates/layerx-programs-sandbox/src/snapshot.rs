//! Canonical renter-authorized sandbox snapshots and prepared restoration.

use core::fmt::{self, Display};

use layerx_programs_runtime::{hash_bytes, HashAlgorithm, Meter, MeterRefusal, PrincipalId};

use crate::{EphemeralNamespace, Lease, LeaseId, LeaseState};

const SNAPSHOT_DOMAIN: &[u8] = b"LayerX/programs/sandbox/snapshot/v1\0";
pub const MAX_SANDBOX_LINEAR_MEMORY_BYTES: usize = 1 << 30;
pub const MAX_SANDBOX_GLOBALS: usize = 65_536;
pub const MAX_SANDBOX_OPERAND_STACK: usize = 65_536;
pub const MAX_SANDBOX_NAMESPACE_CELLS: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ContinuationValue {
    I32(u32) = 0,
    I64(u64) = 1,
    F32(u32) = 2,
    F64(u64) = 3,
}

impl ContinuationValue {
    fn encode(self, output: &mut Vec<u8>) {
        match self {
            Self::I32(value) => { output.push(0); output.extend_from_slice(&value.to_be_bytes()); }
            Self::I64(value) => { output.push(1); output.extend_from_slice(&value.to_be_bytes()); }
            Self::F32(bits) => { output.push(2); output.extend_from_slice(&bits.to_be_bytes()); }
            Self::F64(bits) => { output.push(3); output.extend_from_slice(&bits.to_be_bytes()); }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuationPoint {
    pub function_index: u32,
    pub instruction_offset: u64,
    pub operand_stack: Vec<ContinuationValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceCell {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxState {
    source_lease: LeaseId,
    source_namespace: EphemeralNamespace,
    linear_memory: Vec<u8>,
    globals: Vec<ContinuationValue>,
    continuation: ContinuationPoint,
    namespace_cells: Vec<NamespaceCell>,
}

impl SandboxState {
    pub fn new(
        lease: &Lease,
        linear_memory: Vec<u8>,
        globals: Vec<ContinuationValue>,
        continuation: ContinuationPoint,
        namespace_cells: Vec<NamespaceCell>,
    ) -> Result<Self, SnapshotRefusal> {
        let state = Self { source_lease: lease.id(), source_namespace: lease.namespace(),
            linear_memory, globals, continuation, namespace_cells };
        state.validate()?;
        Ok(state)
    }

    #[must_use] pub const fn source_lease(&self) -> LeaseId { self.source_lease }
    #[must_use] pub const fn source_namespace(&self) -> EphemeralNamespace { self.source_namespace }
    #[must_use] pub fn linear_memory(&self) -> &[u8] { &self.linear_memory }
    #[must_use] pub fn globals(&self) -> &[ContinuationValue] { &self.globals }
    #[must_use] pub const fn continuation(&self) -> &ContinuationPoint { &self.continuation }
    #[must_use] pub fn namespace_cells(&self) -> &[NamespaceCell] { &self.namespace_cells }

    fn validate(&self) -> Result<(), SnapshotRefusal> {
        if self.linear_memory.len() > MAX_SANDBOX_LINEAR_MEMORY_BYTES
            || self.globals.len() > MAX_SANDBOX_GLOBALS
            || self.continuation.operand_stack.len() > MAX_SANDBOX_OPERAND_STACK
            || self.namespace_cells.len() > MAX_SANDBOX_NAMESPACE_CELLS {
            return Err(SnapshotRefusal::StateBoundExceeded);
        }
        let mut prior: Option<&[u8]> = None;
        for cell in &self.namespace_cells {
            if cell.key.is_empty() || prior.is_some_and(|key| key >= cell.key.as_slice()) {
                return Err(SnapshotRefusal::NonCanonicalNamespace);
            }
            prior = Some(&cell.key);
        }
        Ok(())
    }

    fn validate_against(&self, lease: &Lease) -> Result<(), SnapshotRefusal> {
        self.validate()?;
        let memory_bytes = u64::try_from(self.linear_memory.len())
            .map_err(|_| SnapshotRefusal::StateBoundExceeded)?;
        let namespace_bytes = self.namespace_cells.iter().try_fold(0u64, |total, cell| {
            let key = u64::try_from(cell.key.len()).map_err(|_| SnapshotRefusal::StateBoundExceeded)?;
            let value = u64::try_from(cell.value.len()).map_err(|_| SnapshotRefusal::StateBoundExceeded)?;
            total.checked_add(key).and_then(|sum| sum.checked_add(value))
                .ok_or(SnapshotRefusal::StateBoundExceeded)
        })?;
        if memory_bytes > lease.limits().memory_bytes
            || namespace_bytes > lease.limits().namespace_bytes {
            return Err(SnapshotRefusal::TargetBoundExceeded);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SnapshotRefusal> {
        self.validate()?;
        let mut output = Vec::new();
        output.extend_from_slice(SNAPSHOT_DOMAIN);
        output.extend_from_slice(&self.source_lease.bytes());
        output.extend_from_slice(&self.source_namespace.host().bytes());
        output.extend_from_slice(&self.source_namespace.bytes());
        put_bytes(&mut output, &self.linear_memory)?;
        put_len(&mut output, self.globals.len())?;
        for value in &self.globals { value.encode(&mut output); }
        output.extend_from_slice(&self.continuation.function_index.to_be_bytes());
        output.extend_from_slice(&self.continuation.instruction_offset.to_be_bytes());
        put_len(&mut output, self.continuation.operand_stack.len())?;
        for value in &self.continuation.operand_stack { value.encode(&mut output); }
        put_len(&mut output, self.namespace_cells.len())?;
        for cell in &self.namespace_cells {
            put_bytes(&mut output, &cell.key)?;
            put_bytes(&mut output, &cell.value)?;
        }
        Ok(output)
    }

    pub fn digest(&self) -> Result<[u8; 32], SnapshotRefusal> {
        hash_bytes(HashAlgorithm::Sha256, &self.canonical_bytes()?)
            .map_err(|_| SnapshotRefusal::HashRefusal)
    }

    fn rebind(mut self, target: &Lease) -> Self {
        self.source_lease = target.id();
        self.source_namespace = target.namespace();
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    digest: [u8; 32],
    owner: PrincipalId,
    source_lease: LeaseId,
    source_namespace: EphemeralNamespace,
    host_program: [u8; 32],
    image_code_hash: [u8; 32],
    byte_length: u64,
}

impl Snapshot {
    pub fn commit(
        lease: &Lease, state: &SandboxState, renter: PrincipalId, meter: &mut Meter,
    ) -> Result<Self, SnapshotRefusal> {
        if lease.state() != LeaseState::Active { return Err(SnapshotRefusal::LeaseNotActive); }
        if renter != lease.tenant() { return Err(SnapshotRefusal::NotSnapshotOwner); }
        if state.source_lease != lease.id() || state.source_namespace != lease.namespace() {
            return Err(SnapshotRefusal::StateLeaseMismatch);
        }
        state.validate_against(lease)?;
        let bytes = state.canonical_bytes()?;
        let byte_length = u64::try_from(bytes.len()).map_err(|_| SnapshotRefusal::StateBoundExceeded)?;
        meter.charge_storage_write(byte_length).map_err(SnapshotRefusal::Meter)?;
        let digest = hash_bytes(HashAlgorithm::Sha256, &bytes)
            .map_err(|_| SnapshotRefusal::HashRefusal)?;
        Ok(Self { digest, owner: renter, source_lease: lease.id(),
            source_namespace: lease.namespace(), host_program: lease.host_program().bytes(),
            image_code_hash: lease.image_code_hash(), byte_length })
    }

    #[must_use] pub const fn digest(&self) -> [u8; 32] { self.digest }
    #[must_use] pub const fn owner(&self) -> PrincipalId { self.owner }
    #[must_use] pub const fn source_lease(&self) -> LeaseId { self.source_lease }
    #[must_use] pub const fn byte_length(&self) -> u64 { self.byte_length }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedRestore { target_lease: LeaseId, snapshot_digest: [u8; 32], state: SandboxState }

impl PreparedRestore {
    #[must_use] pub const fn target_lease(&self) -> LeaseId { self.target_lease }
    #[must_use] pub const fn snapshot_digest(&self) -> [u8; 32] { self.snapshot_digest }
    #[must_use] pub const fn state(&self) -> &SandboxState { &self.state }
    #[must_use] pub fn into_state(self) -> SandboxState { self.state }
}

pub fn restore(
    target: &Lease, snapshot: &Snapshot, supplied: SandboxState,
    renter: PrincipalId, meter: &mut Meter,
) -> Result<PreparedRestore, SnapshotRefusal> {
    if target.state() != LeaseState::Funded { return Err(SnapshotRefusal::TargetNotFunded); }
    if renter != snapshot.owner || renter != target.tenant() {
        return Err(SnapshotRefusal::NotSnapshotOwner);
    }
    if target.host_program().bytes() != snapshot.host_program
        || target.image_code_hash() != snapshot.image_code_hash {
        return Err(SnapshotRefusal::IncompatibleTarget);
    }
    if supplied.source_lease != snapshot.source_lease
        || supplied.source_namespace != snapshot.source_namespace {
        return Err(SnapshotRefusal::StateLeaseMismatch);
    }
    supplied.validate_against(target)?;
    let bytes = supplied.canonical_bytes()?;
    let byte_length = u64::try_from(bytes.len()).map_err(|_| SnapshotRefusal::StateBoundExceeded)?;
    if byte_length != snapshot.byte_length
        || hash_bytes(HashAlgorithm::Sha256, &bytes).map_err(|_| SnapshotRefusal::HashRefusal)? != snapshot.digest {
        return Err(SnapshotRefusal::DigestMismatch);
    }
    meter.charge_storage_write(byte_length).map_err(SnapshotRefusal::Meter)?;
    Ok(PreparedRestore { target_lease: target.id(), snapshot_digest: snapshot.digest,
        state: supplied.rebind(target) })
}

fn put_len(output: &mut Vec<u8>, length: usize) -> Result<(), SnapshotRefusal> {
    output.extend_from_slice(&u64::try_from(length).map_err(|_| SnapshotRefusal::StateBoundExceeded)?.to_be_bytes());
    Ok(())
}

fn put_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), SnapshotRefusal> {
    put_len(output, bytes.len())?;
    output.extend_from_slice(bytes);
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotRefusal {
    LeaseNotActive,
    TargetNotFunded,
    NotSnapshotOwner,
    StateLeaseMismatch,
    IncompatibleTarget,
    NonCanonicalNamespace,
    StateBoundExceeded,
    TargetBoundExceeded,
    DigestMismatch,
    HashRefusal,
    Meter(MeterRefusal),
}

impl Display for SnapshotRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result { write!(formatter, "{self:?}") }
}

impl std::error::Error for SnapshotRefusal {}
