use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use layerx_programs_runtime::WasmEngine;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use crate::encoding::hex_encode;
use crate::http::{validate_idempotency_key, validate_resource_id, Client};

const DESCRIPTOR: &str = "layerx-program.json";

struct Step {
    command: String,
    args: Vec<String>,
}

struct Toolchain {
    project: PathBuf,
    language: String,
    build: Step,
    artifact: PathBuf,
    lint: Option<Step>,
}

pub fn build(manifest: &Path, artifact: Option<&Path>) -> Result<Value, String> {
    let manifest = manifest
        .canonicalize()
        .map_err(|error| format!("could not resolve {}: {error}", manifest.display()))?;
    let project = if manifest.is_dir() {
        manifest.clone()
    } else {
        manifest
            .parent()
            .ok_or_else(|| "program manifest has no parent directory".to_string())?
            .to_path_buf()
    };
    match load_toolchain(&project)? {
        Some(toolchain) => build_with_toolchain(&toolchain, artifact),
        None => build_with_cargo(&manifest, &project, artifact),
    }
}

fn build_with_toolchain(toolchain: &Toolchain, artifact: Option<&Path>) -> Result<Value, String> {
    run_step(
        &toolchain.project,
        &toolchain.build,
        &format!("{} program toolchain", toolchain.language),
    )?;
    let artifact = match artifact {
        Some(path) => resolve(&toolchain.project, path),
        None => resolve(&toolchain.project, &toolchain.artifact),
    };
    if !artifact.exists() {
        return Err(format!(
            "the {} program toolchain produced no artifact at {}",
            toolchain.language,
            artifact.display()
        ));
    }
    let determinism_lint = match &toolchain.lint {
        Some(lint) => {
            run_step(
                &toolchain.project,
                lint,
                &format!("{} determinism lint", toolchain.language),
            )?;
            "passed"
        }
        None => "not declared by the toolchain descriptor",
    };
    let mut inspected = inspect_artifact(&artifact)?;
    if let Some(object) = inspected.as_object_mut() {
        object.insert("language".into(), json!(toolchain.language));
        object.insert("determinism_lint".into(), json!(determinism_lint));
    }
    Ok(inspected)
}

fn build_with_cargo(
    manifest: &Path,
    project: &Path,
    artifact: Option<&Path>,
) -> Result<Value, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(project)
        .args([
            "build",
            "--manifest-path",
            manifest
                .to_str()
                .ok_or_else(|| "program manifest path is not valid UTF-8".to_string())?,
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .status()
        .map_err(|error| format!("could not start the Rust program toolchain: {error}"))?;
    if !status.success() {
        return Err(format!("Rust program toolchain failed with {status}"));
    }
    let artifact = match artifact {
        Some(path) => resolve(project, path),
        None => discover_artifact(project)?,
    };
    let mut inspected = inspect_artifact(&artifact)?;
    if let Some(object) = inspected.as_object_mut() {
        object.insert("language".into(), json!("rust"));
        object.insert(
            "determinism_lint".into(),
            json!(format!(
                "not run; the project declares no {DESCRIPTOR} toolchain descriptor"
            )),
        );
    }
    Ok(inspected)
}

pub fn inspect_artifact(path: &Path) -> Result<Value, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("could not resolve {}: {error}", path.display()))?;
    let wasm =
        fs::read(&path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let engine = WasmEngine::declared()
        .map_err(|error| format!("could not initialize deterministic WASM engine: {error}"))?;
    let validated = engine
        .validate(&wasm)
        .map_err(|error| format!("program violates the deterministic WASM policy: {error}"))?;
    let code_hash: [u8; 32] = Sha256::digest(&wasm).into();
    Ok(json!({
        "artifact": path.display().to_string(),
        "code_hash": hex_encode(&code_hash),
        "byte_size": validated.byte_size(),
        "function_count": validated.function_count(),
        "abi_version": 1,
        "deterministic_validation": "passed",
    }))
}

pub fn deploy(
    client: &Client,
    path: &Path,
    upgrade_authority: Option<&str>,
    source_uri: Option<&str>,
    idempotency_key: &str,
) -> Result<Value, String> {
    validate_idempotency_key(idempotency_key)?;
    let mut inspected = inspect_artifact(path)?;
    let gate = gate_artifact(path)?;
    if let Some(object) = inspected.as_object_mut() {
        object.insert("language".into(), json!(gate.0));
        object.insert("determinism_lint".into(), json!(gate.1));
    }
    let wasm =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let policy = match upgrade_authority {
        Some(authority) => json!({"kind": "upgradeable", "authority": authority}),
        None => json!({"kind": "immutable"}),
    };
    let request = json!({
        "abi_version": 1,
        "code_hash": inspected["code_hash"],
        "wasm_hex": hex_encode(&wasm),
        "upgrade_policy": policy,
        "source_uri": source_uri,
    });
    let response = client.post("/v1/programs/deploy", &request, Some(idempotency_key))?;
    Ok(json!({
        "artifact": inspected,
        "deployment": response,
        "verification": "server outcome; verify the returned receipt before treating the program as callable",
    }))
}

