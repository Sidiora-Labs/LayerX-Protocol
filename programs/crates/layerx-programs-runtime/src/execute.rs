//! Typed execution surface over instantiated deterministic programs.

use core::fmt::{self, Display};

use wasmi::core::TrapCode;
use wasmi::{Extern, Instance, Memory, Store, Value};

use crate::abi::response::{CallResponse, ResponseRefusal};
use crate::abi::{Abi, AbiEffects, AbiError, AuthorizationContext, ReceiptOracle};
use crate::calls::{CallGraph, Composition, CompositionContext, CompositionRefusal};
use crate::entrypoint::{self, EntrypointRefusal};
use crate::fault::{ProgramFailure, RefusalClass, RefusalReason, CANDIDATE_REFUSAL_SENTINEL};
use crate::host::RuntimeState;
use crate::meter::{FeeSchedule, Meter, MeterRefusal, MeteredUsage, ResourceBudget, ResourceKind};
use crate::storage::{ProgramId, Storage};
use crate::validate::{AbiRevision, ValidatedModule};

/// Runtime version recorded for versioned replay of every execution.
pub const RUNTIME_VERSION: u16 = 1;
/// ABI version recorded alongside the runtime version in execution evidence.
pub const ABI_VERSION: u16 = 1;

/// An integer-only value crossing the program boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmValue {
    /// A 32-bit integer value.
    I32(i32),
    /// A 64-bit integer value.
    I64(i64),
}

impl From<WasmValue> for Value {
    fn from(value: WasmValue) -> Self {
        match value {
            WasmValue::I32(inner) => Self::I32(inner),
            WasmValue::I64(inner) => Self::I64(inner),
        }
    }
}

/// A typed fault produced while instantiating or executing a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionFault {
    /// The named export does not exist.
    UnknownExport {
        /// The export name that was not found.
        name: String,
    },
    /// The named export exists but is not a function.
    NotAFunction {
        /// The export name that is not a function.
        name: String,
    },
    /// Guest code executed the `unreachable` instruction.
    UnreachableExecuted,
    /// Guest code accessed linear memory out of bounds.
    MemoryOutOfBounds,
    /// Guest code accessed a table out of bounds.
    TableOutOfBounds,
    /// Guest code called an uninitialised table element indirectly.
    IndirectCallToNull,
    /// Guest code divided an integer by zero.
    IntegerDivisionByZero,
    /// Guest integer arithmetic overflowed.
    IntegerOverflow,
    /// Guest code attempted an invalid integer conversion.
    BadConversionToInteger,
    /// Execution exceeded the declared value stack height or call depth.
    StackExhausted,
    /// An indirect call used a mismatching signature.
    BadSignature,
    /// Execution exhausted its metered fuel budget.
    OutOfFuel,
    /// A growth operation was refused by a resource limit.
    GrowthLimited,
    /// A deterministic meter refused the execution before an effect escaped.
    Resource {
        /// Exact resource refusal.
        refusal: MeterRefusal,
    },
    /// A program value crossed the boundary outside the integer subset.
    NonIntegerValue,
    /// The engine reported a fault outside the typed trap set.
    EngineFault {
        /// The engine's description of the fault.
        reason: String,
    },
}

impl Display for ExecutionFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownExport { name } => write!(f, "unknown export {name}"),
            Self::NotAFunction { name } => write!(f, "export {name} is not a function"),
            Self::UnreachableExecuted => write!(f, "unreachable instruction executed"),
            Self::MemoryOutOfBounds => write!(f, "memory access out of bounds"),
            Self::TableOutOfBounds => write!(f, "table access out of bounds"),
            Self::IndirectCallToNull => write!(f, "indirect call to null table element"),
            Self::IntegerDivisionByZero => write!(f, "integer division by zero"),
            Self::IntegerOverflow => write!(f, "integer overflow"),
            Self::BadConversionToInteger => write!(f, "invalid conversion to integer"),
            Self::StackExhausted => write!(f, "declared stack or call depth limit exhausted"),
            Self::BadSignature => write!(f, "indirect call signature mismatch"),
            Self::OutOfFuel => write!(f, "metered fuel budget exhausted"),
            Self::GrowthLimited => write!(f, "growth operation refused by resource limit"),
            Self::Resource { refusal } => write!(f, "resource refusal: {refusal}"),
            Self::NonIntegerValue => write!(f, "non-integer value crossed the boundary"),
            Self::EngineFault { reason } => write!(f, "engine fault: {reason}"),
        }
    }
}

impl std::error::Error for ExecutionFault {}

pub(crate) fn fault_from_error(error: &wasmi::Error) -> ExecutionFault {
    if let wasmi::Error::Trap(trap) = error {
        if let Some(code) = trap.trap_code() {
            return fault_from_trap_code(code);
        }
    }
    ExecutionFault::EngineFault {
        reason: error.to_string(),
    }
}

