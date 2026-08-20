//! Version-one capability ABI. Every operation checks an explicit grant from
//! the invoking activity before touching namespaced storage or producing an
//! effect for the kernel to apply.

use core::fmt::{self, Display};
use std::collections::BTreeMap;

use crate::execute::ABI_VERSION;
use crate::meter::{Meter, MeterRefusal};
use crate::storage::{
    metered_bytes, PrincipalId, ProgramId, Storage, StorageError, StorageNamespace,
};

pub const ABI_MODULE: &str = "layerx_v1";
pub const MAX_EVENT_TOPIC_BYTES: usize = 64;
pub const MAX_EVENT_DATA_BYTES: usize = 65_536;
pub const MAX_CALL_INPUT_BYTES: usize = 1_048_576;
pub const MAX_CAPABILITIES: usize = 256;

/// Frozen version-one host-function surface. Signatures use WebAssembly value
/// names, and all values are integer-only.
pub const ABI_MANIFEST: &str = "layerx_v1\0storage_read(i32,i32,i32,i32)->i32\0storage_write(i32,i32,i32,i32)->i32\0storage_delete(i32,i32)->i32\0event_emit(i32,i32,i32,i32)->i32\0program_call(i32,i32,i32,i32,i32,i32)->i32\0transfer_402(i64,i64,i32,i32,i32,i32)->i32\0receipt_read(i32,i32,i32,i32)->i32\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostFunction {
    pub name: &'static str,
    pub signature: &'static str,
}

pub const HOST_FUNCTIONS: [HostFunction; 7] = [
    HostFunction {
        name: "storage_read",
        signature: "(i32,i32,i32,i32)->i32",
    },
    HostFunction {
        name: "storage_write",
        signature: "(i32,i32,i32,i32)->i32",
    },
    HostFunction {
        name: "storage_delete",
        signature: "(i32,i32)->i32",
    },
    HostFunction {
        name: "event_emit",
        signature: "(i32,i32,i32,i32)->i32",
    },
    HostFunction {
        name: "program_call",
        signature: "(i32,i32,i32,i32,i32,i32)->i32",
    },
    HostFunction {
        name: "transfer_402",
        signature: "(i64,i64,i32,i32,i32,i32)->i32",
    },
    HostFunction {
        name: "receipt_read",
        signature: "(i32,i32,i32,i32)->i32",
    },
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CapabilityKey {
    StorageRead,
    StorageWrite,
    EmitEvent,
    Call(ProgramId),
    Transfer { asset: [u8; 32], to: [u8; 32] },
    ReceiptRead([u8; 32]),
}

/// One explicit authority granted by the invoking activity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Capability {
    StorageRead,
    StorageWrite,
    EmitEvent,
    Call {
        program: ProgramId,
    },
    Transfer402 {
        asset: [u8; 32],
        to: [u8; 32],
        maximum_amount: u128,
    },
    ReceiptRead {
        receipt_digest: [u8; 32],
    },
}

impl Capability {
    fn key(&self) -> CapabilityKey {
        match self {
            Self::StorageRead => CapabilityKey::StorageRead,
            Self::StorageWrite => CapabilityKey::StorageWrite,
            Self::EmitEvent => CapabilityKey::EmitEvent,
            Self::Call { program } => CapabilityKey::Call(*program),
            Self::Transfer402 { asset, to, .. } => CapabilityKey::Transfer {
                asset: *asset,
                to: *to,
            },
            Self::ReceiptRead { receipt_digest } => CapabilityKey::ReceiptRead(*receipt_digest),
        }
    }

    fn valid(&self) -> bool {
        match self {
            Self::Transfer402 {
                asset,
                to,
                maximum_amount,
            } => asset != &[0; 32] && to != &[0; 32] && *maximum_amount != 0,
            Self::ReceiptRead { receipt_digest } => receipt_digest != &[0; 32],
            Self::StorageRead | Self::StorageWrite | Self::EmitEvent | Self::Call { .. } => true,
        }
    }
}

