use std::path::PathBuf;

use layerx_mcp::server::{DeploymentMode, ToolDefinition};
use serde_json::{json, Value};

use crate::config::Configuration;
use crate::toolset;

use super::{
    apply, executable, layerx_directory, publish, report, select, variables, Registration,
    SERVER_NAME,
};

pub const PROTOCOL_VERSION: &str = "0.3.0";
pub const ENVIRONMENT_EXTENSION: &str = "https://layerx.dev/a2a/extensions/environment/v1";

const CARD_FILE: &str = "agent-card.json";
const REGISTRY_FILE: &str = "agents.json";
const REGISTRY_SECTION: &str = "agents";

pub struct Request {
    pub environment: Option<String>,
    pub listen: String,
    pub key: Option<String>,
    pub well_known: Option<PathBuf>,
    pub read_only: bool,
    pub token_stdin: bool,
}

/// Installs and registers a payment-capable `LayerX` agent-to-agent server.
pub fn platform_install_a2a(
    configuration: &mut Configuration,
    request: &Request,
) -> Result<Value, String> {
    let address = crate::a2a::endpoint(&request.listen)?;
    let listen = address.to_string();
    let selection = select(
        configuration,
        request.environment.clone(),
        request.key.clone(),
        "a2a",
        request.token_stdin,
    )?;
    let mode = if request.read_only {
        DeploymentMode::ReadOnly
    } else {
        DeploymentMode::Full
    };
    let tools = toolset::surface(mode)?;
    let command = executable()?;
    let arguments = launch_arguments(
        &selection.environment,
        &selection.key,
        &listen,
        request.read_only,
    );
    let variables = variables()?;
    let directory = layerx_directory()?.join("a2a");
    let card_path = directory.join(CARD_FILE);
    let card = agent_card(&selection.environment, &listen, mode, &tools);
    let card_outcome = publish(&card_path, &card)?;
    let registration = Registration {
        path: directory.join(REGISTRY_FILE),
        section: REGISTRY_SECTION,
        name: SERVER_NAME.to_owned(),
        entry: json!({
            "url": endpoint_url(&listen),
            "transport": "JSONRPC",
            "card": card_path.display().to_string(),
            "command": command,
            "args": arguments,
            "env": variables,
            "environment": selection.environment,
        }),
    };
    let registry_outcome = apply(&registration)?;
    let descriptors: Vec<Value> = tools.iter().copied().map(toolset::descriptor).collect();
    let mut published: Vec<Value> = Vec::new();
    let mut changed = card_outcome.changed || registry_outcome.changed;
    if let Some(root) = &request.well_known {
        let path = root.join(".well-known").join(CARD_FILE);
        let outcome = publish(&path, &card)?;
        if outcome.changed {
            changed = true;
        }
        published.push(report(&path, "document", CARD_FILE, &outcome));
    }
    Ok(json!({
        "component": "a2a",
        "transport": "JSONRPC",
        "environment": selection.environment,
        "endpoint": selection.endpoint,
        "network_id": selection.network_id,
        "deployment_mode": toolset::mode_name(mode),
        "listen": listen,
        "url": endpoint_url(&listen),
        "server": {
            "name": SERVER_NAME,
            "command": command,
            "args": arguments,
            "env": variables,
        },
        "agent_card": report(&card_path, "document", CARD_FILE, &card_outcome),
        "registrations": [
            report(
                &registration.path,
                registration.section,
                &registration.name,
                &registry_outcome,
            ),
        ],
        "published": published,
        "skills": descriptors,
        "scopes": toolset::scopes(&tools),
        "daemon_gates": toolset::gates(),
        "credentials": selection.credentials(),
        "changed": changed,
        "idempotent": true,
    }))
}

pub fn agent_card(
    environment: &str,
    listen: &str,
    mode: DeploymentMode,
    tools: &[ToolDefinition],
) -> Value {
    let skills = tools
        .iter()
        .map(|tool| {
            json!({
                "id": tool.name,
                "name": tool.name,
                "description": toolset::description(tool.name),
                "tags": ["layerx", "payment", toolset::kind_name(tool.kind)],
                "inputModes": ["application/json"],
                "outputModes": ["application/json"],
            })
        })
        .collect::<Vec<_>>();
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "name": "LayerX",
        "description": "Receipt-verified LayerX payment agent bound to one environment and one key.",
        "url": endpoint_url(listen),
        "preferredTransport": "JSONRPC",
        "version": env!("CARGO_PKG_VERSION"),
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "stateTransitionHistory": false,
            "extensions": [
                {
                    "uri": ENVIRONMENT_EXTENSION,
                    "description": "Declares the bound LayerX environment, deployment mode and daemon gates.",
                    "required": false,
                    "params": {
                        "environment": environment,
                        "deployment_mode": toolset::mode_name(mode),
                        "scopes": toolset::scopes(tools),
                        "daemon_gates": toolset::gates(),
                    },
                },
            ],
        },
        "defaultInputModes": ["application/json"],
        "defaultOutputModes": ["application/json"],
        "skills": skills,
    })
}

pub fn endpoint_url(listen: &str) -> String {
    format!("http://{listen}/")
}

fn launch_arguments(environment: &str, key: &str, listen: &str, read_only: bool) -> Vec<String> {
    let mut arguments = vec![
        "a2a".to_owned(),
        "serve".to_owned(),
        "--environment".to_owned(),
        environment.to_owned(),
        "--key".to_owned(),
        key.to_owned(),
        "--listen".to_owned(),
        listen.to_owned(),
    ];
    if read_only {
        arguments.push("--read-only".to_owned());
    }
    arguments
}
