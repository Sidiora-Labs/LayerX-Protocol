use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sha2::{Digest, Sha256};

const LOCK_PATH: &str = "platform/sdk/pipeline.kvx";
const GO_GENERATED_PATH: &str = "platform/sdk/go/generated.go";

const SOURCES: [(&str, &str); 2] = [
    ("agent-api", "agent/schema/agent-api"),
    ("human-api", "human/schema/human-api"),
];

const OUTPUTS: [(&str, &str, &str, Option<&[&str]>); 5] = [
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
    (
        "platform-go",
        "go",
        "platform/sdk/go",
        Some(&["generated.go"]),
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
        let name = entry.file_name().into_string().map_err(|name| {
            format!(
                "non-unicode name {} under {}",
                name.display(),
                root.display()
            )
        })?;
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

type Sections = BTreeMap<String, BTreeMap<String, String>>;

fn schema_sections(root: &Path) -> Result<Sections, String> {
    let version_path = root.join("v1.kvx");
    let version_source = fs::read_to_string(&version_path)
        .map_err(|error| format!("read {}: {error}", version_path.display()))?;
    let version = layerx_platform_kvx::parse(&version_source)?;
    let mut files = vec!["v1.kvx".to_owned()];
    files.extend(layerx_platform_kvx::string_list(
        version.required("schema", "includes")?,
    )?);
    let mut sections = Sections::new();
    for file in files {
        let path = root.join(&file);
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let mut section = String::new();
        for (line_number, raw) in source.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
            {
                name.clone_into(&mut section);
                sections.entry(section.clone()).or_default();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(format!(
                    "{}:{} is not a key/value declaration",
                    path.display(),
                    line_number + 1
                ));
            };
            if section.is_empty() {
                return Err(format!(
                    "{}:{} is outside a section",
                    path.display(),
                    line_number + 1
                ));
            }
            sections
                .entry(section.clone())
                .or_default()
                .insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    Ok(sections)
}

fn variants(sections: &Sections, section: &str) -> Result<Vec<String>, String> {
    let value = sections
        .get(section)
        .and_then(|entries| entries.get("variants"))
        .ok_or_else(|| format!("missing {section}.variants"))?;
    layerx_platform_kvx::string_list(value)
}

fn go_identifier(value: &str) -> String {
    let mut output = String::new();
    let mut upper = true;
    for character in value.chars() {
        if !character.is_ascii_alphanumeric() {
            upper = true;
            continue;
        }
        if upper {
            output.push(character.to_ascii_uppercase());
            upper = false;
        } else {
            output.push(character);
        }
    }
    output
}

fn quoted(value: &str) -> String {
    format!("{value:?}")
}

fn render_string_enum(
    output: &mut String,
    type_name: &str,
    constant_prefix: &str,
    values: &[String],
) -> Result<(), String> {
    writeln!(output, "type {type_name} string\n").map_err(|error| error.to_string())?;
    writeln!(output, "const (").map_err(|error| error.to_string())?;
    for value in values {
        writeln!(
            output,
            "\t{constant_prefix}{} {type_name} = {}",
            go_identifier(value),
            quoted(value)
        )
        .map_err(|error| error.to_string())?;
    }
    writeln!(output, ")\n").map_err(|error| error.to_string())?;
    let cases = values
        .iter()
        .map(|value| format!("{constant_prefix}{}", go_identifier(value)))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        output,
        "func (value {type_name}) Valid() bool {{\n\tswitch value {{\n\tcase {cases}:\n\t\treturn true\n\tdefault:\n\t\treturn false\n\t}}\n}}\n"
    )
    .map_err(|error| error.to_string())
}

fn render_operation_type(
    output: &mut String,
    type_name: &str,
    prefix: &str,
    operations: &[String],
    mutations: &BTreeSet<String>,
) -> Result<(), String> {
    writeln!(output, "type {type_name} string\n").map_err(|error| error.to_string())?;
    writeln!(output, "const (").map_err(|error| error.to_string())?;
    for operation in operations {
        writeln!(
            output,
            "\t{prefix}{} {type_name} = {}",
            go_identifier(operation),
            quoted(operation)
        )
        .map_err(|error| error.to_string())?;
    }
    writeln!(output, ")\n").map_err(|error| error.to_string())?;
    writeln!(output, "func All{type_name}s() []{type_name} {{")
        .map_err(|error| error.to_string())?;
    writeln!(output, "\treturn []{type_name}{{").map_err(|error| error.to_string())?;
    for operation in operations {
        writeln!(output, "\t\t{prefix}{},", go_identifier(operation))
            .map_err(|error| error.to_string())?;
    }
    writeln!(output, "\t}}\n}}\n").map_err(|error| error.to_string())?;
    writeln!(output, "func (operation {type_name}) Valid() bool {{")
        .map_err(|error| error.to_string())?;
    writeln!(output, "\tswitch operation {{").map_err(|error| error.to_string())?;
    let valid_cases = operations
        .iter()
        .map(|operation| format!("{prefix}{}", go_identifier(operation)))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(output, "\tcase {valid_cases}:").map_err(|error| error.to_string())?;
    writeln!(
        output,
        "\t\treturn true\n\tdefault:\n\t\treturn false\n\t}}\n}}\n"
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "func (operation {type_name}) RequiresIdempotency() bool {{"
    )
    .map_err(|error| error.to_string())?;
    if mutations.is_empty() {
        writeln!(output, "\treturn false\n}}\n").map_err(|error| error.to_string())?;
    } else {
        writeln!(output, "\tswitch operation {{").map_err(|error| error.to_string())?;
        let mutation_cases = mutations
            .iter()
            .map(|operation| format!("{prefix}{}", go_identifier(operation)))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "\tcase {mutation_cases}:").map_err(|error| error.to_string())?;
        writeln!(
            output,
            "\t\treturn true\n\tdefault:\n\t\treturn false\n\t}}\n}}\n"
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn format_go(source: &str) -> Result<String, String> {
    let mut child = Command::new("gofmt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start gofmt for generated Go SDK: {error}"))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "gofmt stdin unavailable".to_owned())?
        .write_all(source.as_bytes())
        .map_err(|error| format!("write generated Go SDK to gofmt: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for gofmt: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "gofmt rejected generated Go SDK: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("gofmt returned non-UTF-8 output: {error}"))
}

fn generate_go(repo_root: &Path) -> Result<String, String> {
    let agent = schema_sections(&repo_root.join(SOURCES[0].1))?;
    let human = schema_sections(&repo_root.join(SOURCES[1].1))?;
    let agent_operations = agent
        .keys()
        .filter_map(|section| section.strip_prefix("operation.").map(str::to_owned))
        .collect::<Vec<_>>();
    let mut agent_mutations = agent
        .keys()
        .filter_map(|section| section.strip_prefix("mutation.").map(str::to_owned))
        .collect::<BTreeSet<_>>();
    for operation in &agent_operations {
        let has_idempotency_field = agent
            .get(&format!("operation.{operation}"))
            .and_then(|entries| entries.get("required"))
            .is_some_and(|value| value.contains("idempotency_key"));
        if has_idempotency_field {
            agent_mutations.insert(operation.clone());
        }
    }
    let human_operations = human
        .keys()
        .filter_map(|section| section.strip_prefix("operation.").map(str::to_owned))
        .collect::<Vec<_>>();
    let human_mutations = human_operations
        .iter()
        .filter(|operation| {
            human
                .get(&format!("operation.{operation}"))
                .and_then(|entries| entries.get("idempotency"))
                .is_some_and(|value| value == "true")
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    if agent_operations.is_empty() || human_operations.is_empty() {
        return Err("Go SDK generation found an empty operation catalogue".to_owned());
    }

    let mut output = String::from(
        "// Code generated from the LayerX Agent API and Human API schemas. DO NOT EDIT.\n\npackage layerx\n\n",
    );
    for (section, entries) in &agent {
        let Some(name) = section.strip_prefix("scalar.") else {
            continue;
        };
        let rust = entries
            .get("rust")
            .ok_or_else(|| format!("missing {section}.rust"))
            .and_then(|value| layerx_platform_kvx::unquote(value))?;
        let go_type = match rust.as_str() {
            "u128" => "Uint128",
            "u64" => "uint64",
            "u32" => "uint32",
            "u16" => "uint16",
            "u8" => "uint8",
            _ => {
                return Err(format!(
                    "unsupported Go scalar mapping for {section}: {rust}"
                ))
            }
        };
        writeln!(output, "type {name} = {go_type}").map_err(|error| error.to_string())?;
    }
    writeln!(output).map_err(|error| error.to_string())?;
    render_operation_type(
        &mut output,
        "AgentOperation",
        "AgentOperation",
        &agent_operations,
        &agent_mutations,
    )?;
    render_operation_type(
        &mut output,
        "HumanOperation",
        "HumanOperation",
        &human_operations,
        &human_mutations,
    )?;
    writeln!(
        output,
        "type HumanOperationMetadata struct {{\n\tMethod string\n\tPath string\n\tRequest string\n\tResponse string\n}}\n"
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "func (operation HumanOperation) Metadata() (HumanOperationMetadata, bool) {{\n\tswitch operation {{"
    )
    .map_err(|error| error.to_string())?;
    for operation in &human_operations {
        let entries = human
            .get(&format!("operation.{operation}"))
            .ok_or_else(|| format!("missing operation.{operation}"))?;
        let field = |name: &str| -> Result<String, String> {
            entries
                .get(name)
                .ok_or_else(|| format!("missing operation.{operation}.{name}"))
                .and_then(|value| layerx_platform_kvx::unquote(value))
        };
        writeln!(
            output,
            "\tcase HumanOperation{}:\n\t\treturn HumanOperationMetadata{{Method: {}, Path: {}, Request: {}, Response: {}}}, true",
            go_identifier(operation),
            quoted(&field("method")?),
            quoted(&field("path")?),
            quoted(&field("request")?),
            quoted(&field("response")?),
        )
        .map_err(|error| error.to_string())?;
    }
    writeln!(
        output,
        "\tdefault:\n\t\treturn HumanOperationMetadata{{}}, false\n\t}}\n}}\n"
    )
    .map_err(|error| error.to_string())?;
    render_string_enum(
        &mut output,
        "AgentErrorClass",
        "AgentError",
        &variants(&agent, "type.ErrorClass")?,
    )?;
    render_string_enum(
        &mut output,
        "HumanErrorCode",
        "HumanError",
        &variants(&human, "type.ErrorCode")?,
    )?;
    render_string_enum(
        &mut output,
        "JourneyKind",
        "Journey",
        &variants(&human, "type.JourneyKind")?,
    )?;
    render_string_enum(
        &mut output,
        "JourneyState",
        "JourneyState",
        &variants(&human, "type.JourneyState")?,
    )?;
    render_string_enum(
        &mut output,
        "HumanVerificationLevel",
        "HumanVerification",
        &variants(&human, "type.VerificationLevel")?,
    )?;
    render_string_enum(
        &mut output,
        "HumanRetriability",
        "HumanRetry",
        &variants(&human, "type.Retriability")?,
    )?;
    render_string_enum(
        &mut output,
        "HumanApprovalState",
        "HumanApproval",
        &variants(&human, "type.ApprovalState")?,
    )?;
    render_string_enum(
        &mut output,
        "HumanStreamEventKind",
        "HumanStreamEvent",
        &variants(&human, "type.StreamEventKind")?,
    )?;
    render_string_enum(
        &mut output,
        "AgentApprovalEventKind",
        "AgentApprovalEvent",
        &variants(&agent, "type.ApprovalLifecycleEvent")?,
    )?;
    render_string_enum(
        &mut output,
        "AgentApprovalState",
        "AgentApprovalState",
        &variants(&agent, "type.ApprovalState")?,
    )?;
    render_string_enum(
        &mut output,
        "AgentApprovalDecisionOutcome",
        "AgentApprovalOutcome",
        &variants(&agent, "type.ApprovalDecisionOutcome")?,
    )?;
    render_string_enum(
        &mut output,
        "AgentRetriability",
        "AgentRetry",
        &variants(&agent, "type.Retriability")?,
    )?;
    render_string_enum(
        &mut output,
        "AgentDeliveryKind",
        "AgentDelivery",
        &variants(&agent, "type.Delivery")?,
    )?;
    format_go(&output)
}

fn check_go(repo_root: &Path) -> Result<(), String> {
    let path = repo_root.join(GO_GENERATED_PATH);
    let actual = fs::read_to_string(&path)
        .map_err(|error| format!("generated Go file missing {}: {error}", path.display()))?;
    let expected = generate_go(repo_root)?;
    if actual != expected {
        return Err(format!(
            "generated Go file {} is stale or hand-edited; run make platform-sdk-generate",
            path.display()
        ));
    }
    Ok(())
}

fn write_go(repo_root: &Path) -> Result<(), String> {
    let path = repo_root.join(GO_GENERATED_PATH);
    let parent = path
        .parent()
        .ok_or_else(|| format!("generated path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    fs::write(&path, generate_go(repo_root)?)
        .map_err(|error| format!("write {}: {error}", path.display()))
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
    let source_names = layerx_platform_kvx::string_list(document.required("pipeline", "sources")?)?;
    let output_names = layerx_platform_kvx::string_list(document.required("pipeline", "outputs")?)?;
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
    drift_gate(&committed, &live)?;
    check_go(repo_root)
}

/// Captures the live schema and generated-tree state into the lock.
///
/// # Errors
///
/// Fails when a tree is unreadable or the lock cannot be written.
pub fn write_lock(repo_root: &Path, lock_path: &Path) -> Result<(), String> {
    write_go(repo_root)?;
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
    let repo_root = arguments
        .get(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let lock_path = arguments
        .get(2)
        .map_or_else(|| repo_root.join(LOCK_PATH), PathBuf::from);
    match mode {
        "--write" => write_lock(&repo_root, &lock_path),
        "--check" => check(&repo_root, &lock_path),
        _ => Err(
            "usage: layerx-platform-sdkgen [--write|--check] [repo-root] [lock-path]".to_owned(),
        ),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("platform-sdkgen: {error}");
        std::process::exit(1);
    }
}