const fn fault_from_trap_code(code: TrapCode) -> ExecutionFault {
    match code {
        TrapCode::UnreachableCodeReached => ExecutionFault::UnreachableExecuted,
        TrapCode::MemoryOutOfBounds => ExecutionFault::MemoryOutOfBounds,
        TrapCode::TableOutOfBounds => ExecutionFault::TableOutOfBounds,
        TrapCode::IndirectCallToNull => ExecutionFault::IndirectCallToNull,
        TrapCode::IntegerDivisionByZero => ExecutionFault::IntegerDivisionByZero,
        TrapCode::IntegerOverflow => ExecutionFault::IntegerOverflow,
        TrapCode::BadConversionToInteger => ExecutionFault::BadConversionToInteger,
        TrapCode::StackOverflow => ExecutionFault::StackExhausted,
        TrapCode::BadSignature => ExecutionFault::BadSignature,
        TrapCode::OutOfFuel => ExecutionFault::OutOfFuel,
        TrapCode::GrowthOperationLimited => ExecutionFault::GrowthLimited,
    }
}

/// An instantiated program isolated inside its own store.
#[derive(Debug)]
pub struct ProgramInstance {
    store: Store<RuntimeState>,
    instance: Instance,
}

impl ProgramInstance {
    pub(crate) const fn new(store: Store<RuntimeState>, instance: Instance) -> Self {
        Self { store, instance }
    }

    /// Calls an exported function with integer arguments.
    ///
    /// # Errors
    ///
    /// Returns a typed [`ExecutionFault`] when the export is missing or not a
    /// function, when execution traps, or when a non-integer value would cross
    /// the boundary.
    pub fn call(
        &mut self,
        export: &str,
        args: &[WasmValue],
    ) -> Result<Vec<WasmValue>, ExecutionFault> {
        let Some(external) = self.instance.get_export(&self.store, export) else {
            return Err(ExecutionFault::UnknownExport {
                name: export.to_string(),
            });
        };
        let Some(func) = external.into_func() else {
            return Err(ExecutionFault::NotAFunction {
                name: export.to_string(),
            });
        };
        let result_count = func.ty(&self.store).results().len();
        self.store
            .data_mut()
            .meter_mut()
            .charge_output(result_count)
            .map_err(|refusal| ExecutionFault::Resource { refusal })?;
        let inputs: Vec<Value> = args.iter().copied().map(Value::from).collect();
        let mut outputs = vec![Value::I64(0); result_count];
        let result = func.call(&mut self.store, &inputs, &mut outputs);
        let Some(consumed) = self.store.fuel_consumed() else {
            return Err(ExecutionFault::EngineFault {
                reason: "fuel metering disabled".to_string(),
            });
        };
        self.store.data_mut().meter_mut().record_cpu(consumed);
        if let Err(error) = result {
            let fault = fault_from_error(&error);
            if fault == ExecutionFault::OutOfFuel {
                self.store.data_mut().meter_mut().mark_cpu_exhausted();
            }
            return Err(fault);
        }
        outputs
            .into_iter()
            .map(|value| match value {
                Value::I32(inner) => Ok(WasmValue::I32(inner)),
                Value::I64(inner) => Ok(WasmValue::I64(inner)),
                Value::F32(_) | Value::F64(_) | Value::FuncRef(_) | Value::ExternRef(_) => {
                    Err(ExecutionFault::NonIntegerValue)
                }
            })
            .collect()
    }

    /// Borrows the exact meter state for this isolated execution.
    #[must_use]
    pub fn meter(&self) -> &Meter {
        self.store.data().meter()
    }

    pub(crate) fn state(&self) -> &RuntimeState {
        self.store.data()
    }

    pub(crate) fn linear_memory(&self) -> Option<Memory> {
        self.instance
            .get_export(&self.store, "memory")
            .and_then(Extern::into_memory)
    }

    pub(crate) fn write_linear_memory(
        &mut self,
        memory: Memory,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), ExecutionFault> {
        memory
            .write(&mut self.store, offset, bytes)
            .map_err(|_| ExecutionFault::MemoryOutOfBounds)
    }

    pub(crate) fn consume_copy_fuel(&mut self, fuel: u64) -> Result<(), EntrypointRefusal> {
        if self.store.consume_fuel(fuel).is_err() {
            self.store.data_mut().meter_mut().mark_cpu_exhausted();
            return Err(EntrypointRefusal::Resource(MeterRefusal::BudgetExceeded {
                resource: ResourceKind::Cpu,
                limit: self.meter().cpu_budget(),
                attempted: self.meter().cpu_budget().saturating_add(1),
            }));
        }
        Ok(())
    }

    pub(crate) fn into_state(self) -> RuntimeState {
        self.store.into_data()
    }
}

/// Receipt-carriable deterministic execution result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecord {
    /// Runtime version under which the program executed.
    pub runtime_version: u16,
    /// ABI version under which the program executed.
    pub abi_version: u16,
    /// Integer-only guest outputs.
    pub outputs: Vec<WasmValue>,
    /// Exact deterministic resource use and fee units.
    pub usage: MeteredUsage,
}

/// Successful authorized execution plus effects awaiting the kernel's atomic
/// application boundary. The effects and the call graph belong to the whole
/// composition: every program the activity entered contributed to them, and a
/// refusal anywhere in the graph returns none of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedExecutionRecord {
    pub execution: ExecutionRecord,
    pub effects: AbiEffects,
    pub call_graph: CallGraph,
}

