//! Field-addressed execution context exposed to guest code.
//!
//! A program reads one context field at a time through a single frozen host
//! function. Every field is derived from protocol state alone: the executing
//! program and its immediate caller come from the host-maintained call graph,
//! the invoking principal and the remaining fuel come from the authority and
//! meter fixed before guest entry, and the activity sequence, batch height,
//! runtime and ABI versions and effective fee-schedule version are ambient
//! facts the transition function supplies for the whole activity. No field
//! reads a wall-clock time, host entropy, or any node-local value, and no field
//! is writable by guest code. An unknown field identifier is refused rather
//! than answered with a zero.

use crate::execute::{ABI_VERSION, RUNTIME_VERSION};
use crate::storage::{PrincipalId, ProgramId};

use super::{AbiValueType, HostFunction, HostFunctionType};

/// Canonical name of the single field-addressed context host function.
pub const CONTEXT_READ_NAME: &str = "context_read";

/// Frozen WebAssembly signature of the context host function. It takes a field
/// identifier and a bounded output region and returns the number of bytes
/// written or a negative status.
pub const CONTEXT_READ_SIGNATURE: &str = "(i32,i32,i32)->i32";

/// The default effective fee-schedule version, matching the version-one
/// governed fee schedule the runtime prices execution under.
pub const DEFAULT_FEE_SCHEDULE_VERSION: u32 = 1;

/// Domain separator for the frozen context field manifest.
const CONTEXT_MANIFEST_DOMAIN: &[u8] = b"LXP/programs/context/v1\0";

/// Stable frozen field identifiers. Values never change once shipped.
pub const CONTEXT_FIELD_EXECUTING_PROGRAM: u32 = 1;
pub const CONTEXT_FIELD_CALLING_PROGRAM: u32 = 2;
pub const CONTEXT_FIELD_INVOKING_PRINCIPAL: u32 = 3;
pub const CONTEXT_FIELD_ACTIVITY_SEQUENCE: u32 = 4;
pub const CONTEXT_FIELD_BATCH_HEIGHT: u32 = 5;
pub const CONTEXT_FIELD_RUNTIME_VERSION: u32 = 6;
pub const CONTEXT_FIELD_ABI_VERSION: u32 = 7;
pub const CONTEXT_FIELD_REMAINING_FUEL: u32 = 8;
pub const CONTEXT_FIELD_FEE_SCHEDULE_VERSION: u32 = 9;

/// The single host-function surface exposed by the context ABI.
pub const CONTEXT_HOST_FUNCTION: HostFunction = HostFunction {
    name: CONTEXT_READ_NAME,
    signature: CONTEXT_READ_SIGNATURE,
};

const CONTEXT_READ_PARAMS: &[AbiValueType] =
    &[AbiValueType::I32, AbiValueType::I32, AbiValueType::I32];
const CONTEXT_READ_RESULT: &[AbiValueType] = &[AbiValueType::I32];
const CONTEXT_READ_TYPE: HostFunctionType = HostFunctionType {
    params: CONTEXT_READ_PARAMS,
    results: CONTEXT_READ_RESULT,
};

/// Returns the frozen host-function type of a context ABI import, so module
/// validation admits `context_read` with exactly its declared signature and
/// refuses any other name in this family.
#[must_use]
pub(crate) fn context_function_type(name: &str) -> Option<&'static HostFunctionType> {
    match name {
        CONTEXT_READ_NAME => Some(&CONTEXT_READ_TYPE),
        _ => None,
    }
}

/// One field the execution context exposes, addressed by a frozen identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextField {
    /// The program executing in the current call frame.
    ExecutingProgram,
    /// The immediate calling program, absent at the activity's entry frame.
    CallingProgram,
    /// The principal whose authority invoked the activity.
    InvokingPrincipal,
    /// The monotonic sequence number of the activity within its batch.
    ActivitySequence,
    /// The protocol batch height the activity executes in.
    BatchHeight,
    /// The runtime version driving execution.
    RuntimeVersion,
    /// The ABI version the module executes under.
    AbiVersion,
    /// The instruction fuel remaining to the whole call graph.
    RemainingFuel,
    /// The effective fee-schedule version pricing the execution.
    FeeScheduleVersion,
}

impl ContextField {
    /// Every field in frozen identifier order.
    pub const ALL: [Self; 9] = [
        Self::ExecutingProgram,
        Self::CallingProgram,
        Self::InvokingPrincipal,
        Self::ActivitySequence,
        Self::BatchHeight,
        Self::RuntimeVersion,
        Self::AbiVersion,
        Self::RemainingFuel,
        Self::FeeScheduleVersion,
    ];

