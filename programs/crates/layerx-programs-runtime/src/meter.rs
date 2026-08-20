//! Deterministic resource accounting for guest execution.

use core::fmt::{self, Display};

use wasmi::errors::{MemoryError, TableError};
use wasmi::ResourceLimiter;

/// Default instruction fuel admitted for one execution.
pub const DEFAULT_CPU_FUEL: u64 = 1_000_000;
/// Default peak linear-memory budget in bytes.
pub const DEFAULT_MEMORY_BYTES: u64 = 16 * 1_024 * 1_024;
/// Default storage-read budget in bytes.
pub const DEFAULT_STORAGE_READ_BYTES: u64 = 1_048_576;
/// Default storage-write budget in bytes.
pub const DEFAULT_STORAGE_WRITE_BYTES: u64 = 1_048_576;
/// Default result-value count admitted at the guest boundary.
pub const DEFAULT_OUTPUT_VALUES: u32 = 64;
/// Default table-element limit per execution.
pub const DEFAULT_TABLE_ELEMENTS: u32 = 4_096;

/// One independently enforced deterministic resource class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    /// Interpreter instruction fuel.
    Cpu,
    /// Peak guest linear memory.
    Memory,
    /// Bytes read through the storage ABI.
    StorageRead,
    /// Bytes written through the storage ABI.
    StorageWrite,
    /// Integer values returned across the guest boundary.
    Output,
}

impl Display for ResourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu => write!(formatter, "cpu fuel"),
            Self::Memory => write!(formatter, "memory bytes"),
            Self::StorageRead => write!(formatter, "storage read bytes"),
            Self::StorageWrite => write!(formatter, "storage write bytes"),
            Self::Output => write!(formatter, "output values"),
        }
    }
}

/// Exact resource budget applied to one fresh execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBudget {
    cpu_fuel: u64,
    memory_bytes: u64,
    storage_read_bytes: u64,
    storage_write_bytes: u64,
    output_values: u32,
    table_elements: u32,
}

impl ResourceBudget {
    /// Constructs an explicit integer-only execution budget.
    #[must_use]
    pub const fn new(
        cpu_fuel: u64,
        memory_bytes: u64,
        storage_read_bytes: u64,
        storage_write_bytes: u64,
        output_values: u32,
        table_elements: u32,
    ) -> Self {
        Self {
            cpu_fuel,
            memory_bytes,
            storage_read_bytes,
            storage_write_bytes,
            output_values,
            table_elements,
        }
    }

    /// Returns the declared production budget.
    #[must_use]
    pub const fn declared() -> Self {
        Self::new(
            DEFAULT_CPU_FUEL,
            DEFAULT_MEMORY_BYTES,
            DEFAULT_STORAGE_READ_BYTES,
            DEFAULT_STORAGE_WRITE_BYTES,
            DEFAULT_OUTPUT_VALUES,
            DEFAULT_TABLE_ELEMENTS,
        )
    }

    /// Returns the instruction-fuel limit.
    #[must_use]
    pub const fn cpu_fuel(self) -> u64 {
        self.cpu_fuel
    }

    /// Returns the peak linear-memory limit.
    #[must_use]
    pub const fn memory_bytes(self) -> u64 {
        self.memory_bytes
    }

    /// Returns the maximum result-value count.
    #[must_use]
    pub const fn output_values(self) -> u32 {
        self.output_values
    }
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self::declared()
    }
}

/// Integer fee-unit prices for each metered resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeeSchedule {
    cpu: u64,
    memory_byte: u64,
    storage_read_byte: u64,
    storage_write_byte: u64,
    output_value: u64,
}

impl FeeSchedule {
    /// Constructs an explicit integer fee schedule.
    #[must_use]
    pub const fn new(
        cpu: u64,
        memory_byte: u64,
        storage_read_byte: u64,
        storage_write_byte: u64,
        output_value: u64,
    ) -> Self {
        Self {
            cpu,
            memory_byte,
            storage_read_byte,
            storage_write_byte,
            output_value,
        }
    }

    /// Returns the runtime's version-one fee-unit schedule.
    #[must_use]
    pub const fn declared() -> Self {
        Self::new(1, 1, 2, 4, 1)
    }
}

impl Default for FeeSchedule {
    fn default() -> Self {
        Self::declared()
    }
}

