pub mod a2a;
pub mod mcp;

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, DirBuilder, Metadata, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::config::{self, Configuration};
use crate::credential;

pub const SERVER_NAME: &str = "layerx";

const SECRET_MARKERS: [&str; 9] = [
    "token",
    "api_key",
    "apikey",
    "secret",
    "password",
    "seed",
    "private_key",
    "authorization",
    "credential",
];

const HOSTS: [Host; 5] = [
    Host::Layerx,
    Host::ClaudeCode,
    Host::ClaudeDesktop,
    Host::Cursor,
    Host::VsCode,
];

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Host {
    Layerx,
    ClaudeCode,
    ClaudeDesktop,
    Cursor,
    VsCode,
}

impl Host {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "layerx" => Ok(Self::Layerx),
            "claude-code" => Ok(Self::ClaudeCode),
            "claude-desktop" => Ok(Self::ClaudeDesktop),
            "cursor" => Ok(Self::Cursor),
            "vscode" => Ok(Self::VsCode),
            _ => Err(format!(
                "agent runtime {value} is not supported; use layerx, claude-code, claude-desktop, cursor, or vscode"
            )),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Layerx => "layerx",
            Self::ClaudeCode => "claude-code",
            Self::ClaudeDesktop => "claude-desktop",
            Self::Cursor => "cursor",
            Self::VsCode => "vscode",
        }
    }

    pub const fn section(self) -> &'static str {
        match self {
            Self::VsCode => "servers",
            _ => "mcpServers",
        }
    }

    pub fn path(self) -> Result<PathBuf, String> {
        Ok(match self {
            Self::Layerx => layerx_directory()?.join("mcp.json"),
            Self::ClaudeCode => home()?.join(".claude.json"),
            Self::ClaudeDesktop => claude_desktop_directory()?.join("claude_desktop_config.json"),
            Self::Cursor => home()?.join(".cursor").join("mcp.json"),
            Self::VsCode => code_directory()?.join("User").join("mcp.json"),
        })
    }

    fn marker(self) -> Result<PathBuf, String> {
        Ok(match self {
            Self::Layerx => layerx_directory()?,
            Self::ClaudeCode => home()?.join(".claude"),
            Self::ClaudeDesktop => claude_desktop_directory()?,
            Self::Cursor => home()?.join(".cursor"),
            Self::VsCode => code_directory()?,
        })
    }

    pub fn entry(
        self,
        command: &str,
        arguments: &[String],
        variables: &BTreeMap<String, String>,
    ) -> Value {
        let mut entry = Map::new();
        if self == Self::VsCode {
            entry.insert("type".to_owned(), json!("stdio"));
        }
        entry.insert("command".to_owned(), json!(command));
        entry.insert("args".to_owned(), json!(arguments));
        entry.insert("env".to_owned(), json!(variables));
        Value::Object(entry)
    }
}

pub struct Registration {
    pub path: PathBuf,
    pub section: &'static str,
    pub name: String,
    pub entry: Value,
}

pub struct Outcome {
    pub action: &'static str,
    pub changed: bool,
}

pub struct Selection {
    pub environment: String,
    pub endpoint: String,
    pub network_id: u32,
    pub key: String,
    pub did: String,
    pub public_key: String,
    pub created_key: bool,
}

impl Selection {
    pub fn credentials(&self) -> Value {
        json!({
            "key": self.key,
            "did": self.did,
            "public_key": self.public_key,
            "created": self.created_key,
            "storage": "operating-system-credential-store",
            "scope": "one environment and one key",
            "written_to_disk": false,
        })
    }
}