pub fn registry_get(client: &Client, program_id: &str) -> Result<Value, String> {
    validate_resource_id(program_id, "program id")?;
    client.get(&format!("/v1/programs/registry/{program_id}"))
}

pub fn registry_verify_source(
    client: &Client,
    program_id: &str,
    source_uri: &str,
    source_digest: &str,
    idempotency_key: &str,
) -> Result<Value, String> {
    validate_resource_id(program_id, "program id")?;
    validate_idempotency_key(idempotency_key)?;
    let _: [u8; 32] = crate::encoding::fixed_hex("source digest", source_digest)?;
    client.post(
        &format!("/v1/programs/registry/{program_id}/source"),
        &json!({
            "source_uri": source_uri,
            "source_digest": source_digest,
        }),
        Some(idempotency_key),
    )
}

fn gate_artifact(path: &Path) -> Result<(String, String), String> {
    let artifact = path
        .canonicalize()
        .map_err(|error| format!("could not resolve {}: {error}", path.display()))?;
    let Some(project) = enclosing_project(&artifact) else {
        return Ok((
            "unknown".into(),
            format!("not run; no {DESCRIPTOR} toolchain descriptor encloses the artifact"),
        ));
    };
    let Some(toolchain) = load_toolchain(&project)? else {
        return Ok((
            "unknown".into(),
            format!("not run; no {DESCRIPTOR} toolchain descriptor encloses the artifact"),
        ));
    };
    match &toolchain.lint {
        Some(lint) => {
            run_step(
                &toolchain.project,
                lint,
                &format!("{} determinism lint", toolchain.language),
            )?;
            Ok((toolchain.language, "passed".into()))
        }
        None => Ok((
            toolchain.language,
            "not declared by the toolchain descriptor".into(),
        )),
    }
}

fn enclosing_project(artifact: &Path) -> Option<PathBuf> {
    let mut directory = artifact.parent();
    while let Some(candidate) = directory {
        if candidate.join(DESCRIPTOR).is_file() {
            return Some(candidate.to_path_buf());
        }
        directory = candidate.parent();
    }
    None
}

fn load_toolchain(project: &Path) -> Result<Option<Toolchain>, String> {
    let descriptor = project.join(DESCRIPTOR);
    if !descriptor.is_file() {
        return Ok(None);
    }
    let contents = fs::read_to_string(&descriptor)
        .map_err(|error| format!("could not read {}: {error}", descriptor.display()))?;
    let document: Value = serde_json::from_str(&contents)
        .map_err(|error| format!("could not parse {}: {error}", descriptor.display()))?;
    let language = string_field(&document, "language", &descriptor)?;
    let build = document
        .get("build")
        .ok_or_else(|| format!("{} declares no build step", descriptor.display()))?;
    let artifact = string_field(build, "artifact", &descriptor)?;
    Ok(Some(Toolchain {
        project: project.to_path_buf(),
        language,
        build: step(build, "build", &descriptor)?,
        artifact: PathBuf::from(artifact),
        lint: match document.get("lint") {
            Some(value) => Some(step(value, "lint", &descriptor)?),
            None => None,
        },
    }))
}

fn step(value: &Value, name: &str, descriptor: &Path) -> Result<Step, String> {
    let command = string_field(value, "command", descriptor)?;
    let args = match value.get("args") {
        Some(Value::Array(entries)) => entries
            .iter()
            .map(|entry| {
                entry.as_str().map(str::to_owned).ok_or_else(|| {
                    format!(
                        "{} declares a non-string argument in its {name} step",
                        descriptor.display()
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(format!(
                "{} declares a non-array args in its {name} step",
                descriptor.display()
            ))
        }
        None => Vec::new(),
    };
    Ok(Step { command, args })
}

fn string_field(value: &Value, key: &str, descriptor: &Path) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{} declares no {key}", descriptor.display()))
}

fn run_step(project: &Path, step: &Step, description: &str) -> Result<(), String> {
    let status = Command::new(&step.command)
        .current_dir(project)
        .args(&step.args)
        .status()
        .map_err(|error| format!("could not start the {description}: {error}"))?;
    if !status.success() {
        return Err(format!("the {description} failed with {status}"));
    }
    Ok(())
}

fn resolve(project: &Path, artifact: &Path) -> PathBuf {
    if artifact.is_absolute() {
        artifact.to_owned()
    } else {
        project.join(artifact)
    }
}

fn discover_artifact(project: &Path) -> Result<PathBuf, String> {
    let directory = project.join("target/wasm32-unknown-unknown/release");
    let mut artifacts = fs::read_dir(&directory)
        .map_err(|error| format!("could not inspect {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .collect::<Vec<_>>();
    artifacts.sort();
    match artifacts.as_slice() {
        [artifact] => Ok(artifact.clone()),
        [] => Err(format!(
            "the Rust program toolchain produced no .wasm artifact in {}",
            directory.display()
        )),
        _ => Err("multiple .wasm artifacts were produced; select one with --artifact".into()),
    }
}
