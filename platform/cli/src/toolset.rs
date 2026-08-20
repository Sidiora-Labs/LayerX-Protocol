use std::collections::BTreeSet;

use layerx_mcp::server::{
    catalogue, DaemonGate, DeploymentMode, ToolDefinition, ToolKind, REQUIRED_DAEMON_GATES,
};
use serde_json::{json, Value};

use crate::account;
use crate::config::Configuration;
use crate::http::{validate_idempotency_key, validate_resource_id};

const MAX_ARGUMENT_BYTES: usize = 256;

const SERVED: [&str; 5] = [
    "balance.get",
    "receipt.get",
    "activity.prepare",
    "activity.submit",
    "activity.track",
];

pub fn surface(mode: DeploymentMode) -> Result<Vec<ToolDefinition>, String> {
    let mut tools = Vec::with_capacity(SERVED.len());
    for name in SERVED {
        let definition = catalogue()
            .iter()
            .find(|tool| tool.name == name)
            .copied()
            .ok_or_else(|| format!("tool {name} is absent from the protocol tool catalogue"))?;
        if mode == DeploymentMode::ReadOnly && definition.kind != ToolKind::Read {
            continue;
        }
        tools.push(definition);
    }
    if tools.is_empty() {
        return Err("the selected deployment mode would serve no tool".into());
    }
    Ok(tools)
}

pub fn scopes(tools: &[ToolDefinition]) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for tool in tools {
        unique.insert(tool.required_scope.to_owned());
    }
    unique.into_iter().collect()
}

pub fn gates() -> Vec<&'static str> {
    REQUIRED_DAEMON_GATES
        .iter()
        .map(|gate| match gate {
            DaemonGate::Policy => "policy",
            DaemonGate::Capability => "capability",
            DaemonGate::Budget => "budget",
            DaemonGate::RateLimit => "rate-limit",
            DaemonGate::Audit => "audit",
        })
        .collect()
}

pub const fn mode_name(mode: DeploymentMode) -> &'static str {
    match mode {
        DeploymentMode::Full => "full",
        DeploymentMode::ReadOnly => "read-only",
    }
}

pub const fn kind_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "read",
        ToolKind::Write => "write",
    }
}

pub fn descriptor(tool: ToolDefinition) -> Value {
    json!({
        "name": tool.name,
        "kind": kind_name(tool.kind),
        "scope": tool.required_scope,
        "mutation": tool.mutation,
        "evidence": tool.evidence,
        "arguments": schema(tool.name),
    })
}

pub fn description(name: &str) -> &'static str {
    match name {
        "balance.get" => "Read account balance material from the active LayerX environment.",
        "receipt.get" => "Fetch exact receipt material for one receipt identifier.",
        "activity.prepare" => {
            "Quote a payment against the active environment without committing it."
        }
        "activity.submit" => "Commit a quoted payment under a caller-supplied idempotency key.",
        "activity.track" => "Read the current state of one committed payment journey.",
        _ => "This tool is not served by this deployment.",
    }
}

pub fn schema(name: &str) -> Value {
    match name {
        "balance.get" => json!({
            "type": "object",
            "properties": {"did": {"type": "string"}},
            "required": [],
            "additionalProperties": false,
        }),
        "receipt.get" => json!({
            "type": "object",
            "properties": {"receipt_id": {"type": "string"}},
            "required": ["receipt_id"],
            "additionalProperties": false,
        }),
        "activity.prepare" => json!({
            "type": "object",
            "properties": {
                "source": {"type": "string"},
                "destination": {"type": "string"},
                "currency": {"type": "string"},
                "amount": {"type": "string"},
            },
            "required": ["source", "destination", "currency", "amount"],
            "additionalProperties": false,
        }),
        "activity.submit" => json!({
            "type": "object",
            "properties": {
                "quote_id": {"type": "string"},
                "idempotency_key": {"type": "string"},
            },
            "required": ["quote_id", "idempotency_key"],
            "additionalProperties": false,
        }),
        "activity.track" => json!({
            "type": "object",
            "properties": {"journey_id": {"type": "string"}},
            "required": ["journey_id"],
            "additionalProperties": false,
        }),
        _ => json!({"type": "object", "additionalProperties": false}),
    }
}

pub fn invoke(
    configuration: &Configuration,
    subject: Option<&str>,
    tool: ToolDefinition,
    arguments: &Value,
) -> Result<Value, String> {
    let (environment, client) = crate::active_client(configuration)?;
    match tool.name {
        "balance.get" => {
            let requested = optional_text(arguments, "did")?;
            account::get(&client, &environment, requested.as_deref().or(subject))
        }
        "receipt.get" => {
            let receipt = text(arguments, "receipt_id")?;
            validate_resource_id(&receipt, "receipt id")?;
            client.get(&format!("/v1/receipts/{receipt}"))
        }
        "activity.prepare" => {
            let source = text(arguments, "source")?;
            let destination = text(arguments, "destination")?;
            let currency = text(arguments, "currency")?;
            let amount = text(arguments, "amount")?;
            validate_amount(&amount)?;
            client.post(
                "/v1/moves/quote",
                &json!({
                    "source": source,
                    "destination": destination,
                    "money": {"currency": currency, "amount": amount},
                }),
                None,
            )
        }
        "activity.submit" => {
            let quote = text(arguments, "quote_id")?;
            let key = text(arguments, "idempotency_key")?;
            validate_idempotency_key(&key)?;
            client.post("/v1/moves", &json!({"quote_id": quote}), Some(&key))
        }
        "activity.track" => {
            let journey = text(arguments, "journey_id")?;
            validate_resource_id(&journey, "journey id")?;
            client.get(&format!("/v1/journeys/{journey}"))
        }
        _ => Err(format!(
            "tool {} is not served by this deployment",
            tool.name
        )),
    }
}

fn text(arguments: &Value, field: &str) -> Result<String, String> {
    let value = arguments
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("argument {field} must be a string"))?;
    if value.is_empty() || value.len() > MAX_ARGUMENT_BYTES {
        return Err(format!(
            "argument {field} must be 1-{MAX_ARGUMENT_BYTES} bytes"
        ));
    }
    Ok(value.to_owned())
}

fn optional_text(arguments: &Value, field: &str) -> Result<Option<String>, String> {
    match arguments.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => text(arguments, field).map(Some),
    }
}

fn validate_amount(value: &str) -> Result<(), String> {
    let amount = value
        .parse::<u128>()
        .map_err(|_| "amount must be an unsigned protocol integer".to_string())?;
    if amount == 0 {
        return Err("amount must be greater than zero".into());
    }
    Ok(())
}