pub fn select(
    configuration: &mut Configuration,
    environment: Option<String>,
    key: Option<String>,
    fallback_key: &str,
    token_stdin: bool,
) -> Result<Selection, String> {
    let environment = match environment {
        Some(name) => {
            Configuration::validate_environment_name(&name)?;
            name
        }
        None => configuration.current_environment.clone(),
    };
    let profile = configuration.environments.get(&environment).ok_or_else(|| {
        format!(
            "environment {environment} is not configured; run layerx environment use {environment} --endpoint <url> --network-id <id>"
        )
    })?;
    let endpoint = profile.endpoint.clone();
    let network_id = profile.network_id;
    if token_stdin {
        credential::set_token(&environment)?;
    }
    if environment != "emulator" && credential::token(&environment)?.is_none() {
        return Err(format!(
            "no {environment} API token is held in credential storage; pipe one in with --token-stdin or run layerx auth set --environment {environment}"
        ));
    }
    let (name, created_key) = resolve_key(configuration, key, fallback_key)?;
    let metadata = configuration
        .keys
        .get(&name)
        .ok_or_else(|| format!("key {name} does not exist"))?;
    Ok(Selection {
        environment,
        endpoint,
        network_id,
        key: name.clone(),
        did: metadata.did.clone(),
        public_key: metadata.public_key.clone(),
        created_key,
    })
}

fn resolve_key(
    configuration: &mut Configuration,
    key: Option<String>,
    fallback_key: &str,
) -> Result<(String, bool), String> {
    if let Some(name) = key {
        if !configuration.keys.contains_key(&name) {
            return Err(format!("key {name} does not exist"));
        }
        return Ok((name, false));
    }
    if let Some(name) = configuration.default_key.clone() {
        if configuration.keys.contains_key(&name) {
            return Ok((name, false));
        }
    }
    if configuration.keys.contains_key(fallback_key) {
        return Ok((fallback_key.to_owned(), false));
    }
    credential::create_key(configuration, fallback_key, None)?;
    Ok((fallback_key.to_owned(), true))
}

pub fn hosts(requested: &[String]) -> Result<Vec<Host>, String> {
    if requested.is_empty() {
        return detected();
    }
    let mut selected = Vec::with_capacity(requested.len());
    for value in requested {
        let host = Host::parse(value)?;
        if !selected.contains(&host) {
            selected.push(host);
        }
    }
    Ok(selected)
}

fn detected() -> Result<Vec<Host>, String> {
    let mut present = Vec::new();
    for host in HOSTS {
        if host == Host::Layerx || host.marker()?.exists() || host.path()?.exists() {
            present.push(host);
        }
    }
    Ok(present)
}

pub fn executable() -> Result<String, String> {
    let resolved = env::current_exe()
        .map_err(|error| format!("could not resolve the running executable: {error}"))?;
    let resolved = fs::canonicalize(&resolved).unwrap_or(resolved);
    resolved
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| "the running executable path is not valid UTF-8".to_string())
}

pub fn variables() -> Result<BTreeMap<String, String>, String> {
    let mut variables = BTreeMap::new();
    variables.insert(
        "LAYERX_CONFIG".to_owned(),
        config::path()?.display().to_string(),
    );
    Ok(variables)
}

pub fn apply(registration: &Registration) -> Result<Outcome, String> {
    reject_secret_material(&registration.entry)?;
    let mut document = match read_document(&registration.path)? {
        Some(Value::Object(map)) => map,
        Some(_) => {
            return Err(format!(
                "{} does not hold a JSON object",
                registration.path.display()
            ))
        }
        None => Map::new(),
    };
    let mut entries = match document.remove(registration.section) {
        Some(Value::Object(map)) => map,
        Some(_) => {
            return Err(format!(
                "{} does not hold {} as a JSON object",
                registration.path.display(),
                registration.section
            ))
        }
        None => Map::new(),
    };
    let identical = entries.get(&registration.name) == Some(&registration.entry);
    if identical && private(&registration.path)? {
        return Ok(Outcome {
            action: "unchanged",
            changed: false,
        });
    }
    let action = if entries.contains_key(&registration.name) {
        "updated"
    } else {
        "created"
    };
    entries.insert(registration.name.clone(), registration.entry.clone());
    document.insert(registration.section.to_owned(), Value::Object(entries));
    let encoded = serde_json::to_string_pretty(&Value::Object(document))
        .map_err(|error| format!("could not encode {}: {error}", registration.path.display()))?;
    write_private(&registration.path, &encoded)?;
    Ok(Outcome {
        action,
        changed: true,
    })
}