/// Closed set of explicit capabilities. Duplicate authority keys are refused,
/// preventing ambiguous limits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CapabilitySet(BTreeMap<CapabilityKey, Capability>);

impl CapabilitySet {
    /// Constructs a validated capability set.
    ///
    /// # Errors
    ///
    /// Refuses invalid or duplicate grants.
    pub fn new(grants: impl IntoIterator<Item = Capability>) -> Result<Self, AbiError> {
        let mut capabilities = BTreeMap::new();
        for grant in grants {
            if capabilities.len() == MAX_CAPABILITIES {
                return Err(AbiError::InvalidCapability);
            }
            if !grant.valid() {
                return Err(AbiError::InvalidCapability);
            }
            if capabilities.insert(grant.key(), grant).is_some() {
                return Err(AbiError::DuplicateCapability);
            }
        }
        Ok(Self(capabilities))
    }

    /// Encodes this set into the frozen deterministic capability-list format
    /// consumed by `program_call`.
    #[must_use]
    pub fn canonical_encoding(&self) -> Vec<u8> {
        let count = u16::try_from(self.0.len()).unwrap_or(u16::MAX);
        let mut encoded = Vec::with_capacity(2 + self.0.len().saturating_mul(81));
        encoded.extend_from_slice(&count.to_be_bytes());
        for capability in self.0.values() {
            match capability {
                Capability::StorageRead => encoded.push(1),
                Capability::StorageWrite => encoded.push(2),
                Capability::EmitEvent => encoded.push(3),
                Capability::Call { program } => {
                    encoded.push(4);
                    encoded.extend_from_slice(&program.bytes());
                }
                Capability::Transfer402 {
                    asset,
                    to,
                    maximum_amount,
                } => {
                    encoded.push(5);
                    encoded.extend_from_slice(asset);
                    encoded.extend_from_slice(to);
                    encoded.extend_from_slice(&maximum_amount.to_be_bytes());
                }
                Capability::ReceiptRead { receipt_digest } => {
                    encoded.push(6);
                    encoded.extend_from_slice(receipt_digest);
                }
            }
        }
        encoded
    }

    /// Returns an empty ambient-authority-free set.
    #[must_use]
    pub const fn empty() -> Self {
        Self(BTreeMap::new())
    }

    /// Narrows this authority to an explicitly requested subset.
    ///
    /// # Errors
    ///
    /// Refuses every missing grant or increased transfer limit.
    pub fn narrow(
        &self,
        requested: impl IntoIterator<Item = Capability>,
    ) -> Result<Self, AbiError> {
        let narrowed = Self::new(requested)?;
        for (key, request) in &narrowed.0 {
            let parent = self.0.get(key).ok_or(AbiError::CapabilityDenied)?;
            if let (
                Capability::Transfer402 {
                    maximum_amount: requested,
                    ..
                },
                Capability::Transfer402 {
                    maximum_amount: granted,
                    ..
                },
            ) = (request, parent)
            {
                if requested > granted {
                    return Err(AbiError::CapabilityEscalation);
                }
            }
        }
        Ok(narrowed)
    }

    fn grant(&self, key: &CapabilityKey) -> Result<&Capability, AbiError> {
        self.0.get(key).ok_or(AbiError::CapabilityDenied)
    }

    pub(crate) fn permits_transfer(&self, asset: [u8; 32], to: [u8; 32], amount: u128) -> bool {
        matches!(
            self.0.get(&CapabilityKey::Transfer { asset, to }),
            Some(Capability::Transfer402 { maximum_amount, .. }) if amount <= *maximum_amount
        )
    }

