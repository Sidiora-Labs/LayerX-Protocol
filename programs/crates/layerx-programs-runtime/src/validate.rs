//! Module validation against the deterministic subset and the declared limits.

use core::fmt::{self, Display};

use wasmparser_nostd::{BlockType, FunctionBody, Import, Operator, Parser, Payload, Type, ValType};

use crate::engine::WasmEngine;
use crate::execute::{fault_from_error, ExecutionFault, ProgramInstance};

const PERMITTED_IMPORTS: &[(&str, &str)] = &[];

/// A typed refusal produced while validating a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationRefusal {
    /// The module exceeds the declared byte-size limit.
    ModuleTooLarge {
        /// The byte size of the refused module.
        byte_size: u64,
        /// The declared byte-size limit.
        limit: u64,
    },
    /// The module declares more functions than the declared limit allows.
    TooManyFunctions {
        /// The number of functions the module declares.
        function_count: u32,
        /// The declared function-count limit.
        limit: u32,
    },
    /// The module imports a host item outside the declared deterministic ABI.
    ForbiddenImport {
        /// The module field of the refused import.
        import_module: String,
        /// The name field of the refused import.
        import_name: String,
    },
    /// The module declares a floating-point type.
    ForbiddenFloatType,
    /// The module contains a floating-point instruction.
    ForbiddenFloatInstruction,
    /// The module declares a vector type.
    ForbiddenVectorType,
    /// The module bytes are not a well-formed WASM module.
    MalformedModule {
        /// The parser's reason for refusing the bytes.
        reason: String,
    },
    /// The stripped engine refused the module during validation.
    RejectedByEngine {
        /// The engine's reason for refusing the module.
        reason: String,
    },
}

impl Display for ValidationRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModuleTooLarge { byte_size, limit } => {
                write!(f, "module of {byte_size} bytes exceeds declared limit {limit}")
            }
            Self::TooManyFunctions {
                function_count,
                limit,
            } => {
                write!(
                    f,
                    "module declares {function_count} functions exceeding declared limit {limit}"
                )
            }
            Self::ForbiddenImport {
                import_module,
                import_name,
            } => {
                write!(f, "forbidden import {import_module}::{import_name}")
            }
            Self::ForbiddenFloatType => write!(f, "floating-point types are forbidden"),
            Self::ForbiddenFloatInstruction => {
                write!(f, "floating-point instructions are forbidden")
            }
            Self::ForbiddenVectorType => write!(f, "vector types are forbidden"),
            Self::MalformedModule { reason } => write!(f, "malformed module: {reason}"),
            Self::RejectedByEngine { reason } => write!(f, "rejected by engine: {reason}"),
        }
    }
}

impl std::error::Error for ValidationRefusal {}

/// A module accepted by deterministic-subset validation under declared limits.
#[derive(Debug)]
pub struct ValidatedModule {
    module: wasmi::Module,
    byte_size: u64,
    function_count: u32,
}

impl ValidatedModule {
    /// Returns the byte size of the validated module.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the number of functions the validated module declares.
    #[must_use]
    pub const fn function_count(&self) -> u32 {
        self.function_count
    }

    /// Instantiates the validated module in an isolated store.
    ///
    /// # Errors
    ///
    /// Returns a typed [`ExecutionFault`] when instantiation fails or the
    /// module's start function traps.
    pub fn instantiate(&self) -> Result<ProgramInstance, ExecutionFault> {
        let mut store = wasmi::Store::new(self.module.engine(), ());
        let linker: wasmi::Linker<()> = wasmi::Linker::new(self.module.engine());
        let instance = linker
            .instantiate(&mut store, &self.module)
            .map_err(|error| fault_from_error(&error))?
            .start(&mut store)
            .map_err(|error| fault_from_error(&error))?;
        Ok(ProgramInstance::new(store, instance))
    }
}

pub(crate) fn validate_module(
    engine: &WasmEngine,
    wasm: &[u8],
) -> Result<ValidatedModule, ValidationRefusal> {
    let limits = engine.limits();
    let byte_size = wasm.len() as u64;
    if byte_size > limits.max_module_bytes() {
        return Err(ValidationRefusal::ModuleTooLarge {
            byte_size,
            limit: limits.max_module_bytes(),
        });
    }
    let mut function_count: u32 = 0;
    for payload in Parser::new(0).parse_all(wasm) {
        let payload = payload.map_err(|error| ValidationRefusal::MalformedModule {
            reason: error.to_string(),
        })?;
        match payload {
            Payload::TypeSection(reader) => {
                for entry in reader {
                    let entry = entry.map_err(|error| ValidationRefusal::MalformedModule {
                        reason: error.to_string(),
                    })?;
                    let Type::Func(func_type) = entry;
                    for value_type in func_type.params().iter().chain(func_type.results()) {
                        refuse_value_type(*value_type)?;
                    }
                }
            }
            Payload::ImportSection(reader) => {
                for entry in reader {
                    let entry = entry.map_err(|error| ValidationRefusal::MalformedModule {
                        reason: error.to_string(),
                    })?;
                    refuse_import(&entry)?;
                }
            }
            Payload::FunctionSection(reader) => {
                function_count = reader.count();
                if function_count > limits.max_functions() {
                    return Err(ValidationRefusal::TooManyFunctions {
                        function_count,
                        limit: limits.max_functions(),
                    });
                }
            }
            Payload::GlobalSection(reader) => {
                for entry in reader {
                    let entry = entry.map_err(|error| ValidationRefusal::MalformedModule {
                        reason: error.to_string(),
                    })?;
                    refuse_value_type(entry.ty.content_type)?;
                }
            }
            Payload::CodeSectionEntry(body) => {
                refuse_float_code(&body)?;
            }
            _ => {}
        }
    }
    let module = wasmi::Module::new(engine.inner(), wasm).map_err(|error| {
        ValidationRefusal::RejectedByEngine {
            reason: error.to_string(),
        }
    })?;
    Ok(ValidatedModule {
        module,
        byte_size,
        function_count,
    })
}

