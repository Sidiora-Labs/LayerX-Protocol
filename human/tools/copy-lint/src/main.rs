use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_BANNED: &[&str] = &[
    "DID",
    "session key",
    "capability",
    "nullifier",
    "checkpoint",
    "payload",
    "canonical",
    "idempotency",
    "attestation",
    "proof",
];

const STATUS_MESSAGES: &[&str] = &[
    "Getting ready",
    "Sending",
    "Processing",
    "Done",
    "Done, finalised",
    "Still checking — don't send again",
    "Didn't go through",
    "Waiting for you",
    "Waiting for wallet",
    "Confirming on Paxeer",
    "Crediting",
    "Waiting for settlement",
    "Ready to claim",
    "Paid out",
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    key: String,
    message: String,
    context: String,
    surface: String,
    kind: String,
    money_adjacent: bool,
}

fn quoted_field(line: &str, field: &str) -> Option<String> {
    let marker = format!("{field}: \"");
    let start = line.find(&marker)? + marker.len();
    let remainder = &line[start..];
    let mut escaped = false;
    for (offset, character) in remainder.char_indices() {
        if character == '"' && !escaped {
            return Some(remainder[..offset].to_owned());
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    None
}

fn entries(source: &str) -> Vec<Entry> {
    source
        .lines()
        .filter(|line| line.trim_start().starts_with("{ key:"))
        .filter_map(|line| {
            Some(Entry {
                key: quoted_field(line, "key")?,
                message: quoted_field(line, "message")?,
                context: quoted_field(line, "context")?,
                surface: quoted_field(line, "surface")?,
                kind: quoted_field(line, "kind")?,
                money_adjacent: line.contains("moneyAdjacent: true"),
            })
        })
        .collect()
}

fn sentence_case(message: &str) -> bool {
    if message.starts_with('{') {
        return (message.contains("plural") || message.contains("select"))
            && message.contains("other {");
    }
    message
        .chars()
        .find(|character| character.is_alphabetic())
        .is_some_and(char::is_uppercase)
}

fn contains_term(message: &str, term: &str) -> bool {
    let message = message.to_lowercase();
    let term = term.to_lowercase();
    let mut offset = 0;
    while let Some(found) = message[offset..].find(&term) {
        let start = offset + found;
        let end = start + term.len();
        let before = message[..start].chars().next_back();
        let after = message[end..].chars().next();
        if before.is_none_or(|character| !character.is_alphanumeric())
            && after.is_none_or(|character| !character.is_alphanumeric())
        {
            return true;
        }
        offset = end;
    }
    false
}

fn catalog_violations(source: &str) -> Vec<String> {
    let mut violations = Vec::new();
    if !source.contains("COPY_CATALOG_VERSION = \"1.5.0\"") {
        violations.push("catalog-version".to_owned());
    }
    for term in REQUIRED_BANNED {
        if !source.contains(&format!("\"{term}\"")) {
            violations.push(format!("missing-banned-term:{term}"));
        }
    }
    if !source.contains("DATE_FORMAT = \"dd MMM yyyy, HH:mm z\"") {
        violations.push("date-format".to_owned());
    }
    if !source.contains("AMOUNT_FORMAT = \"{sign}{amount} {currencyCode}\"") {
        violations.push("amount-format".to_owned());
    }

    let parsed = entries(source);
    let required_label = parsed.iter().any(|entry| {
        entry.key == "ramp.external_custody.label"
            && entry.surface == "ramp"
            && entry.message == "External custody: this independent market maker controls the off-platform funds and payout."
    });
    if !required_label {
        violations.push("missing-ramp-external-custody-label".to_owned());
    }
    for entry in &parsed {
        if entry.key.starts_with("ramp.") && entry.surface != "ramp" {
            violations.push(format!("ramp-on-default-surface:{}", entry.key));
        }
        if entry.surface == "ramp" && !entry.key.starts_with("ramp.") {
            violations.push(format!("non-ramp-copy-on-ramp-surface:{}", entry.key));
        }
        if entry.key.starts_with("ramp.")
            && entry.message == "Done"
            && !entry.context.contains("verified LayerX receipt")
        {
            violations.push(format!("ramp-done-without-receipt-rule:{}", entry.key));
        }
        if entry.key.starts_with("ramp.") && contains_term(&entry.message, "LayerX balance") {
            violations.push(format!("ambiguous-ramp-balance-claim:{}", entry.key));
        }
        if entry.surface == "default" {
            for term in REQUIRED_BANNED {
                if contains_term(&entry.message, term) {
                    violations.push(format!("banned-default-copy:{term}"));
                }
            }
        }
        if !sentence_case(&entry.message) {
            violations.push(format!("sentence-case:{}", entry.message));
        }
        if entry.money_adjacent && entry.message.contains('!') {
            violations.push(format!("money-exclamation:{}", entry.message));
        }
        if entry.kind == "status" && !STATUS_MESSAGES.contains(&entry.message.as_str()) {
            violations.push(format!("unknown-status:{}", entry.message));
        }
    }
    for status in STATUS_MESSAGES {
        if !parsed
            .iter()
            .any(|entry| entry.kind == "status" && entry.message == *status)
        {
            violations.push(format!("missing-status:{status}"));
        }
    }
    violations
}

fn visit_sources(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in
        fs::read_dir(root).map_err(|error| format!("cannot read {}: {error}", root.display()))?
    {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_dir() {
            visit_sources(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "ts" || extension == "tsx")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn source_violations(source_root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if let Err(error) = visit_sources(source_root, &mut files) {
        return vec![error];
    }
    let mut violations = Vec::new();
    for path in files {
        if path.components().any(|part| part.as_os_str() == "copy") {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            violations.push(format!("unreadable-source:{}", path.display()));
            continue;
        };
        for (number, line) in source.lines().enumerate() {
            let has_quoted_fragment =
                line.contains('"') || line.contains('`') || line.contains('\'');
            if line.contains('+') && has_quoted_fragment {
                violations.push(format!(
                    "runtime-copy-concatenation:{}:{}",
                    path.display(),
                    number + 1
                ));
            }
            for attribute in ["aria-label=\"", "title=\"", "placeholder=\"", "alt=\""] {
                if line.contains(attribute) {
                    violations.push(format!(
                        "uncatalogued-copy:{}:{}",
                        path.display(),
                        number + 1
                    ));
                }
            }
            if path.extension().is_some_and(|extension| extension == "tsx") {
                let mut remainder = line;
                while let Some(opening) = remainder.find('>') {
                    remainder = &remainder[opening + 1..];
                    let Some(closing) = remainder.find('<') else {
                        break;
                    };
                    let content = remainder[..closing].trim();
                    if !(content.starts_with('{') && content.ends_with('}'))
                        && content.chars().any(char::is_alphabetic)
                    {
                        violations.push(format!(
                            "uncatalogued-jsx-copy:{}:{}",
                            path.display(),
                            number + 1
                        ));
                    }
                    remainder = &remainder[closing..];
                }
            }
        }
    }
    violations
}

fn human_copy_lint(web_root: &Path) -> Result<(), Vec<String>> {
    let catalog_path = web_root.join("copy/catalog.ts");
    let catalog = fs::read_to_string(&catalog_path)
        .map_err(|error| vec![format!("cannot read {}: {error}", catalog_path.display())])?;
    let mut violations = catalog_violations(&catalog);
    violations.extend(source_violations(&web_root.join("src")));
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn main() {
    let root = env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("human/apps/web"), PathBuf::from);
    match human_copy_lint(&root) {
        Ok(()) => println!("human copy catalog and source lint passed"),
        Err(violations) => {
            for violation in violations {
                eprintln!("{violation}");
            }
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{catalog_violations, entries, sentence_case};

    #[test]
    fn parses_catalog_entries() {
        let source = "  { key: \"status.done\", message: \"Done\", context: \"Receipt.\", surface: \"default\", kind: \"status\", moneyAdjacent: true },";
        assert_eq!(entries(source).len(), 1);
    }

    #[test]
    fn banned_default_vocabulary_is_rejected() {
        let source = "export const COPY_CATALOG_VERSION = \"1.0.0\";\n  { key: \"bad\", message: \"Canonical payload\", context: \"Bad.\", surface: \"default\", kind: \"body\", moneyAdjacent: false },";
        let violations = catalog_violations(source);
        assert!(violations
            .iter()
            .any(|violation| violation == "banned-default-copy:payload"));
    }

    #[test]
    fn technical_vocabulary_is_exempt() {
        let source = "  { key: \"technical.payload\", message: \"Canonical payload\", context: \"Technical.\", surface: \"technical\", kind: \"body\", moneyAdjacent: false },";
        let parsed = entries(source);
        assert_eq!(parsed[0].surface, "technical");
    }

    #[test]
    fn sentence_case_accepts_plain_and_icu_copy() {
        assert!(sentence_case("Getting ready"));
        assert!(sentence_case(
            "{count, plural, one {One item} other {Many items}}"
        ));
        assert!(!sentence_case("getting ready"));
    }

    fn ramp_catalog(entries: &str) -> String {
        format!(
            "export const COPY_CATALOG_VERSION = \"1.5.0\";\nexport const DATE_FORMAT = \"dd MMM yyyy, HH:mm z\";\nexport const AMOUNT_FORMAT = \"{{sign}}{{amount}} {{currencyCode}}\";\n{}\n  {{ key: \"ramp.external_custody.label\", message: \"External custody: this independent market maker controls the off-platform funds and payout.\", context: \"Required.\", surface: \"ramp\", kind: \"body\", moneyAdjacent: true }},\n{}",
            "\"DID\";\"session key\";\"capability\";\"nullifier\";\"checkpoint\";\"payload\";\"canonical\";\"idempotency\";\"attestation\";\"proof\";",
            entries,
        )
    }

    #[test]
    fn ramp_copy_on_default_surface_is_rejected() {
        let source = ramp_catalog("  { key: \"ramp.offer\", message: \"Market maker offer\", context: \"Offer.\", surface: \"default\", kind: \"body\", moneyAdjacent: true },");
        assert!(catalog_violations(&source)
            .iter()
            .any(|value| value == "ramp-on-default-surface:ramp.offer"));
    }

    #[test]
    fn ramp_done_without_verified_receipt_rule_is_rejected() {
        let source = ramp_catalog("  { key: \"ramp.status.done\", message: \"Done\", context: \"Provider said so.\", surface: \"ramp\", kind: \"status\", moneyAdjacent: true },");
        assert!(catalog_violations(&source)
            .iter()
            .any(|value| value == "ramp-done-without-receipt-rule:ramp.status.done"));
    }

    #[test]
    fn ambiguous_layerx_balance_claim_is_rejected() {
        let source = ramp_catalog("  { key: \"ramp.balance\", message: \"Your LayerX balance is held by the market maker\", context: \"Bad.\", surface: \"ramp\", kind: \"body\", moneyAdjacent: true },");
        assert!(catalog_violations(&source)
            .iter()
            .any(|value| value == "ambiguous-ramp-balance-claim:ramp.balance"));
    }
}