/// Exact deterministic usage and fee units produced by one execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeteredUsage {
    /// Interpreter fuel consumed.
    pub cpu_fuel: u64,
    /// Peak guest memory admitted in bytes.
    pub memory_bytes: u64,
    /// Storage bytes read.
    pub storage_read_bytes: u64,
    /// Storage bytes written.
    pub storage_write_bytes: u64,
    /// Integer result values returned.
    pub output_values: u32,
    /// Exact units handed to the existing fee mechanism.
    pub fee_units: u128,
}

/// Typed resource refusal with exact limit and attempted use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterRefusal {
    /// One resource budget was exceeded.
    BudgetExceeded {
        /// Resource whose budget was exceeded.
        resource: ResourceKind,
        /// Configured resource limit.
        limit: u64,
        /// Attempted cumulative or peak use.
        attempted: u64,
    },
    /// Fee-unit multiplication or accumulation exceeded `u128`.
    FeeOverflow,
}

impl Display for MeterRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BudgetExceeded {
                resource,
                limit,
                attempted,
            } => write!(
                formatter,
                "{resource} budget {limit} exceeded by attempted use {attempted}"
            ),
            Self::FeeOverflow => write!(formatter, "metered fee units overflowed"),
        }
    }
}

impl std::error::Error for MeterRefusal {}

/// Per-execution deterministic meter and guest resource limiter.
#[derive(Debug, Clone)]
pub struct Meter {
    budget: ResourceBudget,
    prices: FeeSchedule,
    cpu_fuel: u64,
    cpu_carried: u64,
    memory_bytes: u64,
    storage_read_bytes: u64,
    storage_write_bytes: u64,
    output_values: u32,
    exhausted: Option<MeterRefusal>,
}

impl Meter {
    /// Creates a fresh meter for one execution.
    #[must_use]
    pub const fn new(budget: ResourceBudget, prices: FeeSchedule) -> Self {
        Self {
            budget,
            prices,
            cpu_fuel: 0,
            cpu_carried: 0,
            memory_bytes: 0,
            storage_read_bytes: 0,
            storage_write_bytes: 0,
            output_values: 0,
            exhausted: None,
        }
    }

    /// Creates a fresh meter under the declared production budget and prices.
    #[must_use]
    pub const fn declared() -> Self {
        Self::new(ResourceBudget::declared(), FeeSchedule::declared())
    }

    /// Returns the declared instruction-fuel budget of the whole call graph.
    #[must_use]
    pub const fn cpu_budget(&self) -> u64 {
        self.budget.cpu_fuel
    }

    /// Returns the fuel already consumed by frames outside the store this
    /// meter is about to drive.
    #[must_use]
    pub const fn cpu_carried(&self) -> u64 {
        self.cpu_carried
    }

    /// Returns the fuel to install into the interpreter store of the next
    /// frame, so a whole call graph never exceeds one declared budget.
    #[must_use]
    pub const fn cpu_remaining(&self) -> u64 {
        self.budget.cpu_fuel.saturating_sub(self.cpu_carried)
    }

    /// Returns the fuel attributed to this meter by the last recorded frame.
    #[must_use]
    pub const fn cpu_total(&self) -> u64 {
        self.cpu_fuel
    }

    pub(crate) fn carry_cpu(&mut self, consumed: u64) -> Result<(), MeterRefusal> {
        let attempted = self.cpu_carried.saturating_add(consumed);
        self.admit(ResourceKind::Cpu, self.budget.cpu_fuel, attempted)?;
        self.cpu_carried = attempted;
        Ok(())
    }

    pub(crate) fn restore_cpu_carry(&mut self, carried: u64) {
        self.cpu_carried = carried;
        self.cpu_fuel = 0;
    }

    /// Charges bytes read through the future versioned storage ABI.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal before the read when its cumulative budget is exceeded.
    pub fn charge_storage_read(&mut self, bytes: u64) -> Result<(), MeterRefusal> {
        let attempted = self.storage_read_bytes.saturating_add(bytes);
        self.admit(
            ResourceKind::StorageRead,
            self.budget.storage_read_bytes,
            attempted,
        )?;
        self.storage_read_bytes = attempted;
        Ok(())
    }

    /// Charges bytes written through the future versioned storage ABI.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal before the write when its cumulative budget is exceeded.
    pub fn charge_storage_write(&mut self, bytes: u64) -> Result<(), MeterRefusal> {
        let attempted = self.storage_write_bytes.saturating_add(bytes);
        self.admit(
            ResourceKind::StorageWrite,
            self.budget.storage_write_bytes,
            attempted,
        )?;
        self.storage_write_bytes = attempted;
        Ok(())
    }

