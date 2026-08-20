use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const LOCK_PATH: &str = "platform/sdk/pipeline.kvx";

const SOURCES: [(&str, &str); 2] = [
    ("agent-api", "agent/schema/agent-api"),
    ("human-api", "human/schema/human-api"),
];

const OUTPUTS: [(&str, &str, &str, Option<&[&str]>); 4] = [
    (
        "agent-typescript",
        "typescript",
        "agent/sdk/typescript/src/generated",
        None,
    ),
    (
        "agent-python",
        "python",
        "agent/sdk/python/layerx_sdk/generated",
        None,
    ),
    (
        "agent-compatibility",
        "markdown",
        "agent/sdk",
        Some(&["COMPATIBILITY.md"]),
    ),
    (
        "human-typescript",
        "typescript",
        "human/apps/web/src/api/generated",
        None,
    ),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceState {
    pub name: String,
    pub root: String,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputState {
    pub name: String,
    pub language: String,
    pub root: String,
    pub files: Vec<(String, String)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pipeline {
    pub sources: Vec<SourceState>,
    pub outputs: Vec<OutputState>,
}

fn hex_digest(bytes: &[u8]) -> Result<String, String> {
    let mut rendered = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(rendered, "{byte:02x}").map_err(|error| format!("render digest: {error}"))?;
    }
    Ok(rendered)
}

fn walk_files(root: &Path, prefix: &str, files: &mut Vec<String>) -> Result<(), String> {
    let entries =
        fs::read_dir(root).map_err(|error| format!("read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read {}: {error}", root.display()))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|name| format!("non-unicode name {} under {}", name.display(), root.display()))?;
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let kind = entry
            .file_type()
            .map_err(|error| format!("stat {}: {error}", entry.path().display()))?;
        if kind.is_dir() {
            walk_files(&entry.path(), &relative, files)?;
        } else if kind.is_file() {
            files.push(relative);
        } else {
            return Err(format!(
                "unsupported entry {} in a pipeline tree",
                entry.path().display()
            ));
        }
    }
    Ok(())
}

fn tree_files(root: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    walk_files(root, "", &mut files)?;
    files.sort();
    Ok(files)
}

fn file_digest(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    hex_digest(&bytes)
}

fn source_digest(root: &Path) -> Result<String, String> {
    let mut manifest = String::new();
    for relative in tree_files(root)? {
        let digest = file_digest(&root.join(&relative))?;
        writeln!(manifest, "{relative}\n{digest}")
            .map_err(|error| format!("render source manifest: {error}"))?;
    }
    hex_digest(manifest.as_bytes())
}

/// Hashes both schema sources and every generated SDK tree as they exist on disk.
///
/// # Errors
///
/// Fails when a schema root or generated root is missing or unreadable.
pub fn capture(repo_root: &Path) -> Result<Pipeline, String> {
    let mut sources = Vec::new();
    for (name, root) in SOURCES {
        let path = repo_root.join(root);
        if !path.is_dir() {
            return Err(format!("schema source missing: {}", path.display()));
        }
        sources.push(SourceState {
            name: name.to_owned(),
            root: root.to_owned(),
            digest: source_digest(&path)?,
        });
    }
    let mut outputs = Vec::new();
    for (name, language, root, explicit) in OUTPUTS {
        let path = repo_root.join(root);
        let names = if let Some(list) = explicit {
            list.iter().map(|item| (*item).to_owned()).collect()
        } else {
            if !path.is_dir() {
                return Err(format!("generated root missing: {}", path.display()));
            }
            tree_files(&path)?
        };
        let mut files = Vec::new();
        for relative in names {
            let file = path.join(&relative);
            if !file.is_file() {
                return Err(format!("generated file missing: {}", file.display()));
            }
            let digest = file_digest(&file)?;
            files.push((relative, digest));
        }
        outputs.push(OutputState {
            name: name.to_owned(),
            language: language.to_owned(),
            root: root.to_owned(),
            files,
        });
    }
    Ok(Pipeline { sources, outputs })
}

/// Renders the pipeline lock document.
///
/// # Errors
///
/// Fails only when formatting into the lock text fails.
pub fn render(pipeline: &Pipeline) -> Result<String, String> {
    let mut text = String::new();
    let fail = |error| format!("render lock: {error}");
    writeln!(text, "[pipeline]").map_err(fail)?;
    let sources = pipeline
        .sources
        .iter()
        .map(|source| layerx_platform_kvx::quote(&source.name))
        .collect::<Vec<_>>()
        .join(", ");
    let outputs = pipeline
        .outputs
        .iter()
        .map(|output| layerx_platform_kvx::quote(&output.name))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(text, "sources = [{sources}]").map_err(fail)?;
    writeln!(text, "outputs = [{outputs}]").map_err(fail)?;
    for source in &pipeline.sources {
        writeln!(text, "\n[source.{}]", source.name).map_err(fail)?;
        writeln!(text, "root = {}", layerx_platform_kvx::quote(&source.root)).map_err(fail)?;
        writeln!(
            text,
            "digest = {}",
            layerx_platform_kvx::quote(&source.digest)
        )
        .map_err(fail)?;
    }
    for output in &pipeline.outputs {
        writeln!(text, "\n[output.{}]", output.name).map_err(fail)?;
        writeln!(
            text,
            "language = {}",
            layerx_platform_kvx::quote(&output.language)
        )
        .map_err(fail)?;
        writeln!(text, "root = {}", layerx_platform_kvx::quote(&output.root)).map_err(fail)?;
        writeln!(text, "\n[files.{}]", output.name).map_err(fail)?;
        for (relative, digest) in &output.files {
            writeln!(
                text,
                "{} = {}",
                layerx_platform_kvx::quote(relative),
                layerx_platform_kvx::quote(digest)
            )
            .map_err(fail)?;
        }
    }
    Ok(text)
}

