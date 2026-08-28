use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use layerx_client::lni::transport::Limits;
use layerx_human_service::server::{
    ComponentConfig, HumanComponentServer, ProductionComponents, ProductionComponentsConfig,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("layerx-human-components refused startup: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let backend = Arc::new(ProductionComponents::open(ProductionComponentsConfig::from_environment()?)?);
    HumanComponentServer::new(
        backend,
        ComponentConfig {
            socket: PathBuf::from(required("LAYERX_HUMAN_COMPONENT_SOCKET")?),
            peer_uid: number("LAYERX_HUMAN_COMPONENT_UID")?,
            peer_gid: number("LAYERX_HUMAN_COMPONENT_GID")?,
            limits: Limits {
                maximum_frame_bytes: number("LAYERX_HUMAN_COMPONENT_MAX_FRAME_BYTES")?,
                maximum_connections: number("LAYERX_HUMAN_COMPONENT_MAX_CONNECTIONS")?,
                maximum_streams: 1,
                maximum_queued_bytes: number("LAYERX_HUMAN_COMPONENT_MAX_QUEUED_BYTES")?,
                deadline: Duration::from_secs(number("LAYERX_HUMAN_COMPONENT_DEADLINE_SECONDS")?),
            },
            maintenance_interval: Duration::from_secs(number("LAYERX_HUMAN_MAINTENANCE_INTERVAL_SECONDS")?),
            maintenance_maximum_items: number("LAYERX_HUMAN_MAINTENANCE_MAXIMUM_ITEMS")?,
        },
    )
    .map_err(|_| "invalid component listener configuration".to_owned())?
    .run()
    .map_err(|error| error.to_string())
}

fn required(name: &str) -> Result<String, String> {
    env::var(name).ok().filter(|value| !value.is_empty()).ok_or_else(|| format!("{name} is required"))
}

fn number<T: std::str::FromStr>(name: &str) -> Result<T, String> {
    required(name)?.parse().map_err(|_| format!("{name} is invalid"))
}
