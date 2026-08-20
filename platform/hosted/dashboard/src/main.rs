//! Executable entry point of the developer dashboard service.
//!
//! Authentication is terminated by the hosted gateway in front, which forwards
//! the principal it authenticated in `x-layerx-principal`, so this process binds
//! a loopback address only and every view is scoped to that principal. The
//! process opens both durable stores read-only: it never issues a key, never
//! dispatches a delivery and never writes a byte to either store.

use std::env;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use layerx_platform_dashboard::error::DashboardError;
use layerx_platform_dashboard::model::{KeyView, PaymentView, RequestRecord};
use layerx_platform_dashboard::service::Dashboard;
use layerx_platform_webhooks::deliveries::DeliveryRecord;
use layerx_platform_webhooks::endpoints::{EndpointHealth, RetryPolicy};
use layerx_platform_webhooks::error::WebhookError;
use layerx_platform_webhooks::events::{EndpointId, Principal};
use layerx_platform_webhooks::http::{self, Reply, Request, PRINCIPAL_HEADER};
use serde::Serialize;

const DEFAULT_LISTEN: &str = "127.0.0.1:9440";
const DEFAULT_GATEWAY_ROOT: &str = "/var/lib/layerx-gateway";
const DEFAULT_WEBHOOK_ROOT: &str = "/var/lib/layerx-webhooks";
const DEFAULT_PAGE: usize = 50;

struct Config {
    listen: String,
    gateway_root: PathBuf,
    webhook_root: PathBuf,
    policy: RetryPolicy,
}

#[derive(Serialize)]
struct KeyPage {
    keys: Vec<KeyView>,
}

#[derive(Serialize)]
struct RequestPage {
    requests: Vec<RequestRecord>,
}

#[derive(Serialize)]
struct EndpointPage {
    endpoints: Vec<EndpointHealth>,
}

#[derive(Serialize)]
struct DeliveryPage {
    deliveries: Vec<DeliveryRecord>,
}

#[derive(Serialize)]
struct PaymentPage {
    payments: Vec<PaymentView>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |value| value.as_secs())
}

fn parse_u64(name: &str, default: u64) -> Result<u64, String> {
    env::var(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|_| format!("{name} must be an integer"))
    })
}

fn parse_u32(name: &str, default: u32) -> Result<u32, String> {
    env::var(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|_| format!("{name} must be an integer"))
    })
}

fn parse_u8(name: &str, default: u8) -> Result<u8, String> {
    env::var(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|_| format!("{name} must be an integer"))
    })
}

fn parse_path(name: &str, default: &str) -> PathBuf {
    env::var(name).map_or_else(|_| PathBuf::from(default), PathBuf::from)
}

