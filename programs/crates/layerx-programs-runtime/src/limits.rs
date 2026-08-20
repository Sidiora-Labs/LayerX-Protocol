//! Declared module validation limits with typed refusals.

use core::fmt::{self, Display};

/// The declared upper bound on the byte size of a deployable module.
pub const DEFAULT_MAX_MODULE_BYTES: u64 = 1_048_576;

/// The declared upper bound on the number of functions a module may define.
pub const DEFAULT_MAX_FUNCTIONS: u32 = 4_096;

/// The declared upper bound on the value stack height during execution.
pub const DEFAULT_MAX_VALUE_STACK_HEIGHT: u32 = 65_536;

/// The declared upper bound on the depth of nested calls during execution.
pub const DEFAULT_MAX_CALL_DEPTH: u32 = 512;

/// Names one declared limit inside a typed refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredLimit {
    /// The module byte-size limit.
    ModuleBytes,
    /// The function-count limit.
    Functions,
    /// The value stack height limit.
    ValueStackHeight,
    /// The call depth limit.
    CallDepth,
}

impl Display for DeclaredLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModuleBytes => write!(f, "module byte size"),
            Self::Functions => write!(f, "function count"),
            Self::ValueStackHeight => write!(f, "value stack height"),
            Self::CallDepth => write!(f, "call depth"),
        }
    }
}

/// A typed refusal produced while constructing [`ValidationLimits`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitsRefusal {
    /// A declared limit was configured as zero, which would refuse every module.
    ZeroLimit {
        /// The limit that was configured as zero.
        limit: DeclaredLimit,
    },
}

impl Display for LimitsRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLimit { limit } => {
                write!(f, "declared {limit} limit must not be zero")
            }
        }
    }
}

impl std::error::Error for LimitsRefusal {}

/// The declared module validation limits, bounds enforced at construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationLimits {
    module_bytes: u64,
    functions: u32,
    value_stack_height: u32,
    call_depth: u32,
}

impl ValidationLimits {
    /// Constructs validation limits, refusing any zero bound.
    ///
    /// # Errors
    ///
    /// Returns [`LimitsRefusal::ZeroLimit`] naming the offending limit when any
    /// bound is zero.
    pub const fn new(
        max_module_bytes: u64,
        max_functions: u32,
        max_value_stack_height: u32,
        max_call_depth: u32,
    ) -> Result<Self, LimitsRefusal> {
        if max_module_bytes == 0 {
            return Err(LimitsRefusal::ZeroLimit {
                limit: DeclaredLimit::ModuleBytes,
            });
        }
        if max_functions == 0 {
            return Err(LimitsRefusal::ZeroLimit {
                limit: DeclaredLimit::Functions,
            });
        }
        if max_value_stack_height == 0 {
            return Err(LimitsRefusal::ZeroLimit {
                limit: DeclaredLimit::ValueStackHeight,
            });
        }
        if max_call_depth == 0 {
            return Err(LimitsRefusal::ZeroLimit {
                limit: DeclaredLimit::CallDepth,
            });
        }
        Ok(Self {
            module_bytes: max_module_bytes,
            functions: max_functions,
            value_stack_height: max_value_stack_height,
            call_depth: max_call_depth,
        })
    }

    /// Returns the declared default limits of the programs runtime.
    #[must_use]
    pub const fn declared() -> Self {
        Self {
            module_bytes: DEFAULT_MAX_MODULE_BYTES,
            functions: DEFAULT_MAX_FUNCTIONS,
            value_stack_height: DEFAULT_MAX_VALUE_STACK_HEIGHT,
            call_depth: DEFAULT_MAX_CALL_DEPTH,
        }
    }

    /// Returns the declared upper bound on the module byte size.
    #[must_use]
    pub const fn max_module_bytes(&self) -> u64 {
        self.module_bytes
    }

    /// Returns the declared upper bound on the number of functions.
    #[must_use]
    pub const fn max_functions(&self) -> u32 {
        self.functions
    }

    /// Returns the declared upper bound on the value stack height.
    #[must_use]
    pub const fn max_value_stack_height(&self) -> u32 {
        self.value_stack_height
    }

    /// Returns the declared upper bound on the call depth.
    #[must_use]
    pub const fn max_call_depth(&self) -> u32 {
        self.call_depth
    }
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self::declared()
    }
}