/// Parses a committed pipeline lock document.
///
/// # Errors
///
/// Refuses malformed lock documents and missing declarations.
pub fn parse_lock(source: &str) -> Result<Pipeline, String> {
    let document = layerx_platform_kvx::parse(source)?;
    let source_names =
        layerx_platform_kvx::string_list(document.required("pipeline", "sources")?)?;
    let output_names =
        layerx_platform_kvx::string_list(document.required("pipeline", "outputs")?)?;
    let mut sources = Vec::new();
    for name in source_names {
        let section = format!("source.{name}");
        sources.push(SourceState {
            root: layerx_platform_kvx::unquote(document.required(&section, "root")?)?,
            digest: layerx_platform_kvx::unquote(document.required(&section, "digest")?)?,
            name,
        });
    }
    let mut outputs = Vec::new();
    for name in output_names {
        let section = format!("output.{name}");
        let language = layerx_platform_kvx::unquote(document.required(&section, "language")?)?;
        let root = layerx_platform_kvx::unquote(document.required(&section, "root")?)?;
        let mut files = Vec::new();
        for (relative, digest) in document.section_entries(&format!("files.{name}")) {
            files.push((relative.to_owned(), layerx_platform_kvx::unquote(digest)?));
        }
        outputs.push(OutputState {
            name,
            language,
            root,
            files,
        });
    }
    Ok(Pipeline { sources, outputs })
}

fn structure_error(detail: &str) -> String {
    format!("pipeline lock does not match the wired pipeline ({detail}); run make platform-sdk-generate")
}

/// Fails when any generated SDK output is stale against its schemas or hand-edited.
///
/// # Errors
///
/// Names the first stale schema source or drifted generated file.
pub fn drift_gate(committed: &Pipeline, live: &Pipeline) -> Result<(), String> {
    if committed.sources.len() != live.sources.len() {
        return Err(structure_error("schema source list changed"));
    }
    for (committed_source, live_source) in committed.sources.iter().zip(&live.sources) {
        if committed_source.name != live_source.name || committed_source.root != live_source.root {
            return Err(structure_error("schema source list changed"));
        }
        if committed_source.digest != live_source.digest {
            return Err(format!(
                "stale generated SDKs: schema {} at {} changed after the last generation; run make platform-sdk-generate",
                live_source.name, live_source.root
            ));
        }
    }
    if committed.outputs.len() != live.outputs.len() {
        return Err(structure_error("generated output list changed"));
    }
    for (committed_output, live_output) in committed.outputs.iter().zip(&live.outputs) {
        if committed_output.name != live_output.name
            || committed_output.language != live_output.language
            || committed_output.root != live_output.root
        {
            return Err(structure_error("generated output list changed"));
        }
        for (relative, digest) in &committed_output.files {
            match live_output
                .files
                .iter()
                .find(|(live_relative, _)| live_relative == relative)
            {
                None => {
                    return Err(format!(
                        "generated {} file missing: {}/{relative}",
                        live_output.language, live_output.root
                    ));
                }
                Some((_, live_digest)) if live_digest != digest => {
                    return Err(format!(
                        "generated {} file {}/{relative} is stale or hand-edited; run make platform-sdk-generate",
                        live_output.language, live_output.root
                    ));
                }
                Some(_) => {}
            }
        }
        for (relative, _) in &live_output.files {
            if !committed_output
                .files
                .iter()
                .any(|(committed_relative, _)| committed_relative == relative)
            {
                return Err(format!(
                    "untracked file in generated {} root: {}/{relative}; run make platform-sdk-generate",
                    live_output.language, live_output.root
                ));
            }
        }
    }
    Ok(())
}

/// Runs the drift gate against the committed lock.
///
/// # Errors
///
/// Fails when the lock is missing, stale or any generated output drifted.
pub fn check(repo_root: &Path, lock_path: &Path) -> Result<(), String> {
    let committed = fs::read_to_string(lock_path).map_err(|error| {
        format!(
            "pipeline lock missing at {}: {error}; run make platform-sdk-generate",
            lock_path.display()
        )
    })?;
    let committed = parse_lock(&committed)?;
    let live = capture(repo_root)?;
    drift_gate(&committed, &live)
}

/// Captures the live schema and generated-tree state into the lock.
///
/// # Errors
///
/// Fails when a tree is unreadable or the lock cannot be written.
pub fn write_lock(repo_root: &Path, lock_path: &Path) -> Result<(), String> {
    let pipeline = capture(repo_root)?;
    let text = render(&pipeline)?;
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(lock_path, text).map_err(|error| format!("write {}: {error}", lock_path.display()))
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let mode = arguments.first().map_or("--check", String::as_str);
    let repo_root = arguments.get(1).map_or_else(|| PathBuf::from("."), PathBuf::from);
    let lock_path = arguments
        .get(2)
        .map_or_else(|| repo_root.join(LOCK_PATH), PathBuf::from);
    match mode {
        "--write" => write_lock(&repo_root, &lock_path),
        "--check" => check(&repo_root, &lock_path),
        _ => Err("usage: layerx-platform-sdkgen [--write|--check] [repo-root] [lock-path]".to_owned()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("platform-sdkgen: {error}");
        std::process::exit(1);
    }
}
