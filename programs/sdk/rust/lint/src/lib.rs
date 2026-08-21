//! Build-time determinism lint for `LayerX` programs.
//!
//! A program is refused before deployment ever sees it when it reaches for a
//! clock, for randomness, for floating point, or for any host interface
//! outside the frozen `layerx_v1` surface. Every refusal is named, so a broken
//! build says which rule was violated rather than which byte offset failed.
//!
//! The lint reuses the runtime's own deterministic-subset validator for the
//! module-level rules, so the toolchain and the node can never disagree about
//! what is admissible.

use std::fmt::{self, Display};
use std::fs;
use std::path::{Path, PathBuf};

use layerx_program_sdk::{CALL_ENTRY_EXPORT, CALL_RESERVE_EXPORT, MEMORY_EXPORT};
use layerx_programs_runtime::{ValidationRefusal, WasmEngine};
use wasmparser_nostd::{ExternalKind, Parser, Payload};

const CLOCK_IMPORT_NAMES: &[&str] = &[
    "clock_time_get",
    "clock_res_get",
    "clock_gettime",
    "gettimeofday",
    "date_now",
    "now",
    "time",
    "times",
];

const RANDOMNESS_IMPORT_NAMES: &[&str] = &[
    "random_get",
    "getrandom",
    "get_random_values",
    "random",
    "sched_yield",
];

const FORBIDDEN_DEPENDENCIES: &[&str] = &[
    "chrono",
    "float-cmp",
    "fastrand",
    "getrandom",
    "instant",
    "js-sys",
    "libm",
    "mio",
    "ordered-float",
    "rand",
    "rand_core",
    "socket2",
    "time",
    "tokio",
    "wasi",
    "wasm-bindgen",
    "web-sys",
];

const FORBIDDEN_SOURCE_ITEMS: &[&str] = &[
    "std::time::",
    "SystemTime::now",
    "Instant::now",
    "getrandom",
    "thread_rng",
    "OsRng",
    "rand::",
    "libm::",
    "wasi::",
    "wasm_bindgen",
];

const FLOAT_TYPE_NAMES: &[&str] = &["f32", "f64"];

/// A named determinism rule a program violated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeterminismViolation {
    /// The module imports a clock.
    ClockImport {
        /// Module field of the refused import.
        import_module: String,
        /// Name field of the refused import.
        import_name: String,
    },
    /// The module imports a source of randomness.
    RandomnessImport {
        /// Module field of the refused import.
        import_module: String,
        /// Name field of the refused import.
        import_name: String,
    },
    /// The module imports a host interface outside the frozen ABI.
    UndeclaredHostImport {
        /// Module field of the refused import.
        import_module: String,
        /// Name field of the refused import.
        import_name: String,
    },
    /// The module declares a floating-point type.
    FloatingPointType,
    /// The module contains a floating-point instruction.
    FloatingPointInstruction,
    /// The module declares a vector type.
    VectorType,
    /// The module exceeds the declared byte-size limit.
    ModuleTooLarge {
        /// Byte size of the refused module.
        byte_size: u64,
        /// Declared byte-size limit.
        limit: u64,
    },
    /// The module declares more functions than the declared limit allows.
    TooManyFunctions {
        /// Number of functions the module declares.
        function_count: u32,
        /// Declared function-count limit.
        limit: u32,
    },
    /// The module omits an export the runtime requires.
    MissingExport {
        /// Export the runtime could not find.
        export: String,
    },
    /// The artifact is not a well-formed WASM module.
    MalformedModule {
        /// Parser's reason for refusing the bytes.
        reason: String,
    },
    /// The deterministic engine refused the module.
    RejectedByEngine {
        /// Engine's reason for refusing the module.
        reason: String,
    },
    /// The deterministic engine could not be constructed.
    EngineUnavailable {
        /// Engine's reason for refusing its declared limits.
        reason: String,
    },
    /// The SDK's frozen ABI surface no longer matches the runtime's.
    AbiDrift {
        /// Exact difference between the two surfaces.
        detail: String,
    },
    /// The project declares a dependency on a clock, randomness or float
    /// library.
    ForbiddenDependency {
        /// Manifest declaring the dependency.
        manifest: PathBuf,
        /// Dependency the programs plane refuses.
        dependency: String,
    },
    /// The project's source reaches for an ambient nondeterministic item.
    ForbiddenSourceItem {
        /// Source file naming the item.
        source: PathBuf,
        /// Item the programs plane refuses.
        item: String,
    },
    /// A path the lint had to read could not be read.
    UnreadablePath {
        /// Path the lint could not read.
        path: PathBuf,
        /// Reason the read failed.
        reason: String,
    },
}