    /// Resolves a frozen field identifier, refusing any unknown value rather
    /// than defaulting to a field.
    #[must_use]
    pub const fn from_id(id: u32) -> Option<Self> {
        match id {
            CONTEXT_FIELD_EXECUTING_PROGRAM => Some(Self::ExecutingProgram),
            CONTEXT_FIELD_CALLING_PROGRAM => Some(Self::CallingProgram),
            CONTEXT_FIELD_INVOKING_PRINCIPAL => Some(Self::InvokingPrincipal),
            CONTEXT_FIELD_ACTIVITY_SEQUENCE => Some(Self::ActivitySequence),
            CONTEXT_FIELD_BATCH_HEIGHT => Some(Self::BatchHeight),
            CONTEXT_FIELD_RUNTIME_VERSION => Some(Self::RuntimeVersion),
            CONTEXT_FIELD_ABI_VERSION => Some(Self::AbiVersion),
            CONTEXT_FIELD_REMAINING_FUEL => Some(Self::RemainingFuel),
            CONTEXT_FIELD_FEE_SCHEDULE_VERSION => Some(Self::FeeScheduleVersion),
            _ => None,
        }
    }

    /// Returns the frozen numeric identifier of this field.
    #[must_use]
    pub const fn id(self) -> u32 {
        match self {
            Self::ExecutingProgram => CONTEXT_FIELD_EXECUTING_PROGRAM,
            Self::CallingProgram => CONTEXT_FIELD_CALLING_PROGRAM,
            Self::InvokingPrincipal => CONTEXT_FIELD_INVOKING_PRINCIPAL,
            Self::ActivitySequence => CONTEXT_FIELD_ACTIVITY_SEQUENCE,
            Self::BatchHeight => CONTEXT_FIELD_BATCH_HEIGHT,
            Self::RuntimeVersion => CONTEXT_FIELD_RUNTIME_VERSION,
            Self::AbiVersion => CONTEXT_FIELD_ABI_VERSION,
            Self::RemainingFuel => CONTEXT_FIELD_REMAINING_FUEL,
            Self::FeeScheduleVersion => CONTEXT_FIELD_FEE_SCHEDULE_VERSION,
        }
    }

    /// Returns the stable field name frozen in the golden manifest.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ExecutingProgram => "executing_program",
            Self::CallingProgram => "calling_program",
            Self::InvokingPrincipal => "invoking_principal",
            Self::ActivitySequence => "activity_sequence",
            Self::BatchHeight => "batch_height",
            Self::RuntimeVersion => "runtime_version",
            Self::AbiVersion => "abi_version",
            Self::RemainingFuel => "remaining_fuel",
            Self::FeeScheduleVersion => "fee_schedule_version",
        }
    }

    /// Returns the exact number of bytes this field encodes to. The calling
    /// program carries a one-byte presence flag ahead of its identifier so an
    /// absent caller at the entry frame is a defined encoding rather than a
    /// zero identifier that a program could mistake for a real one.
    #[must_use]
    pub const fn encoded_len(self) -> usize {
        match self {
            Self::ExecutingProgram | Self::InvokingPrincipal => 32,
            Self::CallingProgram => 33,
            Self::ActivitySequence | Self::BatchHeight | Self::RemainingFuel => 8,
            Self::RuntimeVersion | Self::AbiVersion => 2,
            Self::FeeScheduleVersion => 4,
        }
    }
}

/// The ambient, invocation-wide protocol facts of one execution context. These
/// are fixed for the whole activity and copied unchanged into every nested
/// frame, so composition never alters the sequence, height or versions a
/// program observes. The per-frame identity fields (executing program, caller,
/// principal, remaining fuel) are supplied at read time from host state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionContext {
    activity_sequence: u64,
    batch_height: u64,
    runtime_version: u16,
    abi_version: u16,
    fee_schedule_version: u32,
}

impl ExecutionContext {
    /// Constructs an execution context from explicit protocol facts.
    #[must_use]
    pub const fn new(
        activity_sequence: u64,
        batch_height: u64,
        runtime_version: u16,
        abi_version: u16,
        fee_schedule_version: u32,
    ) -> Self {
        Self {
            activity_sequence,
            batch_height,
            runtime_version,
            abi_version,
            fee_schedule_version,
        }
    }

    /// Returns the declared execution context at the origin of protocol time:
    /// the runtime and ABI versions the crate ships and the version-one fee
    /// schedule, with a zero activity sequence and batch height. Production
    /// transition code overrides the sequence and height with real protocol
    /// state through [`Self::at`].
    #[must_use]
    pub const fn declared() -> Self {
        Self::new(
            0,
            0,
            RUNTIME_VERSION,
            ABI_VERSION,
            DEFAULT_FEE_SCHEDULE_VERSION,
        )
    }

    /// Returns the declared context positioned at a real activity sequence and
    /// batch height while keeping the crate's runtime, ABI and fee-schedule
    /// versions.
    #[must_use]
    pub const fn at(activity_sequence: u64, batch_height: u64) -> Self {
        Self::new(
            activity_sequence,
            batch_height,
            RUNTIME_VERSION,
            ABI_VERSION,
            DEFAULT_FEE_SCHEDULE_VERSION,
        )
    }

    /// Returns the activity's monotonic sequence within its batch.
    #[must_use]
    pub const fn activity_sequence(self) -> u64 {
        self.activity_sequence
    }

    /// Returns the protocol batch height of the activity.
    #[must_use]
    pub const fn batch_height(self) -> u64 {
        self.batch_height
    }

