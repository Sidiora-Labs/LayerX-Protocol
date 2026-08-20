use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use layerx_programs_runtime::WasmEngine;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use crate::encoding::hex_encode;
use crate::http::{validate_idempotency_key, validate_resource_id, Client};

pub fn build(manifest: &Path, artifact: Option<&Path>) -> Result<Value, String> {
    let manifest = manifest
        .canonicalize()
        .map_err(|error| format!("could not resolve {}: {error}", manifest.display()))?;
    let project = manifest
        .parent()
        .ok_or_else(|| "program manifest has no parent directory".to_string())?;
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
    inspect_artifact(&artifact)
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
    let inspected = inspect_artifact(path)?;
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
