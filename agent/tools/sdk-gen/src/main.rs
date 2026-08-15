use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const TYPESCRIPT_TEMPLATE: &str = include_str!("../templates/typescript.tpl");
const PYTHON_TEMPLATE: &str = include_str!("../templates/python.tpl");
const GUARANTEES_TEMPLATE: &str = include_str!("../templates/guarantees.md.tpl");

#[derive(Clone, Debug, Eq, PartialEq)]
struct Scalar {
    name: String,
    rust: String,
    typescript: String,
    python: String,
    wire: String,
    consensus_integer: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Model {
    scalars: Vec<Scalar>,
    operations: Vec<String>,
    levels: Vec<String>,
    errors: Vec<String>,
    guarantees: Vec<(String, String, String)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Generated {
    pub files: BTreeMap<PathBuf, String>,
}

fn section_map(source: &str) -> Result<BTreeMap<String, String>, String> {
    let mut section = String::new();
    let mut entries = BTreeMap::new();
    for (number, raw) in source.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            name.clone_into(&mut section);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "line {} is not a key/value declaration",
                number + 1
            ));
        };
        if section.is_empty() {
            return Err(format!("line {} is outside a section", number + 1));
        }
        let path = format!("{}.{}", section, key.trim());
        if entries
            .insert(path.clone(), value.trim().to_owned())
            .is_some()
        {
            return Err(format!("duplicate schema declaration {path}"));
        }
    }
    Ok(entries)
}

fn sections(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
                .map(str::to_owned)
        })
        .collect()
}

fn unquote(value: &str) -> Result<String, String> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| format!("expected quoted value, got {value}"))
}

fn quoted_list(value: &str) -> Result<Vec<String>, String> {
    let inner = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| format!("expected list, got {value}"))?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    inner.split(',').map(|item| unquote(item.trim())).collect()
}

fn required<'a>(entries: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    entries
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| format!("missing {key}"))
}