/// Qualification-only result produced under the explicitly selected candidate ABI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateAuthorizedExecutionRecord {
    root_program: ProgramId,
    abi_revision: AbiRevision,
    execution: CandidateExecutionRecord,
    outcome: CandidateActivityOutcome,
    call_graph: CallGraph,
}

/// Mutually exclusive candidate activity result carried into receipt projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateActivityOutcome {
    Success {
        response: CallResponse,
        effects: AbiEffects,
    },
    Failure(ProgramFailure),
}

/// Public, canonical candidate activity receipt projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateActivityReceipt {
    root_program: ProgramId,
    abi_revision: u16,
    runtime_version: u16,
    usage: MeteredUsage,
    graph_evidence: Vec<u8>,
    outcome: CandidateReceiptOutcome,
}

/// Receipt outcome with no representable success/failure overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateReceiptOutcome {
    Success(CallResponse),
    Failure(ProgramFailure),
}

/// Execution facts that cannot be confused with frozen v1 receipt evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateExecutionRecord {
    runtime_version: u16,
    outputs: Vec<WasmValue>,
    usage: MeteredUsage,
}

impl CandidateAuthorizedExecutionRecord {
    #[must_use]
    pub const fn root_program(&self) -> ProgramId {
        self.root_program
    }

    #[must_use]
    pub const fn abi_revision(&self) -> AbiRevision {
        self.abi_revision
    }

    #[must_use]
    pub const fn execution(&self) -> &CandidateExecutionRecord {
        &self.execution
    }

    #[must_use]
    pub const fn outcome(&self) -> &CandidateActivityOutcome {
        &self.outcome
    }

    #[must_use]
    pub const fn call_graph(&self) -> &CallGraph {
        &self.call_graph
    }

    #[must_use]
    pub fn receipt_projection(&self) -> CandidateActivityReceipt {
        CandidateActivityReceipt {
            root_program: self.root_program,
            abi_revision: 2,
            runtime_version: self.execution.runtime_version,
            usage: self.execution.usage,
            graph_evidence: self.call_graph.canonical_evidence(),
            outcome: match &self.outcome {
                CandidateActivityOutcome::Success { response, .. } => {
                    CandidateReceiptOutcome::Success(response.clone())
                }
                CandidateActivityOutcome::Failure(failure) => {
                    CandidateReceiptOutcome::Failure(failure.clone())
                }
            },
        }
    }
    #[must_use]
    pub const fn response(&self) -> Option<&CallResponse> {
        match &self.outcome {
            CandidateActivityOutcome::Success { response, .. } => Some(response),
            CandidateActivityOutcome::Failure(_) => None,
        }
    }

    #[must_use]
    pub const fn failure(&self) -> Option<&ProgramFailure> {
        match &self.outcome {
            CandidateActivityOutcome::Failure(failure) => Some(failure),
            CandidateActivityOutcome::Success { .. } => None,
        }
    }

    #[must_use]
    pub const fn effects(&self) -> Option<&AbiEffects> {
        match &self.outcome {
            CandidateActivityOutcome::Success { effects, .. } => Some(effects),
            CandidateActivityOutcome::Failure(_) => None,
        }
    }

    #[must_use]
    pub fn canonical_evidence(&self) -> Vec<u8> {
        let mut evidence = b"LXP/program-execution/v2-candidate\0".to_vec();
        evidence.extend_from_slice(&self.execution.runtime_version.to_be_bytes());
        evidence.extend_from_slice(&(self.execution.outputs.len() as u64).to_be_bytes());
        for output in &self.execution.outputs {
            match output {
                WasmValue::I32(value) => {
                    evidence.push(1);
                    evidence.extend_from_slice(&value.to_be_bytes());
                }
                WasmValue::I64(value) => {
                    evidence.push(2);
                    evidence.extend_from_slice(&value.to_be_bytes());
                }
            }
        }
        evidence.extend_from_slice(&self.execution.usage.cpu_fuel.to_be_bytes());
        evidence.extend_from_slice(&self.execution.usage.memory_bytes.to_be_bytes());
        evidence.extend_from_slice(&self.execution.usage.storage_read_bytes.to_be_bytes());
        evidence.extend_from_slice(&self.execution.usage.storage_write_bytes.to_be_bytes());
        evidence.extend_from_slice(&self.execution.usage.output_values.to_be_bytes());
        evidence.extend_from_slice(&self.execution.usage.output_bytes.to_be_bytes());
        evidence.extend_from_slice(&self.execution.usage.fee_units.to_be_bytes());
        evidence.extend_from_slice(&self.root_program.bytes());
        let abi_revision = match self.abi_revision {
            AbiRevision::V1 => ABI_VERSION,
            AbiRevision::CandidateV2 => 2,
        };
        evidence.extend_from_slice(&abi_revision.to_be_bytes());
        match &self.outcome {
            CandidateActivityOutcome::Failure(failure) => {
                evidence.push(1);
                let failure = failure.canonical_encode();
                evidence.extend_from_slice(&(failure.len() as u64).to_be_bytes());
                evidence.extend_from_slice(&failure);
            }
            CandidateActivityOutcome::Success { response, .. } => {
                evidence.push(0);
                evidence.extend_from_slice(&response.code.to_be_bytes());
                evidence.extend_from_slice(&(response.bytes.len() as u64).to_be_bytes());
                evidence.extend_from_slice(&response.bytes);
            }
        }
        let graph = self.call_graph.canonical_evidence();
        evidence.extend_from_slice(&(graph.len() as u64).to_be_bytes());
        evidence.extend_from_slice(&graph);
        evidence
    }
}