pub fn publish(path: &Path, document: &Value) -> Result<Outcome, String> {
    reject_secret_material(document)?;
    let current = read_document(path)?;
    if current.as_ref() == Some(document) && private(path)? {
        return Ok(Outcome {
            action: "unchanged",
            changed: false,
        });
    }
    let action = if current.is_some() {
        "updated"
    } else {
        "created"
    };
    let encoded = serde_json::to_string_pretty(document)
        .map_err(|error| format!("could not encode {}: {error}", path.display()))?;
    write_private(path, &encoded)?;
    Ok(Outcome {
        action,
        changed: true,
    })
}

pub fn report(path: &Path, section: &str, name: &str, outcome: &Outcome) -> Value {
    json!({
        "path": path.display().to_string(),
        "section": section,
        "name": name,
        "action": outcome.action,
        "changed": outcome.changed,
        "permissions": "owner-only",
    })
}

fn read_document(path: &Path) -> Result<Option<Value>, String> {
    match fs::read_to_string(path) {
        Ok(source) if source.trim().is_empty() => Ok(None),
        Ok(source) => serde_json::from_str(&source)
            .map(Some)
            .map_err(|error| format!("could not parse {}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("could not read {}: {error}", path.display())),
    }
}

fn reject_secret_material(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                let lowered = key.to_ascii_lowercase();
                if SECRET_MARKERS.iter().any(|marker| lowered.contains(marker)) {
                    return Err(format!(
                        "installed configuration may not carry the field {key}; secrets stay in credential storage"
                    ));
                }
                reject_secret_material(nested)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for item in items {
                reject_secret_material(item)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn write_private(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
    ensure_directory(parent)?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = write_temporary(&temporary, path, contents);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return result;
    }
    if private(path)? {
        Ok(())
    } else {
        Err(format!(
            "{} is readable beyond its owner after installation",
            path.display()
        ))
    }
}

fn write_temporary(temporary: &Path, path: &Path, contents: &str) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(temporary)
        .map_err(|error| format!("could not create {}: {error}", temporary.display()))?;
    file.write_all(contents.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    fs::rename(temporary, path)
        .map_err(|error| format!("could not replace {}: {error}", path.display()))
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))
}

fn private(path: &Path) -> Result<bool, String> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(owner_only(&metadata)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("could not inspect {}: {error}", path.display())),
    }
}

#[cfg(unix)]
#[allow(clippy::verbose_bit_mask)]
fn owner_only(metadata: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.mode() & 0o077 == 0
}

#[cfg(not(unix))]
const fn owner_only(_metadata: &Metadata) -> bool {
    true
}

pub fn layerx_directory() -> Result<PathBuf, String> {
    let path = config::path()?;
    path.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "the CLI configuration path has no parent directory".to_string())
}

fn home() -> Result<PathBuf, String> {
    if let Some(root) = env::var_os("LAYERX_INSTALL_ROOT") {
        return Ok(PathBuf::from(root));
    }
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is required to locate agent runtime configuration".to_string())
}

fn xdg_config() -> Result<PathBuf, String> {
    if env::var_os("LAYERX_INSTALL_ROOT").is_some() {
        return Ok(home()?.join(".config"));
    }
    if let Some(base) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(base));
    }
    Ok(home()?.join(".config"))
}

fn application_support() -> Result<PathBuf, String> {
    Ok(home()?.join("Library").join("Application Support"))
}

fn claude_desktop_directory() -> Result<PathBuf, String> {
    if cfg!(target_os = "macos") {
        application_support().map(|base| base.join("Claude"))
    } else {
        xdg_config().map(|base| base.join("Claude"))
    }
}

fn code_directory() -> Result<PathBuf, String> {
    if cfg!(target_os = "macos") {
        application_support().map(|base| base.join("Code"))
    } else {
        xdg_config().map(|base| base.join("Code"))
    }
}