fn read_schema(schema_root: &Path) -> Result<Model, String> {
    let root_source = fs::read_to_string(schema_root.join("v1.kvx"))
        .map_err(|error| format!("read v1.kvx: {error}"))?;
    let root = section_map(&root_source)?;
    let includes = quoted_list(required(&root, "schema.includes")?)?;
    let mut module_sources = Vec::new();
    let mut module_entries = Vec::new();
    for include in &includes {
        let source = fs::read_to_string(schema_root.join(include))
            .map_err(|error| format!("read {include}: {error}"))?;
        module_entries.push(section_map(&source)?);
        module_sources.push(source);
    }

    let scalar_names = sections(&root_source)
        .into_iter()
        .filter_map(|section| section.strip_prefix("scalar.").map(str::to_owned))
        .collect::<Vec<_>>();
    let mut scalars = Vec::new();
    for name in scalar_names {
        let prefix = format!("scalar.{name}");
        scalars.push(Scalar {
            name,
            rust: unquote(required(&root, &format!("{prefix}.rust"))?)?,
            typescript: unquote(required(&root, &format!("{prefix}.typescript"))?)?,
            python: unquote(required(&root, &format!("{prefix}.python"))?)?,
            wire: unquote(required(&root, &format!("{prefix}.wire"))?)?,
            consensus_integer: required(&root, &format!("{prefix}.consensus_integer"))? == "true",
        });
    }

    let operations = module_sources
        .iter()
        .flat_map(|source| sections(source))
        .filter_map(|section| section.strip_prefix("operation.").map(str::to_owned))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let combined = module_entries
        .iter()
        .flat_map(|entries| {
            entries
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let levels = quoted_list(required(&combined, "type.Level.variants")?)?;
    let errors = quoted_list(required(&combined, "type.ErrorClass.variants")?)?;
    let protocol = unquote(required(
        &combined,
        "type.BudgetEnforcement.ProtocolBudget.guarantee",
    )?)?;
    let daemon = unquote(required(
        &combined,
        "type.BudgetEnforcement.DaemonLimit.guarantee",
    )?)?;
    let notice = unquote(required(
        &combined,
        "type.BudgetEnforcement.DaemonLimit.notice",
    )?)?;
    let model = Model {
        scalars,
        operations,
        levels,
        errors,
        guarantees: vec![
            (
                "ProtocolBudget".to_owned(),
                protocol,
                "Enforced by the LayerX protocol state machine.".to_owned(),
            ),
            ("DaemonLimit".to_owned(), daemon, notice),
        ],
    };
    validate_model(&model)?;
    Ok(model)
}

fn validate_model(model: &Model) -> Result<(), String> {
    if model.scalars.is_empty() || model.operations.is_empty() {
        return Err("schema has no SDK surface".to_owned());
    }
    for scalar in &model.scalars {
        if !scalar.consensus_integer {
            continue;
        }
        if scalar.typescript != "bigint"
            || scalar.python != "int"
            || !matches!(scalar.rust.as_str(), "u8" | "u16" | "u32" | "u64" | "u128")
            || scalar.wire != "decimal_string"
        {
            return Err(format!(
                "consensus integer {} has a lossy language boundary",
                scalar.name
            ));
        }
    }
    let daemon = model
        .guarantees
        .iter()
        .find(|(name, _, _)| name == "DaemonLimit")
        .ok_or_else(|| "missing DaemonLimit guarantee".to_owned())?;
    if daemon.1 != "daemon_enforced"
        || !daemon
            .2
            .contains("Bypassing the daemon bypasses this limit")
        || daemon.2.contains("protocol-enforced")
    {
        return Err("daemon-only restriction is overstated".to_owned());
    }
    Ok(())
}

fn integer_bound(rust: &str) -> Result<&'static str, String> {
    match rust {
        "u8" => Ok("255"),
        "u16" => Ok("65535"),
        "u32" => Ok("4294967295"),
        "u64" => Ok("18446744073709551615"),
        "u128" => Ok("340282366920938463463374607431768211455"),
        _ => Err(format!("unsupported exact integer {rust}")),
    }
}

fn snake_case(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() && index != 0 {
            output.push('_');
        }
        output.push(character.to_ascii_lowercase());
    }
    output
}

fn typescript(model: &Model) -> Result<String, String> {
    let mut scalars = String::new();
    for scalar in &model.scalars {
        let bound = integer_bound(&scalar.rust)?;
        write!(
            &mut scalars,
            "export type {0} = bigint;\nexport function parse{0}(value: string): {0} {{\n  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new RangeError(\"invalid {0}\");\n  const parsed = BigInt(value);\n  if (parsed > {1}n) throw new RangeError(\"{0} out of range\");\n  return parsed;\n}}\n",
            scalar.name, bound
        )
        .map_err(|_| "format TypeScript scalar".to_owned())?;
    }
    let levels = model
        .levels
        .iter()
        .enumerate()
        .map(|(rank, level)| format!("  {level} = {rank},"))
        .collect::<Vec<_>>()
        .join("\n");
    let errors = model
        .errors
        .iter()
        .map(|error| format!("\"{error}\""))
        .collect::<Vec<_>>()
        .join(" | ");
    let operations = model
        .operations
        .iter()
        .map(|operation| format!("\"{operation}\""))
        .collect::<Vec<_>>()
        .join(" | ");
    Ok(TYPESCRIPT_TEMPLATE
        .replace("{{SCALARS}}", scalars.trim_end())
        .replace("{{LEVELS}}", &levels)
        .replace("{{ERRORS}}", &errors)
        .replace("{{OPERATIONS}}", &operations))
}