fn config() -> Result<Config, String> {
    let listen = env::var("LAYERX_DASHBOARD_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_owned());
    if !http::loopback(&listen) {
        return Err("LAYERX_DASHBOARD_LISTEN must be a loopback address".to_owned());
    }
    let policy = RetryPolicy {
        base_delay_seconds: parse_u64("LAYERX_WEBHOOKS_BASE_DELAY_SECONDS", 10)?,
        maximum_delay_seconds: parse_u64("LAYERX_WEBHOOKS_MAXIMUM_DELAY_SECONDS", 3_600)?,
        maximum_attempts: parse_u32("LAYERX_WEBHOOKS_MAXIMUM_ATTEMPTS", 8)?,
        spread_percent: parse_u8("LAYERX_WEBHOOKS_SPREAD_PERCENT", 20)?,
        suspend_after_dead_letters: parse_u32("LAYERX_WEBHOOKS_SUSPEND_AFTER_DEAD_LETTERS", 20)?,
        in_flight_timeout_seconds: parse_u64("LAYERX_WEBHOOKS_IN_FLIGHT_TIMEOUT_SECONDS", 120)?,
    }
    .validate()
    .map_err(|_| "webhook retry policy bounds are unusable".to_owned())?;
    Ok(Config {
        listen,
        gateway_root: parse_path("LAYERX_GATEWAY_STATE", DEFAULT_GATEWAY_ROOT),
        webhook_root: parse_path("LAYERX_WEBHOOKS_STATE", DEFAULT_WEBHOOK_ROOT),
        policy,
    })
}

fn webhook_refusal(error: &WebhookError) -> Reply {
    match error {
        WebhookError::Gateway(_) => Reply::refusal(403, "principal_refused", None),
        WebhookError::InvalidRequest => Reply::refusal(400, "invalid_request", None),
        WebhookError::UnknownEndpoint => Reply::refusal(404, "unknown_endpoint", None),
        _ => Reply::refusal(503, "state_unavailable", Some(10)),
    }
}

fn refusal(error: &DashboardError) -> Reply {
    match error {
        DashboardError::InvalidRequest => Reply::refusal(400, "invalid_request", None),
        DashboardError::UnknownReceipt => Reply::refusal(404, "unknown_receipt", None),
        DashboardError::UnknownRoot | DashboardError::CorruptStore | DashboardError::Io(_) => {
            Reply::refusal(503, "state_unavailable", Some(10))
        }
        DashboardError::Gateway(_) => Reply::refusal(403, "principal_refused", None),
        DashboardError::Webhooks(webhooks) => webhook_refusal(webhooks),
    }
}

fn encoded<T: Serialize>(value: &T) -> Reply {
    serde_json::to_string(value).map_or_else(
        |_| Reply::refusal(503, "encoding_failed", Some(10)),
        |body| Reply::json(200, body),
    )
}

fn answered<T: Serialize>(outcome: Result<T, DashboardError>) -> Reply {
    outcome.map_or_else(|error| refusal(&error), |value| encoded(&value))
}

fn principal_of(request: &Request) -> Result<Principal, DashboardError> {
    let header = request
        .header(PRINCIPAL_HEADER)
        .ok_or(DashboardError::InvalidRequest)?;
    Ok(Principal::new(header)?)
}

fn page_size(request: &Request) -> usize {
    request
        .parameter("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PAGE)
        .clamp(1, 200)
}

fn deliveries(dashboard: &Dashboard, request: &Request, principal: &Principal) -> Reply {
    let endpoint = match request.parameter("endpoint").map(EndpointId::new) {
        Some(Ok(value)) => Some(value),
        Some(Err(error)) => return webhook_refusal(&error),
        None => None,
    };
    answered(
        dashboard
            .deliveries(principal, endpoint.as_ref(), page_size(request))
            .map(|deliveries| DeliveryPage { deliveries }),
    )
}

fn owned_route(
    dashboard: &Dashboard,
    request: &Request,
    principal: &Principal,
    tail: &[&str],
    at: u64,
) -> Reply {
    match (request.method.as_str(), tail) {
        ("GET", ["overview"]) => answered(dashboard.overview(principal, at)),
        ("GET", ["keys"]) => answered(dashboard.keys(principal, at).map(|keys| KeyPage { keys })),
        ("GET", ["usage"]) => answered(dashboard.usage(principal, at)),
        ("GET", ["requests"]) => answered(
            dashboard
                .requests(principal, page_size(request))
                .map(|requests| RequestPage { requests }),
        ),
        ("GET", ["endpoints"]) => answered(
            dashboard
                .endpoints(principal, at)
                .map(|endpoints| EndpointPage { endpoints }),
        ),
        ("GET", ["deliveries"]) => deliveries(dashboard, request, principal),
        ("GET", ["dead-letters"]) => answered(
            dashboard
                .dead_letters(principal, page_size(request))
                .map(|deliveries| DeliveryPage { deliveries }),
        ),
        ("GET", ["payments"]) => answered(
            dashboard
                .payments(principal, page_size(request))
                .map(|payments| PaymentPage { payments }),
        ),
        ("GET", ["receipts", key]) => answered(dashboard.receipt(principal, key)),
        _ => Reply::refusal(404, "not_found", None),
    }
}

fn route(dashboard: &Dashboard, request: &Request) -> Reply {
    let at = now();
    let segments = request.segments();
    if request.method == "GET" && matches!(segments.as_slice(), ["healthz"]) {
        return Reply::json(
            200,
            "{\"status\":\"ready\",\"service\":\"layerx-dashboard\"}".to_owned(),
        );
    }
    let ["v1", "dashboard", tail @ ..] = segments.as_slice() else {
        return Reply::refusal(404, "not_found", None);
    };
    match principal_of(request) {
        Ok(principal) => owned_route(dashboard, request, &principal, tail, at),
        Err(error) => refusal(&error),
    }
}

fn serve(config: &Config) -> Result<(), String> {
    let dashboard = Dashboard::open(&config.gateway_root, &config.webhook_root, config.policy)
        .map_err(|error| {
            format!(
                "{error}: gateway state {} and webhook state {} must both exist",
                config.gateway_root.display(),
                config.webhook_root.display()
            )
        })?;
    eprintln!(
        "LayerX developer dashboard ready on {} over gateway state {} and webhook state {}",
        config.listen,
        config.gateway_root.display(),
        config.webhook_root.display()
    );
    http::serve(&config.listen, |request| route(&dashboard, request))
        .map_err(|error| error.to_string())
}

fn main() {
    if let Err(error) = config().and_then(|config| serve(&config)) {
        eprintln!("layerx-dashboard: {error}");
        std::process::exit(2);
    }
}