impl CandidateExecutionRecord {
    #[must_use]
    pub const fn runtime_version(&self) -> u16 {
        self.runtime_version
    }

    #[must_use]
    pub fn outputs(&self) -> &[WasmValue] {
        &self.outputs
    }

    #[must_use]
    pub const fn usage(&self) -> MeteredUsage {
        self.usage
    }
}

impl CandidateActivityReceipt {
    const DOMAIN: &'static [u8] = b"LXP/candidate-activity-receipt/v2\0";
    const MAX_GRAPH_EVIDENCE_BYTES: usize = b"LayerX/programs/call-graph/v1\0".len()
        + 32
        + 16
        + 8
        + (crate::calls::DEFAULT_MAX_CALL_GRAPH_EDGES as usize * 68);

    #[must_use]
    pub const fn root_program(&self) -> ProgramId {
        self.root_program
    }

    #[must_use]
    pub const fn abi_revision(&self) -> u16 {
        self.abi_revision
    }

    #[must_use]
    pub const fn runtime_version(&self) -> u16 {
        self.runtime_version
    }

    #[must_use]
    pub const fn usage(&self) -> MeteredUsage {
        self.usage
    }

    #[must_use]
    pub fn graph_evidence(&self) -> &[u8] {
        &self.graph_evidence
    }

    #[must_use]
    pub const fn outcome(&self) -> &CandidateReceiptOutcome {
        &self.outcome
    }

    #[must_use]
    pub fn canonical_encode(&self) -> Vec<u8> {
        let mut encoded = Self::DOMAIN.to_vec();
        encoded.extend_from_slice(&self.root_program.bytes());
        encoded.extend_from_slice(&self.abi_revision.to_be_bytes());
        encoded.extend_from_slice(&self.runtime_version.to_be_bytes());
        for value in [
            self.usage.cpu_fuel,
            self.usage.memory_bytes,
            self.usage.storage_read_bytes,
            self.usage.storage_write_bytes,
            self.usage.output_bytes,
        ] {
            encoded.extend_from_slice(&value.to_be_bytes());
        }
        encoded.extend_from_slice(&self.usage.output_values.to_be_bytes());
        encoded.extend_from_slice(&self.usage.fee_units.to_be_bytes());
        encoded.extend_from_slice(
            &u32::try_from(self.graph_evidence.len())
                .unwrap_or(u32::MAX)
                .to_be_bytes(),
        );
        encoded.extend_from_slice(&self.graph_evidence);
        match &self.outcome {
            CandidateReceiptOutcome::Success(response) => {
                encoded.push(0);
                encoded.extend_from_slice(&response.code.to_be_bytes());
                encoded.extend_from_slice(
                    &u32::try_from(response.bytes.len())
                        .unwrap_or(u32::MAX)
                        .to_be_bytes(),
                );
                encoded.extend_from_slice(&response.bytes);
            }
            CandidateReceiptOutcome::Failure(failure) => {
                encoded.push(1);
                let failure = failure.canonical_encode();
                encoded.extend_from_slice(
                    &u32::try_from(failure.len())
                        .unwrap_or(u32::MAX)
                        .to_be_bytes(),
                );
                encoded.extend_from_slice(&failure);
            }
        }
        encoded
    }

    /// Strictly decodes a candidate receipt projection.
    ///
    /// # Errors
    ///
    /// Refuses the wrong domain/revision, invalid fields, truncation, and trailing bytes.
    pub fn canonical_decode(encoded: &[u8]) -> Result<Self, crate::fault::FailureEncodingError> {
        use crate::fault::FailureEncodingError as Error;
        let mut cursor = ReceiptCursor::new(encoded);
        if cursor.take(Self::DOMAIN.len())? != Self::DOMAIN {
            return Err(Error::Malformed);
        }
        let root_program =
            ProgramId::new(cursor.array::<32>()?).map_err(|_| Error::InvalidProgram)?;
        let abi_revision = u16::from_be_bytes(cursor.array()?);
        if abi_revision != 2 {
            return Err(Error::Malformed);
        }
        let runtime_version = u16::from_be_bytes(cursor.array()?);
        let usage = MeteredUsage {
            cpu_fuel: u64::from_be_bytes(cursor.array()?),
            memory_bytes: u64::from_be_bytes(cursor.array()?),
            storage_read_bytes: u64::from_be_bytes(cursor.array()?),
            storage_write_bytes: u64::from_be_bytes(cursor.array()?),
            output_bytes: u64::from_be_bytes(cursor.array()?),
            output_values: u32::from_be_bytes(cursor.array()?),
            fee_units: u128::from_be_bytes(cursor.array()?),
        };
        let graph_length = u32::from_be_bytes(cursor.array()?) as usize;
        if graph_length > Self::MAX_GRAPH_EVIDENCE_BYTES {
            return Err(Error::Malformed);
        }
        let graph_evidence = cursor.take(graph_length)?.to_vec();
        let tag = cursor.take(1)?[0];
        let outcome = match tag {
            0 => {
                let code = i32::from_be_bytes(cursor.array()?);
                if code < 0 {
                    return Err(Error::Malformed);
                }
                let length = u32::from_be_bytes(cursor.array()?) as usize;
                if length > crate::MAX_CALL_RESPONSE_BYTES {
                    return Err(Error::Malformed);
                }
                CandidateReceiptOutcome::Success(CallResponse {
                    code,
                    bytes: cursor.take(length)?.to_vec(),
                })
            }
            1 => {
                let length = u32::from_be_bytes(cursor.array()?) as usize;
                CandidateReceiptOutcome::Failure(ProgramFailure::canonical_decode(
                    cursor.take(length)?,
                )?)
            }
            _ => return Err(Error::Malformed),
        };
        if !cursor.is_empty() {
            return Err(Error::Malformed);
        }
        Ok(Self {
            root_program,
            abi_revision,
            runtime_version,
            usage,
            graph_evidence,
            outcome,
        })
    }
}