impl DeterminismViolation {
    /// Returns the stable rule name a broken build reports.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::ClockImport { .. } => "clock-import",
            Self::RandomnessImport { .. } => "randomness-import",
            Self::UndeclaredHostImport { .. } => "undeclared-host-import",
            Self::FloatingPointType => "floating-point-type",
            Self::FloatingPointInstruction => "floating-point-instruction",
            Self::VectorType => "vector-type",
            Self::ModuleTooLarge { .. } => "module-too-large",
            Self::TooManyFunctions { .. } => "too-many-functions",
            Self::MissingExport { .. } => "missing-export",
            Self::MalformedModule { .. } => "malformed-module",
            Self::RejectedByEngine { .. } => "rejected-by-engine",
            Self::EngineUnavailable { .. } => "engine-unavailable",
            Self::AbiDrift { .. } => "abi-drift",
            Self::ForbiddenDependency { .. } => "forbidden-dependency",
            Self::ForbiddenSourceItem { .. } => "forbidden-source-item",
            Self::UnreadablePath { .. } => "unreadable-path",
        }
    }
}

impl Display for DeterminismViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ClockImport {
                import_module,
                import_name,
            } => write!(formatter, "clock import {import_module}::{import_name}"),
            Self::RandomnessImport {
                import_module,
                import_name,
            } => write!(
                formatter,
                "randomness import {import_module}::{import_name}"
            ),
            Self::UndeclaredHostImport {
                import_module,
                import_name,
            } => write!(
                formatter,
                "undeclared host import {import_module}::{import_name}"
            ),
            Self::FloatingPointType => formatter.write_str("floating-point types are forbidden"),
            Self::FloatingPointInstruction => {
                formatter.write_str("floating-point instructions are forbidden")
            }
            Self::VectorType => formatter.write_str("vector types are forbidden"),
            Self::ModuleTooLarge { byte_size, limit } => write!(
                formatter,
                "module of {byte_size} bytes exceeds declared limit {limit}"
            ),
            Self::TooManyFunctions {
                function_count,
                limit,
            } => write!(
                formatter,
                "module declares {function_count} functions exceeding declared limit {limit}"
            ),
            Self::MissingExport { export } => {
                write!(formatter, "module exports no {export}")
            }
            Self::MalformedModule { reason } => write!(formatter, "malformed module: {reason}"),
            Self::RejectedByEngine { reason } => write!(formatter, "rejected by engine: {reason}"),
            Self::EngineUnavailable { reason } => {
                write!(formatter, "deterministic engine unavailable: {reason}")
            }
            Self::AbiDrift { detail } => {
                write!(formatter, "SDK ABI drifted from runtime: {detail}")
            }
            Self::ForbiddenDependency {
                manifest,
                dependency,
            } => write!(
                formatter,
                "{} declares forbidden dependency {dependency}",
                manifest.display()
            ),
            Self::ForbiddenSourceItem { source, item } => {
                write!(formatter, "{} reaches for {item}", source.display())
            }
            Self::UnreadablePath { path, reason } => {
                write!(formatter, "could not read {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for DeterminismViolation {}

/// Checks that the SDK still speaks exactly the runtime's frozen ABI.
#[must_use]
pub fn abi_surface_violations() -> Vec<DeterminismViolation> {
    let mut violations = Vec::new();
    if layerx_program_sdk::ABI_MODULE != layerx_programs_runtime::ABI_MODULE {
        violations.push(DeterminismViolation::AbiDrift {
            detail: format!(
                "host module {} is not {}",
                layerx_program_sdk::ABI_MODULE,
                layerx_programs_runtime::ABI_MODULE
            ),
        });
    }
    if layerx_program_sdk::ABI_MANIFEST != layerx_programs_runtime::ABI_MANIFEST {
        violations.push(DeterminismViolation::AbiDrift {
            detail: "frozen host-function manifest differs".to_string(),
        });
    }
    for (guest, host) in layerx_program_sdk::HOST_FUNCTIONS
        .iter()
        .zip(layerx_programs_runtime::HOST_FUNCTIONS.iter())
    {
        if guest.name != host.name || guest.signature != host.signature {
            violations.push(DeterminismViolation::AbiDrift {
                detail: format!(
                    "{}{} is not {}{}",
                    guest.name, guest.signature, host.name, host.signature
                ),
            });
        }
    }
    violations
}

/// Lints one compiled program artifact.
#[must_use]
pub fn lint_artifact(wasm: &[u8]) -> Vec<DeterminismViolation> {
    let mut violations = import_and_export_violations(wasm);
    violations.extend(engine_violations(wasm));
    violations
}

/// Lints the source and manifests of one program project.
#[must_use]
pub fn lint_sources(project: &Path) -> Vec<DeterminismViolation> {
    let mut violations = Vec::new();
    let mut files = Vec::new();
    if let Err(violation) = visit_files(project, &mut files) {
        violations.push(violation);
        return violations;
    }
    files.sort();
    for path in files {
        let body = match fs::read_to_string(&path) {
            Ok(body) => body,
            Err(error) => {
                violations.push(DeterminismViolation::UnreadablePath {
                    path,
                    reason: error.to_string(),
                });
                continue;
            }
        };
        if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            manifest_violations(&path, &body, &mut violations);
        } else {
            source_violations(&path, &body, &mut violations);
        }
    }
    violations
}

/// Lints one program project end to end: frozen ABI surface, project source
/// and manifests, and the compiled artifact.
#[must_use]
pub fn lint_project(project: &Path, artifact: Option<&Path>) -> Vec<DeterminismViolation> {
    let mut violations = abi_surface_violations();
    violations.extend(lint_sources(project));
    let path = match artifact.map(Path::to_path_buf).map_or_else(
        || discover_artifact(project),
        Ok::<PathBuf, DeterminismViolation>,
    ) {
        Ok(path) => path,
        Err(violation) => {
            violations.push(violation);
            return violations;
        }
    };
    match fs::read(&path) {
        Ok(wasm) => violations.extend(lint_artifact(&wasm)),
        Err(error) => violations.push(DeterminismViolation::UnreadablePath {
            path,
            reason: error.to_string(),
        }),
    }
    violations
}

/// Locates the single release artifact a program project produced.
///
/// # Errors
///
/// Names the directory when no artifact, or more than one, is present.
pub fn discover_artifact(project: &Path) -> Result<PathBuf, DeterminismViolation> {
    let directory = project.join("target/wasm32-unknown-unknown/release");
    let entries =
        fs::read_dir(&directory).map_err(|error| DeterminismViolation::UnreadablePath {
            path: directory.clone(),
            reason: error.to_string(),
        })?;
    let mut artifacts: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .collect();
    artifacts.sort();
    match artifacts.as_slice() {
        [artifact] => Ok(artifact.clone()),
        [] => Err(DeterminismViolation::UnreadablePath {
            path: directory,
            reason: "no .wasm artifact was produced".to_string(),
        }),
        _ => Err(DeterminismViolation::UnreadablePath {
            path: directory,
            reason: "multiple .wasm artifacts were produced".to_string(),
        }),
    }
}

fn permitted_import(import_module: &str, import_name: &str) -> bool {
    import_module == layerx_programs_runtime::ABI_MODULE
        && layerx_programs_runtime::HOST_FUNCTIONS
            .iter()
            .any(|function| function.name == import_name)
}

fn classify_import(import_module: &str, import_name: &str) -> DeterminismViolation {
    if CLOCK_IMPORT_NAMES.contains(&import_name) {
        return DeterminismViolation::ClockImport {
            import_module: import_module.to_string(),
            import_name: import_name.to_string(),
        };
    }
    if RANDOMNESS_IMPORT_NAMES.contains(&import_name) {
        return DeterminismViolation::RandomnessImport {
            import_module: import_module.to_string(),
            import_name: import_name.to_string(),
        };
    }
    DeterminismViolation::UndeclaredHostImport {
        import_module: import_module.to_string(),
        import_name: import_name.to_string(),
    }
}

fn import_and_export_violations(wasm: &[u8]) -> Vec<DeterminismViolation> {
    let mut violations = Vec::new();
    let mut memory = false;
    let mut entrypoint = false;
    let mut reservation = false;
    for payload in Parser::new(0).parse_all(wasm) {
        let payload = match payload {
            Ok(payload) => payload,
            Err(error) => {
                violations.push(DeterminismViolation::MalformedModule {
                    reason: error.to_string(),
                });
                return violations;
            }
        };
        match payload {
            Payload::ImportSection(reader) => {
                for entry in reader {
                    match entry {
                        Ok(import) => {
                            if !permitted_import(import.module, import.name) {
                                violations.push(classify_import(import.module, import.name));
                            }
                        }
                        Err(error) => violations.push(DeterminismViolation::MalformedModule {
                            reason: error.to_string(),
                        }),
                    }
                }
            }
            Payload::ExportSection(reader) => {
                for entry in reader {
                    match entry {
                        Ok(export) => {
                            if export.kind == ExternalKind::Memory && export.name == MEMORY_EXPORT {
                                memory = true;
                            }
                            if export.kind == ExternalKind::Func && export.name == CALL_ENTRY_EXPORT
                            {
                                entrypoint = true;
                            }
                            if export.kind == ExternalKind::Func
                                && export.name == CALL_RESERVE_EXPORT
                            {
                                reservation = true;
                            }
                        }
                        Err(error) => violations.push(DeterminismViolation::MalformedModule {
                            reason: error.to_string(),
                        }),
                    }
                }
            }
            _ => {}
        }
    }
    if !memory {
        violations.push(DeterminismViolation::MissingExport {
            export: MEMORY_EXPORT.to_string(),
        });
    }
    if !entrypoint {
        violations.push(DeterminismViolation::MissingExport {
            export: CALL_ENTRY_EXPORT.to_string(),
        });
    }
    if !reservation {
        violations.push(DeterminismViolation::MissingExport {
            export: CALL_RESERVE_EXPORT.to_string(),
        });
    }
    violations
}

fn engine_violations(wasm: &[u8]) -> Vec<DeterminismViolation> {
    let engine = match WasmEngine::declared() {
        Ok(engine) => engine,
        Err(error) => {
            return vec![DeterminismViolation::EngineUnavailable {
                reason: error.to_string(),
            }]
        }
    };
    match engine.validate(wasm) {
        Ok(_) => Vec::new(),
        Err(refusal) => vec![from_validation(&refusal)],
    }
}

fn from_validation(refusal: &ValidationRefusal) -> DeterminismViolation {
    match refusal {
        ValidationRefusal::ModuleTooLarge { byte_size, limit } => {
            DeterminismViolation::ModuleTooLarge {
                byte_size: *byte_size,
                limit: *limit,
            }
        }
        ValidationRefusal::TooManyFunctions {
            function_count,
            limit,
        } => DeterminismViolation::TooManyFunctions {
            function_count: *function_count,
            limit: *limit,
        },
        ValidationRefusal::ForbiddenImport {
            import_module,
            import_name,
        } => classify_import(import_module, import_name),
        ValidationRefusal::WrongImportKind { import_name }
        | ValidationRefusal::WrongImportSignature { import_name } => {
            DeterminismViolation::RejectedByEngine {
                reason: format!("invalid LayerX ABI import {import_name}"),
            }
        }
        ValidationRefusal::ForbiddenFloatType => DeterminismViolation::FloatingPointType,
        ValidationRefusal::ForbiddenFloatInstruction => {
            DeterminismViolation::FloatingPointInstruction
        }
        ValidationRefusal::ForbiddenVectorType => DeterminismViolation::VectorType,
        ValidationRefusal::MalformedModule { reason } => DeterminismViolation::MalformedModule {
            reason: reason.clone(),
        },
        ValidationRefusal::RejectedByEngine { reason } => DeterminismViolation::RejectedByEngine {
            reason: reason.clone(),
        },
    }
}

fn visit_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), DeterminismViolation> {
    let entries = fs::read_dir(root).map_err(|error| DeterminismViolation::UnreadablePath {
        path: root.to_path_buf(),
        reason: error.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| DeterminismViolation::UnreadablePath {
            path: root.to_path_buf(),
            reason: error.to_string(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|name| name == "target" || name == ".git")
            {
                continue;
            }
            visit_files(&path, output)?;
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml")
            || path.extension().is_some_and(|extension| extension == "rs")
        {
            output.push(path);
        }
    }
    Ok(())
}

fn manifest_violations(path: &Path, body: &str, violations: &mut Vec<DeterminismViolation>) {
    for line in body.lines() {
        let declaration = line.split('#').next().unwrap_or("").trim();
        for dependency in FORBIDDEN_DEPENDENCIES {
            let declared = declaration
                .strip_prefix(*dependency)
                .is_some_and(|tail| tail.trim_start().starts_with('='));
            if declared {
                violations.push(DeterminismViolation::ForbiddenDependency {
                    manifest: path.to_path_buf(),
                    dependency: (*dependency).to_string(),
                });
            }
        }
    }
}

fn source_violations(path: &Path, body: &str, violations: &mut Vec<DeterminismViolation>) {
    let code = strip_line_comments(body);
    for item in FORBIDDEN_SOURCE_ITEMS {
        if code.contains(item) {
            violations.push(DeterminismViolation::ForbiddenSourceItem {
                source: path.to_path_buf(),
                item: (*item).to_string(),
            });
        }
    }
    for name in FLOAT_TYPE_NAMES {
        if contains_word(&code, name) {
            violations.push(DeterminismViolation::ForbiddenSourceItem {
                source: path.to_path_buf(),
                item: (*name).to_string(),
            });
        }
    }
}

fn strip_line_comments(body: &str) -> String {
    let mut code = String::with_capacity(body.len());
    for line in body.lines() {
        let statement = line.split("//").next().unwrap_or("");
        code.push_str(statement);
        code.push('\n');
    }
    code
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn contains_word(body: &str, word: &str) -> bool {
    let bytes = body.as_bytes();
    let mut cursor = 0;
    while let Some(offset) = body.get(cursor..).and_then(|tail| tail.find(word)) {
        let start = cursor.saturating_add(offset);
        let end = start.saturating_add(word.len());
        let leading = start == 0
            || bytes
                .get(start.saturating_sub(1))
                .is_none_or(|byte| !is_identifier_byte(*byte));
        let trailing = bytes.get(end).is_none_or(|byte| !is_identifier_byte(*byte));
        if leading && trailing {
            return true;
        }
        cursor = end;
    }
    false
}