    /// Returns the runtime version driving execution.
    #[must_use]
    pub const fn runtime_version(self) -> u16 {
        self.runtime_version
    }

    /// Returns the ABI version the module executes under.
    #[must_use]
    pub const fn abi_version(self) -> u16 {
        self.abi_version
    }

    /// Returns the effective fee-schedule version pricing the execution.
    #[must_use]
    pub const fn fee_schedule_version(self) -> u32 {
        self.fee_schedule_version
    }

    /// Encodes one context field into architecture-independent bytes. Every
    /// multi-byte integer uses network byte order so the same activity yields
    /// the same bytes on every operating system, architecture and optimisation
    /// level. The executing program, caller and principal come from host state
    /// and can never be supplied by guest code.
    #[must_use]
    pub fn encode_field(
        self,
        field: ContextField,
        executing_program: ProgramId,
        calling_program: Option<ProgramId>,
        principal: PrincipalId,
        remaining_fuel: u64,
    ) -> Vec<u8> {
        match field {
            ContextField::ExecutingProgram => executing_program.bytes().to_vec(),
            ContextField::CallingProgram => {
                let mut encoded = Vec::with_capacity(33);
                match calling_program {
                    Some(caller) => {
                        encoded.push(1);
                        encoded.extend_from_slice(&caller.bytes());
                    }
                    None => {
                        encoded.push(0);
                        encoded.extend_from_slice(&[0u8; 32]);
                    }
                }
                encoded
            }
            ContextField::InvokingPrincipal => principal.bytes().to_vec(),
            ContextField::ActivitySequence => self.activity_sequence.to_be_bytes().to_vec(),
            ContextField::BatchHeight => self.batch_height.to_be_bytes().to_vec(),
            ContextField::RuntimeVersion => self.runtime_version.to_be_bytes().to_vec(),
            ContextField::AbiVersion => self.abi_version.to_be_bytes().to_vec(),
            ContextField::RemainingFuel => remaining_fuel.to_be_bytes().to_vec(),
            ContextField::FeeScheduleVersion => self.fee_schedule_version.to_be_bytes().to_vec(),
        }
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::declared()
    }
}

/// Builds the frozen context field manifest: the domain separator, the module
/// and the context host-function signature, then, for each field in identifier
/// order, its numeric identifier, its name and its exact encoded length. The
/// golden vector freezes both the identifiers and the shape of their encodings
/// so no field can silently change width or meaning.
#[must_use]
pub fn canonical_field_manifest() -> Vec<u8> {
    let mut manifest = CONTEXT_MANIFEST_DOMAIN.to_vec();
    manifest.extend_from_slice(super::response::CANDIDATE_ABI_MODULE.as_bytes());
    manifest.push(0);
    manifest.extend_from_slice(CONTEXT_READ_NAME.as_bytes());
    manifest.extend_from_slice(CONTEXT_READ_SIGNATURE.as_bytes());
    manifest.push(0);
    let field_count = u32::try_from(ContextField::ALL.len()).unwrap_or(u32::MAX);
    manifest.extend_from_slice(&field_count.to_be_bytes());
    for field in ContextField::ALL {
        manifest.extend_from_slice(&field.id().to_be_bytes());
        manifest.extend_from_slice(field.name().as_bytes());
        manifest.push(0);
        let encoded_len = u32::try_from(field.encoded_len()).unwrap_or(u32::MAX);
        manifest.extend_from_slice(&encoded_len.to_be_bytes());
    }
    manifest
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn field_identifiers_round_trip_through_their_frozen_values() {
        for field in ContextField::ALL {
            assert_eq!(ContextField::from_id(field.id()), Some(field));
        }
    }

    #[test]
    fn unknown_field_identifier_is_refused_not_defaulted() {
        assert_eq!(ContextField::from_id(0), None);
        assert_eq!(ContextField::from_id(10), None);
        assert_eq!(ContextField::from_id(u32::MAX), None);
    }

    #[test]
    fn encoded_lengths_match_the_field_encodings() {
        let program = ProgramId::new([0x11; 32]).expect("program id");
        let caller = ProgramId::new([0x22; 32]).expect("caller id");
        let principal = PrincipalId::new([0x33; 32]).expect("principal id");
        let context = ExecutionContext::new(7, 9, 1, 1, 1);
        for field in ContextField::ALL {
            let encoded = context.encode_field(field, program, Some(caller), principal, 42);
            assert_eq!(encoded.len(), field.encoded_len(), "{}", field.name());
        }
    }

    #[test]
    fn absent_caller_encodes_a_zero_presence_flag() {
        let program = ProgramId::new([0x11; 32]).expect("program id");
        let principal = PrincipalId::new([0x33; 32]).expect("principal id");
        let context = ExecutionContext::declared();
        let encoded =
            context.encode_field(ContextField::CallingProgram, program, None, principal, 0);
        assert_eq!(encoded.len(), 33);
        assert_eq!(encoded[0], 0);
        assert_eq!(&encoded[1..], &[0u8; 32]);
    }
}