struct ReceiptCursor<'a> {
    remaining: &'a [u8],
}

impl<'a> ReceiptCursor<'a> {
    const fn new(remaining: &'a [u8]) -> Self {
        Self { remaining }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], crate::fault::FailureEncodingError> {
        let (value, remaining) = self
            .remaining
            .split_at_checked(length)
            .ok_or(crate::fault::FailureEncodingError::Malformed)?;
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], crate::fault::FailureEncodingError> {
        self.take(N)?
            .try_into()
            .map_err(|_| crate::fault::FailureEncodingError::Malformed)
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}

/// Complete immutable input to one authorized guest execution.
pub struct AuthorizedExecutionRequest<'a> {
    pub module: &'a ValidatedModule,
    pub program: ProgramId,
    pub authorization: AuthorizationContext,
    pub receipts: &'a dyn ReceiptOracle,
    pub entrypoint: &'a str,
    pub calldata: &'a [u8],
    pub composition: CompositionContext,
    pub response_capacity: usize,
}

impl ExecutionRecord {
    /// Encodes the execution outcome into architecture-independent evidence bytes.
    ///
    /// Every integer uses network byte order and every value carries an explicit
    /// width tag, so the same execution can be compared byte-for-byte across
    /// operating systems, CPU architectures and optimisation profiles.
    #[must_use]
    pub fn canonical_evidence(&self) -> Vec<u8> {
        let mut evidence = Vec::with_capacity(64 + self.outputs.len().saturating_mul(9));
        evidence.extend_from_slice(b"LXP/program-execution\0");
        evidence.extend_from_slice(&self.runtime_version.to_be_bytes());
        evidence.extend_from_slice(&self.abi_version.to_be_bytes());
        let native_output_count = self.outputs.len().to_be_bytes();
        let mut output_count = [0u8; 16];
        let count_offset = output_count.len() - native_output_count.len();
        output_count[count_offset..].copy_from_slice(&native_output_count);
        evidence.extend_from_slice(&output_count);
        for output in &self.outputs {
            match output {
                WasmValue::I32(value) => {
                    evidence.push(1);
                    evidence.extend_from_slice(&value.to_be_bytes());
                }
                WasmValue::I64(value) => {
                    evidence.push(2);
                    evidence.extend_from_slice(&value.to_be_bytes());
                }
            }
        }
        evidence.extend_from_slice(&self.usage.cpu_fuel.to_be_bytes());
        evidence.extend_from_slice(&self.usage.memory_bytes.to_be_bytes());
        evidence.extend_from_slice(&self.usage.storage_read_bytes.to_be_bytes());
        evidence.extend_from_slice(&self.usage.storage_write_bytes.to_be_bytes());
        evidence.extend_from_slice(&self.usage.output_values.to_be_bytes());
        evidence.extend_from_slice(&self.usage.fee_units.to_be_bytes());
        evidence
    }
}

/// Failure of an isolated execution; no instance or guest mutation is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// Guest or engine execution fault.
    Fault(ExecutionFault),
    /// Typed resource-budget refusal.
    Resource(MeterRefusal),
    /// Capability ABI refusal before or during execution.
    Abi(AbiError),
    /// Canonical calldata entry protocol refusal.
    Entrypoint(EntrypointRefusal),
    /// Program-to-program composition refusal; no leg of the call graph was
    /// committed.
    Composition(CompositionRefusal),
    /// Candidate successful-response transport refusal.
    Response(ResponseRefusal),
}

impl Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fault(fault) => write!(formatter, "execution fault: {fault}"),
            Self::Resource(refusal) => write!(formatter, "resource refusal: {refusal}"),
            Self::Abi(error) => write!(formatter, "ABI refusal: {error}"),
            Self::Entrypoint(refusal) => write!(formatter, "entrypoint refusal: {refusal}"),
            Self::Composition(refusal) => write!(formatter, "composition refusal: {refusal}"),
            Self::Response(refusal) => write!(formatter, "response refusal: {refusal}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Stateless executor creating a fresh isolated instance for every call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Executor {
    budget: ResourceBudget,
    prices: FeeSchedule,
    runtime_version: u16,
    abi_version: u16,
}

