//! Typed execution surface over instantiated deterministic programs.

use core::fmt::{self, Display};

use wasmi::core::TrapCode;
use wasmi::{Extern, Instance, Memory, Store, Value};

use crate::abi::response::{CallResponse, ResponseRefusal};
use crate::abi::{Abi, AbiEffects, AbiError, AuthorizationContext, ReceiptOracle};
use crate::calls::{CallGraph, Composition, CompositionContext, CompositionRefusal};
use crate::entrypoint::{self, EntrypointRefusal};
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
    pub execution: CandidateExecutionRecord,
    pub response: CallResponse,
    pub effects: AbiEffects,
    pub call_graph: CallGraph,
}

/// Execution facts that cannot be confused with frozen v1 receipt evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateExecutionRecord {
    pub runtime_version: u16,
    pub outputs: Vec<WasmValue>,
    pub usage: MeteredUsage,
}

impl CandidateAuthorizedExecutionRecord {
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
        evidence.extend_from_slice(&self.response.code.to_be_bytes());
        evidence.extend_from_slice(&(self.response.bytes.len() as u64).to_be_bytes());
        evidence.extend_from_slice(&self.response.bytes);
        evidence
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
        let mut instance = request
            .module
            .instantiate_composed_response(meter, abi, composition, request.response_capacity)
            .map_err(ExecutionError::Response)?
            .map_err(|(fault, exhausted)| self.classify_fault(fault, exhausted))?;
        let code = match entrypoint::invoke(&mut instance, request.entrypoint, request.calldata) {
            Ok(code) => code,
            Err(EntrypointRefusal::Fault(fault)) => {
                if let Some(refusal) = instance.state().refusal() {
                    return Err(ExecutionError::Composition(refusal.clone()));
                }
                return Err(self.classify_fault(fault, instance.meter().exhaustion()));
            }
            Err(EntrypointRefusal::Resource(refusal)) => {
                return Err(ExecutionError::Resource(refusal))
            }
            Err(refusal) => return Err(ExecutionError::Entrypoint(refusal)),
        };
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
            execution: CandidateExecutionRecord {
                runtime_version: self.runtime_version,
                outputs: vec![WasmValue::I32(code)],
                usage,
            },
            response,
            effects: committed.effects,
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

impl Default for Executor {
    fn default() -> Self {
        Self::declared()
    }
}