fn refuse_import(import: &Import<'_>) -> Result<(), ValidationRefusal> {
    let permitted = PERMITTED_IMPORTS
        .iter()
        .any(|(module, name)| *module == import.module && *name == import.name);
    if permitted {
        Ok(())
    } else {
        Err(ValidationRefusal::ForbiddenImport {
            import_module: import.module.to_string(),
            import_name: import.name.to_string(),
        })
    }
}

fn refuse_value_type(value_type: ValType) -> Result<(), ValidationRefusal> {
    match value_type {
        ValType::F32 | ValType::F64 => Err(ValidationRefusal::ForbiddenFloatType),
        ValType::V128 => Err(ValidationRefusal::ForbiddenVectorType),
        ValType::I32 | ValType::I64 | ValType::FuncRef | ValType::ExternRef => Ok(()),
    }
}

fn refuse_block_type(block_type: BlockType) -> Result<(), ValidationRefusal> {
    match block_type {
        BlockType::Type(value_type) => refuse_value_type(value_type),
        BlockType::Empty | BlockType::FuncType(_) => Ok(()),
    }
}

fn refuse_float_code(body: &FunctionBody<'_>) -> Result<(), ValidationRefusal> {
    let locals = body
        .get_locals_reader()
        .map_err(|error| ValidationRefusal::MalformedModule {
            reason: error.to_string(),
        })?;
    for local in locals {
        let (_, value_type) = local.map_err(|error| ValidationRefusal::MalformedModule {
            reason: error.to_string(),
        })?;
        refuse_value_type(value_type)?;
    }
    let operators = body
        .get_operators_reader()
        .map_err(|error| ValidationRefusal::MalformedModule {
            reason: error.to_string(),
        })?;
    for operator in operators {
        let operator = operator.map_err(|error| ValidationRefusal::MalformedModule {
            reason: error.to_string(),
        })?;
        match operator {
            Operator::Block { blockty } | Operator::Loop { blockty } | Operator::If { blockty } => {
                refuse_block_type(blockty)?;
            }
            Operator::TypedSelect { ty } => refuse_value_type(ty)?,
            other => {
                if operator_uses_float(&other) {
                    return Err(ValidationRefusal::ForbiddenFloatInstruction);
                }
            }
        }
    }
    Ok(())
}

fn operator_uses_float(operator: &Operator<'_>) -> bool {
    matches!(
        operator,
        Operator::F32Load { .. }
            | Operator::F64Load { .. }
            | Operator::F32Store { .. }
            | Operator::F64Store { .. }
            | Operator::F32Const { .. }
            | Operator::F64Const { .. }
            | Operator::F32Eq
            | Operator::F32Ne
            | Operator::F32Lt
            | Operator::F32Gt
            | Operator::F32Le
            | Operator::F32Ge
            | Operator::F64Eq
            | Operator::F64Ne
            | Operator::F64Lt
            | Operator::F64Gt
            | Operator::F64Le
            | Operator::F64Ge
            | Operator::F32Abs
            | Operator::F32Neg
            | Operator::F32Ceil
            | Operator::F32Floor
            | Operator::F32Trunc
            | Operator::F32Nearest
            | Operator::F32Sqrt
            | Operator::F32Add
            | Operator::F32Sub
            | Operator::F32Mul
            | Operator::F32Div
            | Operator::F32Min
            | Operator::F32Max
            | Operator::F32Copysign
            | Operator::F64Abs
            | Operator::F64Neg
            | Operator::F64Ceil
            | Operator::F64Floor
            | Operator::F64Trunc
            | Operator::F64Nearest
            | Operator::F64Sqrt
            | Operator::F64Add
            | Operator::F64Sub
            | Operator::F64Mul
            | Operator::F64Div
            | Operator::F64Min
            | Operator::F64Max
            | Operator::F64Copysign
            | Operator::I32TruncF32S
            | Operator::I32TruncF32U
            | Operator::I32TruncF64S
            | Operator::I32TruncF64U
            | Operator::I64TruncF32S
            | Operator::I64TruncF32U
            | Operator::I64TruncF64S
            | Operator::I64TruncF64U
            | Operator::F32ConvertI32S
            | Operator::F32ConvertI32U
            | Operator::F32ConvertI64S
            | Operator::F32ConvertI64U
            | Operator::F32DemoteF64
            | Operator::F64ConvertI32S
            | Operator::F64ConvertI32U
            | Operator::F64ConvertI64S
            | Operator::F64ConvertI64U
            | Operator::F64PromoteF32
            | Operator::I32ReinterpretF32
            | Operator::I64ReinterpretF64
            | Operator::F32ReinterpretI32
            | Operator::F64ReinterpretI64
            | Operator::I32TruncSatF32S
            | Operator::I32TruncSatF32U
            | Operator::I32TruncSatF64S
            | Operator::I32TruncSatF64U
            | Operator::I64TruncSatF32S
            | Operator::I64TruncSatF32U
            | Operator::I64TruncSatF64S
            | Operator::I64TruncSatF64U
    )
}