impl Executor {
    /// Constructs an executor with explicit integer-only budgets and prices.
    #[must_use]
    pub const fn new(budget: ResourceBudget, prices: FeeSchedule) -> Self {
        Self {
            budget,
            prices,
            runtime_version: RUNTIME_VERSION,
            abi_version: ABI_VERSION,
        }
    }

    /// Constructs the declared production executor.
    #[must_use]
    pub const fn declared() -> Self {
        Self::new(ResourceBudget::declared(), FeeSchedule::declared())
    }

    /// Executes a validated module under a fresh store and exact resource budget.
    ///
    /// A failed call returns no instance, output or guest mutation, providing the
    /// rollback boundary consumed by the programs module transition.
    ///
    /// # Errors
    ///
    /// Returns a typed guest fault or resource refusal.
    pub fn execute(
        &self,
        module: &ValidatedModule,
        export: &str,
        args: &[WasmValue],
    ) -> Result<ExecutionRecord, ExecutionError> {
        if module.abi_revision() != AbiRevision::V1 {
            return Err(ExecutionError::Abi(AbiError::WrongVersion));
        }
        let meter = Meter::new(self.budget, self.prices);
        let mut instance = module
            .instantiate_metered(meter)
            .map_err(|(fault, exhausted)| self.classify_fault(fault, exhausted))?;
        let outputs = match instance.call(export, args) {
            Ok(outputs) => outputs,
            Err(fault) => {
                return Err(self.classify_fault(fault, instance.meter().exhaustion()));
            }
        };
        let usage = instance
            .meter()
            .finish()
            .map_err(ExecutionError::Resource)?;
        Ok(ExecutionRecord {
            runtime_version: self.runtime_version,
            abi_version: self.abi_version,
            outputs,
            usage,
        })
    }

    /// Executes a program with an explicit authorization context and atomic
    /// namespaced storage. Durable storage changes only after guest success and
    /// successful resource finalization.
    ///
    /// # Errors
    ///
    /// Returns typed ABI, guest, or resource refusals without committing
    /// storage or exposing partial effects.
    pub fn execute_authorized(
        &self,
        storage: &mut Storage,
        request: AuthorizedExecutionRequest<'_>,
    ) -> Result<AuthorizedExecutionRecord, ExecutionError> {
        if request.module.abi_revision() != AbiRevision::V1 {
            return Err(ExecutionError::Abi(AbiError::WrongVersion));
        }
        entrypoint::preflight(request.calldata).map_err(ExecutionError::Entrypoint)?;
        let meter = Meter::new(self.budget, self.prices);
        let principal = request.authorization.principal();
        let abi = Abi::new(
            self.abi_version,
            request.program,
            request.authorization,
            storage.clone(),
            request.receipts,
        )
        .map_err(ExecutionError::Abi)?;
        let composition = Composition::new(
            request.composition.resolver(),
            CallGraph::root(request.composition.rules(), request.program, principal),
            AbiRevision::V1,
        );
        let mut instance = request
            .module
            .instantiate_composed(meter, abi, composition)
            .map_err(|(fault, exhausted)| self.classify_fault(fault, exhausted))?;
        let code = match entrypoint::invoke(&mut instance, request.entrypoint, request.calldata) {
            Ok(code) => code,
            Err(EntrypointRefusal::Fault(fault)) => {
                if let Some(refusal) = instance.state().refusal() {
                    return Err(ExecutionError::Composition(refusal.clone()));
                }
                return Err(self.classify_fault(fault, instance.meter().exhaustion()));
            }
            Err(EntrypointRefusal::Resource(MeterRefusal::BudgetExceeded {
                resource: ResourceKind::Cpu,
                ..
            })) => return Err(self.classify_fault(ExecutionFault::OutOfFuel, None)),
            Err(EntrypointRefusal::Resource(refusal)) => {
                return Err(ExecutionError::Resource(refusal));
            }
            Err(refusal) => return Err(ExecutionError::Entrypoint(refusal)),
        };
        let usage = instance
            .meter()
            .finish()
            .map_err(ExecutionError::Resource)?;
        let (_, abi, composition) = instance.into_state().into_parts();
        let committed = abi
            .ok_or(ExecutionError::Abi(AbiError::CapabilityDenied))?
            .commit();
        let call_graph = composition
            .ok_or(ExecutionError::Composition(
                CompositionRefusal::NotComposable,
            ))?
            .into_graph();
        *storage = committed.storage;
        Ok(AuthorizedExecutionRecord {
            execution: ExecutionRecord {
                runtime_version: self.runtime_version,
                abi_version: self.abi_version,
                outputs: vec![WasmValue::I32(code)],
                usage,
            },
            effects: committed.effects,
            call_graph,
        })
    }