    pub(crate) fn decode_canonical(bytes: &[u8]) -> Result<Vec<Capability>, AbiError> {
        if bytes.len() < 2 {
            return Err(AbiError::InvalidEncoding);
        }
        let count = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
        let mut cursor = 2usize;
        let mut grants = Vec::with_capacity(count);
        for _ in 0..count {
            let tag = *bytes.get(cursor).ok_or(AbiError::InvalidEncoding)?;
            cursor = cursor.checked_add(1).ok_or(AbiError::InvalidEncoding)?;
            let grant = match tag {
                1 => Capability::StorageRead,
                2 => Capability::StorageWrite,
                3 => Capability::EmitEvent,
                4 => Capability::Call {
                    program: ProgramId::new(take_array::<32>(bytes, &mut cursor)?)?,
                },
                5 => Capability::Transfer402 {
                    asset: take_array::<32>(bytes, &mut cursor)?,
                    to: take_array::<32>(bytes, &mut cursor)?,
                    maximum_amount: u128::from_be_bytes(take_array::<16>(bytes, &mut cursor)?),
                },
                6 => Capability::ReceiptRead {
                    receipt_digest: take_array::<32>(bytes, &mut cursor)?,
                },
                _ => return Err(AbiError::InvalidEncoding),
            };
            grants.push(grant);
        }
        if cursor != bytes.len() {
            return Err(AbiError::InvalidEncoding);
        }
        Ok(grants)
    }

    fn receipt_digests(&self) -> impl Iterator<Item = [u8; 32]> + '_ {
        self.0.values().filter_map(|capability| match capability {
            Capability::ReceiptRead { receipt_digest } => Some(*receipt_digest),
            _ => None,
        })
    }
}

/// Exact authority fixed for one invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationContext {
    principal: PrincipalId,
    capabilities: CapabilitySet,
}

impl AuthorizationContext {
    #[must_use]
    pub const fn new(principal: PrincipalId, capabilities: CapabilitySet) -> Self {
        Self {
            principal,
            capabilities,
        }
    }

    #[must_use]
    pub const fn principal(&self) -> PrincipalId {
        self.principal
    }

    #[must_use]
    pub const fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }
}

/// Verified receipt facts exposed without raw kernel state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptView {
    pub receipt_digest: [u8; 32],
    pub result_code: i32,
    pub asset: [u8; 32],
    pub amount: u128,
    pub state_root: [u8; 32],
}

/// Core-owned boundary supplying only locally verified receipt facts.
pub trait ReceiptOracle {
    /// Returns facts only after canonical receipt and authority verification.
    ///
    /// # Errors
    ///
    /// Returns an evidence refusal when the named digest is absent or invalid.
    fn verified_receipt(&self, receipt_digest: [u8; 32]) -> Result<ReceiptView, AbiError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramEvent {
    pub program: ProgramId,
    pub principal: PrincipalId,
    pub topic: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramCall {
    pub caller: ProgramId,
    pub callee: ProgramId,
    pub principal: PrincipalId,
    pub input: Vec<u8>,
    pub capabilities: CapabilitySet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferRequest {
    pub program: ProgramId,
    pub principal: PrincipalId,
    pub asset: [u8; 32],
    pub to: [u8; 32],
    pub amount: u128,
}

/// Effects emitted by one successfully committed ABI transaction. Monetary
/// effects remain typed requests for the kernel's 402LXP transfer primitive;
/// no balance-writing handle exists here.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AbiEffects {
    pub events: Vec<ProgramEvent>,
    pub calls: Vec<ProgramCall>,
    pub transfers: Vec<TransferRequest>,
}

/// Successful atomic ABI state returned to the executor for durable commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbiCommit {
    pub storage: Storage,
    pub effects: AbiEffects,
}

/// Stable capability-ABI refusal taxonomy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbiError {
    WrongVersion,
    InvalidCapability,
    DuplicateCapability,
    CapabilityDenied,
    CapabilityEscalation,
    EventBounds,
    CallBounds,
    AmountBounds,
    ReceiptMismatch,
    InvalidEncoding,
    Storage(StorageError),
    Meter(MeterRefusal),
}