fn python(model: &Model) -> Result<String, String> {
    let mut scalars = String::new();
    for scalar in &model.scalars {
        let bound = integer_bound(&scalar.rust)?;
        let function = snake_case(&scalar.name);
        write!(
            &mut scalars,
            "{0} = int\n\ndef parse_{1}(value: str) -> {0}:\n    if not value or (value != \"0\" and value.startswith(\"0\")) or not value.isascii() or not value.isdigit():\n        raise ValueError(\"invalid {0}\")\n    parsed = int(value)\n    if parsed > {2}:\n        raise OverflowError(\"{0} out of range\")\n    return parsed\n\n",
            scalar.name, function, bound
        )
        .map_err(|_| "format Python scalar".to_owned())?;
    }
    let levels = model
        .levels
        .iter()
        .enumerate()
        .map(|(rank, level)| format!("    {} = {rank}", snake_case(level).to_ascii_uppercase()))
        .collect::<Vec<_>>()
        .join("\n");
    let errors = model
        .errors
        .iter()
        .map(|error| format!("\"{error}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let operations = model
        .operations
        .iter()
        .map(|operation| format!("\"{operation}\""))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(PYTHON_TEMPLATE
        .replace("{{SCALARS}}", scalars.trim_end())
        .replace("{{LEVELS}}", &levels)
        .replace("{{ERRORS}}", &errors)
        .replace("{{OPERATIONS}}", &operations))
}

fn guarantees(model: &Model) -> String {
    let rows = model
        .guarantees
        .iter()
        .map(|(name, enforcement, statement)| {
            format!("| `{name}` | `{enforcement}` | {statement} |")
        })
        .collect::<Vec<_>>()
        .join("\n");
    GUARANTEES_TEMPLATE.replace("{{GUARANTEES}}", &rows)
}

/// Generates both language SDKs and their guarantee documentation from one model.
///
/// # Errors
///
/// Refuses malformed schema, lossy integer mappings and overstated guarantees.
pub fn agent_sdk_generator(schema_root: &Path) -> Result<Generated, String> {
    let model = read_schema(schema_root)?;
    let documentation = guarantees(&model);
    Ok(Generated {
        files: BTreeMap::from([
            (
                PathBuf::from("typescript/src/generated/client.ts"),
                typescript(&model)?,
            ),
            (
                PathBuf::from("typescript/src/generated/guarantees.md"),
                documentation.clone(),
            ),
            (
                PathBuf::from("python/layerx_sdk/generated/client.py"),
                python(&model)?,
            ),
            (
                PathBuf::from("python/layerx_sdk/generated/guarantees.md"),
                documentation,
            ),
        ]),
    })
}

/// Compares every generated byte with the committed SDK tree.
///
/// # Errors
///
/// Names the first missing or hand-edited generated file.
pub fn agent_sdk_drift_gate(generated: &Generated, output_root: &Path) -> Result<(), String> {
    for (relative, expected) in &generated.files {
        let path = output_root.join(relative);
        let actual = fs::read_to_string(&path)
            .map_err(|error| format!("generated file missing {}: {error}", path.display()))?;
        if &actual != expected {
            return Err(format!("generated SDK drift at {}", path.display()));
        }
    }
    Ok(())
}

fn write_generated(generated: &Generated, output_root: &Path) -> Result<(), String> {
    for (relative, source) in &generated.files {
        let path = output_root.join(relative);
        let parent = path
            .parent()
            .ok_or_else(|| format!("generated path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
        fs::write(&path, source).map_err(|error| format!("write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let mode = arguments.first().map_or("--check", String::as_str);
    let schema = arguments
        .get(1)
        .map_or_else(|| PathBuf::from("agent/schema/agent-api"), PathBuf::from);
    let output = arguments
        .get(2)
        .map_or_else(|| PathBuf::from("agent/sdk"), PathBuf::from);
    let generated = agent_sdk_generator(&schema)?;
    match mode {
        "--write" => write_generated(&generated, &output),
        "--check" => agent_sdk_drift_gate(&generated, &output),
        _ => Err("usage: agent-sdk-gen [--write|--check] [schema-root] [output-root]".to_owned()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("agent-sdk-gen: {error}");
        std::process::exit(1);
    }
}