    /// Executes an authorized activity through the explicitly selected candidate ABI.
    ///
    /// # Errors
    ///
    /// Returns typed validation, execution, composition, response, or resource refusals.
    #[allow(clippy::too_many_lines)]
    pub fn execute_authorized_candidate(
        &self,
        storage: &mut Storage,
        request: AuthorizedExecutionRequest<'_>,
    ) -> Result<CandidateAuthorizedExecutionRecord, ExecutionError> {
        if request.module.abi_revision() != AbiRevision::CandidateV2 {
            return Err(ExecutionError::Abi(AbiError::WrongVersion));
        }
        if request.response_capacity > crate::abi::response::MAX_CALL_RESPONSE_BYTES {
            return Err(ExecutionError::Response(ResponseRefusal::TooLarge {
                bytes: request.response_capacity,
                limit: crate::abi::response::MAX_CALL_RESPONSE_BYTES,
            }));
        }
        entrypoint::preflight(request.calldata).map_err(ExecutionError::Entrypoint)?;
        let meter = Meter::new(self.budget, self.prices);
        let principal = request.authorization.principal();
        let abi = Abi::new(
            self.abi_version,
            request.program,
            request.authorization,
            storage.clone(),
            request.receipts,
        )
        .map_err(ExecutionError::Abi)?;
        let composition = Composition::new(
            request.composition.resolver(),
            CallGraph::root(request.composition.rules(), request.program, principal),
            AbiRevision::CandidateV2,
        );
        let retained = request
            .module
            .instantiate_composed_response_retained(
                meter,
                abi,
                composition,
                request.response_capacity,
            )
            .map_err(ExecutionError::Response)?;
        let mut instance = match retained {
            Ok(instance) => instance,
            Err(error) => {
                let (fault, state) = *error;
                return self.finish_candidate_start(request.program, fault, state);
            }
        };
        let (code, failure) =
            match entrypoint::invoke(&mut instance, request.entrypoint, request.calldata) {
                Ok(code) => {
                    if let Some(refusal) = instance.state().refusal() {
                        return Err(ExecutionError::Composition(refusal.clone()));
                    }
                    if instance.state().failure().is_some() {
                        return Err(ExecutionError::Response(ResponseRefusal::CodeMismatch {
                            published: CANDIDATE_REFUSAL_SENTINEL,
                            returned: code,
                        }));
                    }
                    (code, None)
                }
                Err(EntrypointRefusal::GuestRefused { code }) => {
                    if let Some(refusal) = instance.state().refusal() {
                        return Err(ExecutionError::Composition(refusal.clone()));
                    }
                    let failure = match instance.state().failure().cloned() {
                        Some(failure) if code == CANDIDATE_REFUSAL_SENTINEL => failure,
                        Some(_) => {
                            return Err(ExecutionError::Response(ResponseRefusal::CodeMismatch {
                                published: CANDIDATE_REFUSAL_SENTINEL,
                                returned: code,
                            }));
                        }
                        None if code == CANDIDATE_REFUSAL_SENTINEL => {
                            return Err(ExecutionError::Response(
                                ResponseRefusal::InvalidPublication,
                            ));
                        }
                        None => ProgramFailure::authenticated(
                            request.program,
                            RefusalClass::Legacy,
                            RefusalReason::empty(),
                        ),
                    };
                    (code, Some(failure))
                }
                Err(EntrypointRefusal::Fault(fault)) => {
                    if let Some(refusal) = instance.state().refusal() {
                        if let CompositionRefusal::Program(failure) = refusal {
                            (
                                crate::fault::CANDIDATE_REFUSAL_SENTINEL,
                                Some(failure.clone()),
                            )
                        } else {
                            return Err(ExecutionError::Composition(refusal.clone()));
                        }
                    } else if let Some(failure) = instance.state().failure().cloned() {
                        (crate::fault::CANDIDATE_REFUSAL_SENTINEL, Some(failure))
                    } else if is_candidate_runtime_fault(&fault) {
                        (
                            crate::fault::CANDIDATE_REFUSAL_SENTINEL,
                            Some(ProgramFailure::authenticated(
                                request.program,
                                crate::fault::RefusalClass::RuntimeFault,
                                crate::fault::RefusalReason::empty(),
                            )),
                        )
                    } else {
                        return Err(self.classify_fault(fault, instance.meter().exhaustion()));
                    }
                }
                Err(EntrypointRefusal::Resource(refusal)) => {
                    if let Some(CompositionRefusal::Program(failure)) = instance.state().refusal() {
                        (CANDIDATE_REFUSAL_SENTINEL, Some(failure.clone()))
                    } else if let Some(failure) = instance.state().failure().cloned() {
                        (CANDIDATE_REFUSAL_SENTINEL, Some(failure))
                    } else {
                        return Err(ExecutionError::Resource(refusal));
                    }
                }
                Err(refusal) => return Err(ExecutionError::Entrypoint(refusal)),
            };
        if let Some(failure) = failure {
            let usage = instance
                .meter()
                .finish_published_failure()
                .map_err(ExecutionError::Resource)?;
            let mut state = instance.into_state();
            let failure_graph = state.take_failure_graph();
            let (_, _, composition) = state.into_parts();
            let call_graph = failure_graph
                .or_else(|| composition.map(Composition::into_graph))
                .ok_or(ExecutionError::Composition(
                    CompositionRefusal::NotComposable,
                ))?;
            return Ok(CandidateAuthorizedExecutionRecord {
                root_program: request.program,
                abi_revision: AbiRevision::CandidateV2,
                execution: CandidateExecutionRecord {
                    runtime_version: self.runtime_version,
                    outputs: vec![WasmValue::I32(code)],
                    usage,
                },
                outcome: CandidateActivityOutcome::Failure(failure),
                call_graph,
            });
        }
        let response = match instance.state().finalize_response(code) {
            Ok(response) => response,
            Err(ResponseRefusal::Meter(refusal)) => return Err(ExecutionError::Resource(refusal)),
            Err(refusal) => return Err(ExecutionError::Response(refusal)),
        };
        let usage = instance
            .meter()
            .finish()
            .map_err(ExecutionError::Resource)?;
        let (_, abi, composition) = instance.into_state().into_parts();
        let committed = abi
            .ok_or(ExecutionError::Abi(AbiError::CapabilityDenied))?
            .commit();
        let call_graph = composition
            .ok_or(ExecutionError::Composition(
                CompositionRefusal::NotComposable,
            ))?
            .into_graph();
        *storage = committed.storage;
        Ok(CandidateAuthorizedExecutionRecord {
            root_program: request.program,
            abi_revision: AbiRevision::CandidateV2,
            execution: CandidateExecutionRecord {
                runtime_version: self.runtime_version,
                outputs: vec![WasmValue::I32(code)],
                usage,
            },
            outcome: CandidateActivityOutcome::Success {
                response,
                effects: committed.effects,
            },
            call_graph,
        })
    }