impl Display for AbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongVersion => formatter.write_str("program ABI version is unsupported"),
            Self::InvalidCapability => formatter.write_str("capability declaration is invalid"),
            Self::DuplicateCapability => formatter.write_str("capability is declared twice"),
            Self::CapabilityDenied => formatter.write_str("capability was not granted"),
            Self::CapabilityEscalation => formatter.write_str("capability narrowing escalates"),
            Self::EventBounds => formatter.write_str("program event exceeds ABI bounds"),
            Self::CallBounds => formatter.write_str("program call exceeds ABI bounds"),
            Self::AmountBounds => formatter.write_str("402LXP transfer amount is invalid"),
            Self::ReceiptMismatch => formatter.write_str("verified receipt facts do not match"),
            Self::InvalidEncoding => formatter.write_str("program ABI input encoding is invalid"),
            Self::Storage(error) => write!(formatter, "storage refusal: {error}"),
            Self::Meter(error) => write!(formatter, "meter refusal: {error}"),
        }
    }
}

impl std::error::Error for AbiError {}

impl From<StorageError> for AbiError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

impl From<MeterRefusal> for AbiError {
    fn from(value: MeterRefusal) -> Self {
        Self::Meter(value)
    }
}

/// One atomic, explicitly authorised ABI transaction. Storage is an owned
/// snapshot so a trap or refusal discards every write and emitted effect.
#[derive(Clone, Debug)]
pub struct Abi {
    version: u16,
    program: ProgramId,
    authorization: AuthorizationContext,
    namespace: StorageNamespace,
    storage: Storage,
    receipts: BTreeMap<[u8; 32], ReceiptView>,
    effects: AbiEffects,
}

impl Abi {
    /// Opens the version-one ABI over an atomic namespaced transaction.
    ///
    /// # Errors
    ///
    /// Refuses an ABI version other than the runtime's declared version.
    pub fn new(
        version: u16,
        program: ProgramId,
        authorization: AuthorizationContext,
        storage: Storage,
        receipts: &dyn ReceiptOracle,
    ) -> Result<Self, AbiError> {
        if version != ABI_VERSION {
            return Err(AbiError::WrongVersion);
        }
        let namespace = StorageNamespace::new(program, authorization.principal());
        let mut verified = BTreeMap::new();
        for digest in authorization.capabilities().receipt_digests() {
            let view = receipts.verified_receipt(digest)?;
            if view.receipt_digest != digest {
                return Err(AbiError::ReceiptMismatch);
            }
            verified.insert(digest, view);
        }
        Ok(Self {
            version,
            program,
            authorization,
            namespace,
            storage,
            receipts: verified,
            effects: AbiEffects::default(),
        })
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    #[must_use]
    pub const fn host_functions() -> &'static [HostFunction; 7] {
        &HOST_FUNCTIONS
    }

