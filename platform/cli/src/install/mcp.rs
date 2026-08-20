use layerx_mcp::server::DeploymentMode;
use serde_json::{json, Value};

use crate::config::Configuration;
use crate::toolset;

use super::{apply, executable, hosts, report, select, variables, Registration, SERVER_NAME};

pub struct Request {
    pub environment: Option<String>,
    pub hosts: Vec<String>,
    pub key: Option<String>,
    pub read_only: bool,
    pub token_stdin: bool,
}

/// Installs and registers a payment-capable `LayerX` model context protocol server.
pub fn platform_install_mcp(
    configuration: &mut Configuration,
    request: &Request,
) -> Result<Value, String> {
    let selection = select(
        configuration,
        request.environment.clone(),
        request.key.clone(),
        "mcp",
        request.token_stdin,
    )?;
    let mode = if request.read_only {
        DeploymentMode::ReadOnly
    } else {
        DeploymentMode::Full
    };
    let tools = toolset::surface(mode)?;
    let command = executable()?;
    let arguments = launch_arguments(&selection.environment, &selection.key, request.read_only);
    let variables = variables()?;
    let descriptors: Vec<Value> = tools.iter().copied().map(toolset::descriptor).collect();
    let mut registrations = Vec::new();
    let mut changed = false;
    for host in hosts(&request.hosts)? {
        let registration = Registration {
            path: host.path()?,
            section: host.section(),
            name: SERVER_NAME.to_owned(),
            entry: host.entry(&command, &arguments, &variables),
        };
        let outcome = apply(&registration)?;
        if outcome.changed {
            changed = true;
        }
        let mut record = report(
            &registration.path,
            registration.section,
            &registration.name,
            &outcome,
        );
        if let Some(fields) = record.as_object_mut() {
            fields.insert("host".to_owned(), json!(host.name()));
        }
        registrations.push(record);
    }
    if registrations.is_empty() {
        return Err("no agent runtime was selected for installation".into());
    }
    Ok(json!({
        "component": "mcp",
        "transport": "stdio",
        "environment": selection.environment,
        "endpoint": selection.endpoint,
        "network_id": selection.network_id,
        "deployment_mode": toolset::mode_name(mode),
        "server": {
            "name": SERVER_NAME,
            "command": command,
            "args": arguments,
            "env": variables,
        },
        "tools": descriptors,
        "scopes": toolset::scopes(&tools),
        "daemon_gates": toolset::gates(),
        "credentials": selection.credentials(),
        "registrations": registrations,
        "changed": changed,
        "idempotent": true,
    }))
}

fn launch_arguments(environment: &str, key: &str, read_only: bool) -> Vec<String> {
    let mut arguments = vec![
        "mcp".to_owned(),
        "serve".to_owned(),
        "--environment".to_owned(),
        environment.to_owned(),
        "--key".to_owned(),
        key.to_owned(),
    ];
    if read_only {
        arguments.push("--read-only".to_owned());
    }
    arguments
}