    fn finish_candidate_start(
        &self,
        program: ProgramId,
        fault: ExecutionFault,
        state: RuntimeState,
    ) -> Result<CandidateAuthorizedExecutionRecord, ExecutionError> {
        if let Some(refusal) = state.refusal() {
            if let CompositionRefusal::Program(failure) = refusal {
                return self.candidate_failure_from_state(program, failure.clone(), state);
            }
            return Err(ExecutionError::Composition(refusal.clone()));
        }
        let failure = state.failure().cloned().unwrap_or_else(|| {
            ProgramFailure::authenticated(
                program,
                RefusalClass::RuntimeFault,
                RefusalReason::empty(),
            )
        });
        if !is_candidate_runtime_fault(&fault) && state.failure().is_none() {
            return Err(self.classify_fault(fault, state.meter().exhaustion()));
        }
        self.candidate_failure_from_state(program, failure, state)
    }

    fn candidate_failure_from_state(
        &self,
        program: ProgramId,
        failure: ProgramFailure,
        mut state: RuntimeState,
    ) -> Result<CandidateAuthorizedExecutionRecord, ExecutionError> {
        let usage = state
            .meter()
            .finish_published_failure()
            .map_err(ExecutionError::Resource)?;
        let failure_graph = state.take_failure_graph();
        let (_, _, composition) = state.into_parts();
        let call_graph = failure_graph
            .or_else(|| composition.map(Composition::into_graph))
            .ok_or(ExecutionError::Composition(
                CompositionRefusal::NotComposable,
            ))?;
        Ok(CandidateAuthorizedExecutionRecord {
            root_program: program,
            abi_revision: AbiRevision::CandidateV2,
            execution: CandidateExecutionRecord {
                runtime_version: self.runtime_version,
                outputs: vec![WasmValue::I32(CANDIDATE_REFUSAL_SENTINEL)],
                usage,
            },
            outcome: CandidateActivityOutcome::Failure(failure),
            call_graph,
        })
    }

    fn classify_fault(
        &self,
        fault: ExecutionFault,
        exhausted: Option<MeterRefusal>,
    ) -> ExecutionError {
        if let Some(refusal) = exhausted {
            return ExecutionError::Resource(refusal);
        }
        match fault {
            ExecutionFault::Resource { refusal } => ExecutionError::Resource(refusal),
            ExecutionFault::OutOfFuel => ExecutionError::Resource(MeterRefusal::BudgetExceeded {
                resource: ResourceKind::Cpu,
                limit: self.budget.cpu_fuel(),
                attempted: self.budget.cpu_fuel().saturating_add(1),
            }),
            ExecutionFault::GrowthLimited => {
                ExecutionError::Resource(MeterRefusal::BudgetExceeded {
                    resource: ResourceKind::Memory,
                    limit: self.budget.memory_bytes(),
                    attempted: self.budget.memory_bytes().saturating_add(1),
                })
            }
            other => ExecutionError::Fault(other),
        }
    }
}

fn is_candidate_runtime_fault(fault: &ExecutionFault) -> bool {
    !matches!(
        fault,
        ExecutionFault::EngineFault { .. }
            | ExecutionFault::UnknownExport { .. }
            | ExecutionFault::NotAFunction { .. }
            | ExecutionFault::OutOfFuel
            | ExecutionFault::GrowthLimited
            | ExecutionFault::Resource { .. }
    )
}

impl Default for Executor {
    fn default() -> Self {
        Self::declared()
    }
}