    pub(crate) fn record_cpu(&mut self, consumed: u64) {
        self.cpu_fuel = self.cpu_carried.saturating_add(consumed);
    }

    pub(crate) fn mark_cpu_exhausted(&mut self) {
        self.exhausted = Some(MeterRefusal::BudgetExceeded {
            resource: ResourceKind::Cpu,
            limit: self.budget.cpu_fuel,
            attempted: self.cpu_fuel.saturating_add(1),
        });
    }

    pub(crate) fn charge_output(&mut self, values: usize) -> Result<(), MeterRefusal> {
        let requested = u64::try_from(values).unwrap_or(u64::MAX);
        let attempted = u64::from(self.output_values).saturating_add(requested);
        self.admit(
            ResourceKind::Output,
            u64::from(self.budget.output_values),
            attempted,
        )?;
        self.output_values =
            u32::try_from(attempted).map_err(|_| MeterRefusal::BudgetExceeded {
                resource: ResourceKind::Output,
                limit: u64::from(self.budget.output_values),
                attempted,
            })?;
        Ok(())
    }

    pub(crate) const fn exhaustion(&self) -> Option<MeterRefusal> {
        self.exhausted
    }

    /// Finalises exact usage and fee units after guest execution.
    ///
    /// # Errors
    ///
    /// Returns a prior resource refusal or fee overflow.
    pub fn finish(&self) -> Result<MeteredUsage, MeterRefusal> {
        if let Some(refusal) = self.exhausted {
            return Err(refusal);
        }
        let priced = [
            (u128::from(self.cpu_fuel), self.prices.cpu),
            (u128::from(self.memory_bytes), self.prices.memory_byte),
            (
                u128::from(self.storage_read_bytes),
                self.prices.storage_read_byte,
            ),
            (
                u128::from(self.storage_write_bytes),
                self.prices.storage_write_byte,
            ),
            (u128::from(self.output_values), self.prices.output_value),
        ];
        let mut fee_units = 0u128;
        for (use_units, price) in priced {
            fee_units = fee_units
                .checked_add(
                    use_units
                        .checked_mul(u128::from(price))
                        .ok_or(MeterRefusal::FeeOverflow)?,
                )
                .ok_or(MeterRefusal::FeeOverflow)?;
        }
        Ok(MeteredUsage {
            cpu_fuel: self.cpu_fuel,
            memory_bytes: self.memory_bytes,
            storage_read_bytes: self.storage_read_bytes,
            storage_write_bytes: self.storage_write_bytes,
            output_values: self.output_values,
            fee_units,
        })
    }

    fn admit(
        &mut self,
        resource: ResourceKind,
        limit: u64,
        attempted: u64,
    ) -> Result<(), MeterRefusal> {
        if attempted <= limit {
            return Ok(());
        }
        let refusal = MeterRefusal::BudgetExceeded {
            resource,
            limit,
            attempted,
        };
        self.exhausted = Some(refusal);
        Err(refusal)
    }
}

impl Default for Meter {
    fn default() -> Self {
        Self::declared()
    }
}

impl ResourceLimiter for Meter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<bool, MemoryError> {
        if maximum.is_some_and(|maximum| desired > maximum) {
            return Ok(false);
        }
        let attempted = u64::try_from(desired).unwrap_or(u64::MAX);
        if self
            .admit(ResourceKind::Memory, self.budget.memory_bytes, attempted)
            .is_err()
        {
            return Err(MemoryError::OutOfBoundsGrowth);
        }
        self.memory_bytes = self.memory_bytes.max(attempted);
        Ok(true)
    }

    fn table_growing(
        &mut self,
        current: u32,
        desired: u32,
        maximum: Option<u32>,
    ) -> Result<bool, TableError> {
        let limit = maximum
            .unwrap_or(self.budget.table_elements)
            .min(self.budget.table_elements);
        if desired <= limit {
            return Ok(true);
        }
        self.exhausted = Some(MeterRefusal::BudgetExceeded {
            resource: ResourceKind::Memory,
            limit: u64::from(limit),
            attempted: u64::from(desired),
        });
        Err(TableError::GrowOutOfBounds {
            maximum: limit,
            current,
            delta: desired.saturating_sub(current),
        })
    }

    fn instances(&self) -> usize {
        1
    }

    fn tables(&self) -> usize {
        1
    }

    fn memories(&self) -> usize {
        1
    }
}
