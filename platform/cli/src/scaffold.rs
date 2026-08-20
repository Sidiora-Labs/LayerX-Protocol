use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::config::ensure_directory_empty;

pub fn create(name: &str, parent: &Path) -> Result<serde_json::Value, String> {
    validate_name(name)?;
    let project = parent.join(name);
    ensure_directory_empty(&project)?;
    fs::create_dir_all(project.join("src"))
        .map_err(|error| format!("could not create {}: {error}", project.display()))?;
    let crate_name = name.replace('-', "_");
    write_new(
        &project.join("Cargo.toml"),
        &format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n[profile.release]\npanic = \"abort\"\n"
        ),
    )?;
    write_new(
        &project.join("src/lib.rs"),
        "#![no_std]\n\nuse core::panic::PanicInfo;\n\n#[panic_handler]\nfn panic(_information: &PanicInfo<'_>) -> ! {\n    loop {}\n}\n\n#[no_mangle]\npub extern \"C\" fn layerx_main(value: i64) -> i64 {\n    value\n}\n",
    )?;
    write_new(
        &project.join("LayerX.toml"),
        "schema = 1\nkind = \"program\"\nabi = 1\nenvironment = \"emulator\"\n",
    )?;
    write_new(&project.join(".gitignore"), "/target\n")?;
    Ok(json!({
        "path": absolute(&project)?.display().to_string(),
        "package": name,
        "crate": crate_name,
        "kind": "rust-program",
        "files": ["Cargo.toml", "LayerX.toml", "src/lib.rs", ".gitignore"],
    }))
}

fn write_new(path: &Path, contents: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    file.write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || name.starts_with('-')
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err("project name must be a lowercase Cargo package name".into());
    }
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("could not resolve {}: {error}", path.display()))
}