    #[must_use]
    pub const fn canonical_manifest() -> &'static [u8] {
        ABI_MANIFEST.as_bytes()
    }

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
        self.authorization
            .capabilities()
            .grant(&CapabilityKey::StorageRead)?;
        let value = self.storage.read(self.namespace, key)?;
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
        self.authorization
            .capabilities()
            .grant(&CapabilityKey::StorageWrite)?;
        let bytes = metered_bytes(key, Some(value))?;
        meter.charge_storage_write(bytes)?;
        self.storage.write(self.namespace, key, value)?;
        Ok(())
    }

    /// Stages deletion in the current program/principal namespace.
    ///
    /// # Errors
    ///
    /// Refuses missing authority, invalid keys, or meter exhaustion.
    pub fn storage_delete(&mut self, meter: &mut Meter, key: &[u8]) -> Result<(), AbiError> {
        self.authorization
            .capabilities()
            .grant(&CapabilityKey::StorageWrite)?;
        meter.charge_storage_write(metered_bytes(key, None)?)?;
        self.storage.delete(self.namespace, key)?;
        Ok(())
    }

    /// Emits one event under the current program namespace.
    ///
    /// # Errors
    ///
    /// Refuses missing authority and invalid topic/data bounds.
    pub fn emit_event(&mut self, topic: &[u8], data: &[u8]) -> Result<(), AbiError> {
        self.authorization
            .capabilities()
            .grant(&CapabilityKey::EmitEvent)?;
        if topic.is_empty()
            || topic.len() > MAX_EVENT_TOPIC_BYTES
            || data.len() > MAX_EVENT_DATA_BYTES
        {
            return Err(AbiError::EventBounds);
        }
        self.effects.events.push(ProgramEvent {
            program: self.program,
            principal: self.authorization.principal(),
            topic: topic.to_vec(),
            data: data.to_vec(),
        });
        Ok(())
    }

    /// Requests a program call with an explicitly narrowed capability set.
    ///
    /// # Errors
    ///
    /// Refuses missing call authority, oversized input, or any escalation.
    pub fn call_program(
        &mut self,
        callee: ProgramId,
        input: &[u8],
        requested: impl IntoIterator<Item = Capability>,
    ) -> Result<(), AbiError> {
        self.authorization
            .capabilities()
            .grant(&CapabilityKey::Call(callee))?;
        if input.len() > MAX_CALL_INPUT_BYTES {
            return Err(AbiError::CallBounds);
        }
        let capabilities = self.authorization.capabilities().narrow(requested)?;
        self.effects.calls.push(ProgramCall {
            caller: self.program,
            callee,
            principal: self.authorization.principal(),
            input: input.to_vec(),
            capabilities,
        });
        Ok(())
    }

    /// Requests an authenticated 402LXP transfer for the kernel to apply.
    ///
    /// # Errors
    ///
    /// Refuses missing authority, a zero amount, or a grant-limit excess.
    pub fn request_transfer(
        &mut self,
        asset: [u8; 32],
        to: [u8; 32],
        amount: u128,
    ) -> Result<(), AbiError> {
        if amount == 0 {
            return Err(AbiError::AmountBounds);
        }
        let grant = self
            .authorization
            .capabilities()
            .grant(&CapabilityKey::Transfer { asset, to })?;
        let Capability::Transfer402 { maximum_amount, .. } = grant else {
            return Err(AbiError::CapabilityDenied);
        };
        if amount > *maximum_amount {
            return Err(AbiError::CapabilityEscalation);
        }
        self.effects.transfers.push(TransferRequest {
            program: self.program,
            principal: self.authorization.principal(),
            asset,
            to,
            amount,
        });
        Ok(())
    }

    /// Reads facts through the core's receipt-verification authority.
    ///
    /// # Errors
    ///
    /// Refuses missing digest authority and all absent or mismatched evidence.
    pub fn receipt_read(&self, receipt_digest: [u8; 32]) -> Result<ReceiptView, AbiError> {
        self.authorization
            .capabilities()
            .grant(&CapabilityKey::ReceiptRead(receipt_digest))?;
        self.receipts
            .get(&receipt_digest)
            .cloned()
            .ok_or(AbiError::ReceiptMismatch)
    }

    /// Atomically commits storage and returns effects for the kernel. Dropping
    /// an ABI transaction instead rolls all storage writes and effects back.
    #[must_use]
    pub fn commit(self) -> AbiCommit {
        AbiCommit {
            storage: self.storage,
            effects: self.effects,
        }
    }
}

fn take_array<const N: usize>(bytes: &[u8], cursor: &mut usize) -> Result<[u8; N], AbiError> {
    let end = cursor.checked_add(N).ok_or(AbiError::InvalidEncoding)?;
    let slice = bytes.get(*cursor..end).ok_or(AbiError::InvalidEncoding)?;
    let mut output = [0u8; N];
    output.copy_from_slice(slice);
    *cursor = end;
    Ok(output)
}
