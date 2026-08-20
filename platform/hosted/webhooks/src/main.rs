//! Executable entry point of the hosted webhook service.
//!
//! Authentication is terminated by the hosted gateway in front, which forwards
//! the principal it authenticated in `x-layerx-principal`, so this process binds
//! a loopback address only. Outbound delivery runs on its own thread against the
//! real transport; setting the dispatch interval to zero leaves every attempt
//! under the operator's control, which is what an exercise that injects drops,
//! duplicates and reordering needs in order to stay deterministic.
//!
//! A payment event reaches the wire one of two ways: through the receipt-checked
//! path, where the caller hands over the gateway's own receipt bytes and the
//! event presents settlement at the level those bytes establish, or as an
//! explicitly unverified notification. A published fact that claims receipt
//! evidence on a payment without those bytes behind it is refused.

use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use layerx_platform_gateway::VerifiedOperation;
use layerx_platform_webhooks::deliveries::DeliveryRecord;
use layerx_platform_webhooks::encoding::base64_decode;
use layerx_platform_webhooks::endpoints::{EndpointHealth, RetryPolicy};
use layerx_platform_webhooks::error::WebhookError;
use layerx_platform_webhooks::events::{
    settled_payment, DeliveryId, EndpointId, EventDraft, EventId, EventKind, PaymentDraft,
    Principal, ProtocolEvent, ProtocolFact, SubjectId, Verification,
};
use layerx_platform_webhooks::http::{self, Reply, Request, PRINCIPAL_HEADER};
use layerx_platform_webhooks::scheme;
use layerx_platform_webhooks::service::{EndpointRequest, Service};
use layerx_platform_webhooks::transport::HttpTransport;
use serde::{Deserialize, Serialize};

const DEFAULT_LISTEN: &str = "127.0.0.1:9430";
const DEFAULT_ROOT: &str = "/var/lib/layerx-webhooks";
const DEFAULT_PAGE: usize = 50;
const DEFAULT_RETENTION_EVENTS: usize = 10_000;

struct Config {
    listen: String,
    root: PathBuf,
    policy: RetryPolicy,
    attempt_deadline: Duration,
    dispatch_interval: Duration,
    dispatch_budget: u32,
    key_overlap_seconds: u64,
}

#[derive(Deserialize)]
struct RegisterBody {
    url: String,
    #[serde(default)]
    kinds: Vec<String>,
    #[serde(default)]
    minimum_verification: Option<String>,
}

#[derive(Deserialize)]
struct SuspendBody {
    reason: String,
}

#[derive(Deserialize)]
struct FactBody {
    name: String,
    value: String,
    #[serde(default)]
    verification: Option<String>,
    #[serde(default)]
    receipt_digest: Option<String>,
}

#[derive(Deserialize)]
struct PaymentBody {
    amount: String,
    asset: String,
    #[serde(default)]
    response: String,
    receipt: String,
    verification_level: String,
}

#[derive(Deserialize)]
struct EventBody {
    #[serde(default)]
    id: Option<String>,
    kind: String,
    subject: String,
    subject_sequence: u64,
    #[serde(default)]
    occurred_at: Option<u64>,
    #[serde(default)]
    facts: Vec<FactBody>,
    #[serde(default)]
    payment: Option<PaymentBody>,
}

#[derive(Serialize)]
struct SchemeDocument {
    scheme: &'static str,
    algorithm: &'static str,
    signed_message: &'static str,
    signature_encoding: &'static str,
    id_header: &'static str,
    timestamp_header: &'static str,
    key_header: &'static str,
    signature_header: &'static str,
    signature_prefix: &'static str,
    metadata_headers: [&'static str; 6],
    tolerance_seconds: u64,
    maximum_future_skew_seconds: u64,
    public_key_environment_variable: &'static str,
    receiver_obligation: &'static str,
}

#[derive(Serialize)]
struct HealthPage {
    endpoints: Vec<EndpointHealth>,
}

#[derive(Serialize)]
struct DeliveryPage {
    deliveries: Vec<DeliveryRecord>,
}

#[derive(Serialize)]
struct EventLogPage {
    events: Vec<ProtocolEvent>,
}

#[derive(Serialize)]
struct ReplayOutcome {
    queued: String,
}

#[derive(Serialize)]
struct PruneOutcome {
    released: usize,
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

fn config() -> Result<Config, String> {
    let listen = env::var("LAYERX_WEBHOOKS_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_owned());
    if !http::loopback(&listen) {
        return Err("LAYERX_WEBHOOKS_LISTEN must be a loopback address".to_owned());
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
    let deadline = parse_u64("LAYERX_WEBHOOKS_ATTEMPT_DEADLINE_SECONDS", 15)?;
    if deadline == 0 {
        return Err("LAYERX_WEBHOOKS_ATTEMPT_DEADLINE_SECONDS must be positive".to_owned());
    }
    Ok(Config {
        listen,
        root: env::var("LAYERX_WEBHOOKS_STATE")
            .map_or_else(|_| PathBuf::from(DEFAULT_ROOT), PathBuf::from),
        policy,
        attempt_deadline: Duration::from_secs(deadline),
        dispatch_interval: Duration::from_secs(parse_u64(
            "LAYERX_WEBHOOKS_DISPATCH_INTERVAL_SECONDS",
            1,
        )?),
        dispatch_budget: parse_u32("LAYERX_WEBHOOKS_DISPATCH_BUDGET", 64)?,
        key_overlap_seconds: parse_u64("LAYERX_WEBHOOKS_KEY_OVERLAP_SECONDS", 86_400)?,
    })
}

fn refusal(error: &WebhookError) -> Reply {
    let (status, code, retry) = match error {
        WebhookError::InvalidRequest => (400, "invalid_request", None),
        WebhookError::UnknownEndpoint => (404, "unknown_endpoint", None),
        WebhookError::UnknownDelivery => (404, "unknown_delivery", None),
        WebhookError::NotDeadLettered => (409, "not_dead_lettered", None),
        WebhookError::EndpointSuspended => (409, "endpoint_suspended", None),
        WebhookError::EventConflict => (409, "event_conflict", None),
        WebhookError::OrderViolation => (409, "order_violation", None),
        WebhookError::InvalidCursor => (400, "invalid_cursor", None),
        WebhookError::CursorExpired => (410, "cursor_expired", None),
        WebhookError::VerificationRequired => (422, "verification_required", None),
        WebhookError::SignatureRejected => (401, "signature_rejected", None),
        WebhookError::ReplayRejected => (409, "replay_rejected", None),
        WebhookError::StaleTimestamp => (400, "stale_timestamp", None),
        WebhookError::ReplayCapacity => (503, "replay_capacity", Some(10)),
        WebhookError::Entropy => (503, "entropy_unavailable", Some(5)),
        WebhookError::CorruptStore | WebhookError::Unavailable | WebhookError::Io(_) => {
            (503, "state_unavailable", Some(10))
        }
        WebhookError::Gateway(_) => (403, "principal_refused", None),
    };
    Reply::refusal(status, code, retry)
}

fn encoded<T: Serialize>(status: u16, value: &T) -> Reply {
    serde_json::to_string(value).map_or_else(
        |_| Reply::refusal(503, "encoding_failed", Some(10)),
        |body| Reply::json(status, body),
    )
}

fn principal_of(request: &Request) -> Result<Principal, WebhookError> {
    Principal::new(
        request
            .header(PRINCIPAL_HEADER)
            .ok_or(WebhookError::InvalidRequest)?,
    )
}

fn body_of<T: for<'a> Deserialize<'a>>(request: &Request) -> Result<T, WebhookError> {
    serde_json::from_slice(&request.body).map_err(|_| WebhookError::InvalidRequest)
}

fn page_size(request: &Request) -> usize {
    request
        .parameter("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_PAGE)
        .clamp(1, 200)
}

fn scheme_document() -> SchemeDocument {
    SchemeDocument {
        scheme: scheme::SCHEME_VERSION,
        algorithm: "ed25519",
        signed_message:
            "\"<layerx-webhook-id>.<layerx-webhook-timestamp>.\" followed by the exact body bytes",
        signature_encoding: "standard padded base64 of the 64-byte signature",
        id_header: scheme::ID_HEADER,
        timestamp_header: scheme::TIMESTAMP_HEADER,
        key_header: scheme::KEY_HEADER,
        signature_header: scheme::SIGNATURE_HEADER,
        signature_prefix: scheme::SIGNATURE_PREFIX,
        metadata_headers: [
            scheme::DELIVERY_HEADER,
            scheme::KIND_HEADER,
            scheme::SUBJECT_HEADER,
            scheme::SEQUENCE_HEADER,
            scheme::ATTEMPT_HEADER,
            scheme::ENDPOINT_HEADER,
        ],
        tolerance_seconds: scheme::DEFAULT_TOLERANCE_SECONDS,
        maximum_future_skew_seconds: scheme::MAXIMUM_FUTURE_SKEW_SECONDS,
        public_key_environment_variable: "LAYERX_WEBHOOK_PUBLIC_KEYS_JSON",
        receiver_obligation: scheme::RECEIVER_OBLIGATION,
    }
}

fn draft_facts(facts: Vec<FactBody>) -> Result<Vec<ProtocolFact>, WebhookError> {
    facts
        .into_iter()
        .map(|fact| match (fact.verification, fact.receipt_digest) {
            (Some(level), Some(receipt)) => {
                ProtocolFact::verified(fact.name, fact.value, Verification::parse(&level)?, receipt)
            }
            (None, None) => ProtocolFact::unverified(fact.name, fact.value),
            _ => Err(WebhookError::VerificationRequired),
        })
        .collect()
}

fn payment_event(
    payment: PaymentBody,
    id: EventId,
    principal: &Principal,
    subject: SubjectId,
    subject_sequence: u64,
    occurred_at: u64,
) -> Result<ProtocolEvent, WebhookError> {
    let response = if payment.response.is_empty() {
        Vec::new()
    } else {
        base64_decode(&payment.response)?
    };
    let operation = VerifiedOperation {
        response,
        receipt: base64_decode(&payment.receipt)?,
        verification_level: payment.verification_level,
    };
    settled_payment(PaymentDraft {
        id,
        principal: principal.clone(),
        subject,
        subject_sequence,
        occurred_at,
        operation: &operation,
        amount: payment.amount,
        asset: payment.asset,
    })
}

fn build_event(
    body: EventBody,
    principal: &Principal,
    at: u64,
) -> Result<ProtocolEvent, WebhookError> {
    let id = match body.id {
        Some(value) => EventId::new(value)?,
        None => EventId::generate()?,
    };
    let kind = EventKind::parse(&body.kind)?;
    let subject = SubjectId::new(body.subject)?;
    let occurred_at = body.occurred_at.unwrap_or(at);
    if let Some(payment) = body.payment {
        if kind != EventKind::Payment || !body.facts.is_empty() {
            return Err(WebhookError::InvalidRequest);
        }
        return payment_event(
            payment,
            id,
            principal,
            subject,
            body.subject_sequence,
            occurred_at,
        );
    }
    let facts = draft_facts(body.facts)?;
    if kind == EventKind::Payment
        && facts
            .iter()
            .any(|fact| fact.verification().requires_receipt())
    {
        return Err(WebhookError::VerificationRequired);
    }
    ProtocolEvent::new(EventDraft {
        id,
        kind,
        principal: principal.clone(),
        subject,
        subject_sequence: body.subject_sequence,
        occurred_at,
        facts,
    })
}

fn register(
    service: &Service<HttpTransport>,
    request: &Request,
    principal: &Principal,
    at: u64,
) -> Reply {
    let body = match body_of::<RegisterBody>(request) {
        Ok(value) => value,
        Err(error) => return refusal(&error),
    };
    let kinds = body
        .kinds
        .iter()
        .map(|kind| EventKind::parse(kind.as_str()))
        .collect::<Result<Vec<EventKind>, WebhookError>>();
    let kinds = match kinds {
        Ok(value) => value,
        Err(error) => return refusal(&error),
    };
    let minimum = match body
        .minimum_verification
        .as_deref()
        .map(Verification::parse)
    {
        Some(Ok(value)) => value,
        Some(Err(error)) => return refusal(&error),
        None => Verification::Unverified,
    };
    service
        .register(
            &EndpointRequest {
                principal,
                url: &body.url,
                kinds: &kinds,
                minimum_verification: minimum,
            },
            at,
        )
        .map_or_else(|error| refusal(&error), |issued| encoded(201, &issued))
}

fn publish(
    service: &Service<HttpTransport>,
    request: &Request,
    principal: &Principal,
    at: u64,
) -> Reply {
    let event = body_of::<EventBody>(request)
        .and_then(|body| build_event(body, principal, at))
        .and_then(|event| service.publish(&event, at));
    event.map_or_else(|error| refusal(&error), |outcome| encoded(202, &outcome))
}

fn event_log(service: &Service<HttpTransport>, request: &Request, principal: &Principal) -> Reply {
    let kind = match request.parameter("kind").map(EventKind::parse) {
        Some(Ok(value)) => Some(value),
        Some(Err(error)) => return refusal(&error),
        None => None,
    };
    service
        .events(principal, kind, page_size(request))
        .map_or_else(
            |error| refusal(&error),
            |events| encoded(200, &EventLogPage { events }),
        )
}

fn delivery_log(
    service: &Service<HttpTransport>,
    request: &Request,
    principal: &Principal,
) -> Reply {
    let endpoint = match request.parameter("endpoint").map(EndpointId::new) {
        Some(Ok(value)) => Some(value),
        Some(Err(error)) => return refusal(&error),
        None => None,
    };
    service
        .deliveries(principal, endpoint.as_ref(), page_size(request))
        .map_or_else(
            |error| refusal(&error),
            |deliveries| encoded(200, &DeliveryPage { deliveries }),
        )
}

fn replay(
    service: &Service<HttpTransport>,
    principal: &Principal,
    delivery: &str,
    at: u64,
) -> Reply {
    DeliveryId::new(delivery)
        .and_then(|delivery| service.replay_dead_letter(principal, &delivery, at))
        .map_or_else(
            |error| refusal(&error),
            |queued| encoded(202, &ReplayOutcome { queued }),
        )
}

fn endpoint_route(
    service: &Service<HttpTransport>,
    request: &Request,
    principal: &Principal,
    endpoint: &str,
    action: &str,
    at: u64,
) -> Reply {
    let endpoint = match EndpointId::new(endpoint) {
        Ok(value) => value,
        Err(error) => return refusal(&error),
    };
    match (request.method.as_str(), action) {
        ("GET", "events") => service
            .events_since(
                principal,
                &endpoint,
                request.parameter("cursor"),
                page_size(request),
            )
            .map_or_else(|error| refusal(&error), |page| encoded(200, &page)),
        ("GET", "keys") => service
            .signing_keys(principal, &endpoint, at)
            .map_or_else(|error| refusal(&error), |keys| encoded(200, &keys)),
        ("POST", "keys") => service
            .rotate_key(principal, &endpoint, at)
            .map_or_else(|error| refusal(&error), |issued| encoded(201, &issued)),
        ("POST", "redeliveries") => service
            .redeliver(
                principal,
                &endpoint,
                request.parameter("cursor"),
                page_size(request),
                at,
            )
            .map_or_else(|error| refusal(&error), |outcome| encoded(202, &outcome)),
        ("POST", "suspensions") => body_of::<SuspendBody>(request)
            .and_then(|body| service.suspend(principal, &endpoint, &body.reason, at))
            .map_or_else(
                |error| refusal(&error),
                |()| Reply::json(200, "{\"suspended\":true}".to_owned()),
            ),
        ("POST", "resumptions") => service.resume(principal, &endpoint).map_or_else(
            |error| refusal(&error),
            |()| Reply::json(200, "{\"suspended\":false}".to_owned()),
        ),
        _ => Reply::refusal(404, "not_found", None),
    }
}

fn owned_route(
    service: &Service<HttpTransport>,
    request: &Request,
    principal: &Principal,
    tail: &[&str],
    at: u64,
) -> Reply {
    match (request.method.as_str(), tail) {
        ("POST", ["endpoints"]) => register(service, request, principal, at),
        ("GET", ["endpoints"]) => service.health(principal, at).map_or_else(
            |error| refusal(&error),
            |endpoints| encoded(200, &HealthPage { endpoints }),
        ),
        ("POST", ["events"]) => publish(service, request, principal, at),
        ("GET", ["events"]) => event_log(service, request, principal),
        ("GET", ["deliveries"]) => delivery_log(service, request, principal),
        ("GET", ["dead-letters"]) => service
            .dead_letters(principal, page_size(request))
            .map_or_else(
                |error| refusal(&error),
                |deliveries| encoded(200, &DeliveryPage { deliveries }),
            ),
        ("POST", ["deliveries", delivery, "replays"]) => replay(service, principal, delivery, at),
        (_, ["endpoints", endpoint, action]) => {
            endpoint_route(service, request, principal, endpoint, action, at)
        }
        _ => Reply::refusal(404, "not_found", None),
    }
}

fn route(service: &Service<HttpTransport>, request: &Request, budget: u32) -> Reply {
    let at = now();
    let segments = request.segments();
    if request.method == "GET" && matches!(segments.as_slice(), ["healthz"]) {
        return Reply::json(
            200,
            "{\"status\":\"ready\",\"service\":\"layerx-webhooks\"}".to_owned(),
        );
    }
    let ["v1", "webhooks", tail @ ..] = segments.as_slice() else {
        return Reply::refusal(404, "not_found", None);
    };
    match (request.method.as_str(), tail) {
        ("GET", ["scheme"]) => encoded(200, &scheme_document()),
        ("POST", ["dispatch"]) => service
            .dispatch(at, budget)
            .map_or_else(|error| refusal(&error), |report| encoded(200, &report)),
        ("POST", ["prune"]) => {
            let keep = request
                .parameter("keep")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(DEFAULT_RETENTION_EVENTS);
            service.prune(keep).map_or_else(
                |error| refusal(&error),
                |released| encoded(200, &PruneOutcome { released }),
            )
        }
        _ => match principal_of(request) {
            Ok(principal) => owned_route(service, request, &principal, tail, at),
            Err(error) => refusal(&error),
        },
    }
}

fn serve(config: &Config) -> Result<(), String> {
    let transport =
        HttpTransport::new(config.attempt_deadline).map_err(|_| "attempt deadline is unusable")?;
    let service = Arc::new(
        Service::open(&config.root, transport, config.policy)
            .map_err(|error| error.to_string())?
            .with_key_overlap(config.key_overlap_seconds),
    );
    let budget = config.dispatch_budget;
    let interval = config.dispatch_interval;
    if interval.is_zero() {
        eprintln!("LayerX webhooks dispatch is operator driven: POST /v1/webhooks/dispatch");
    } else {
        let dispatcher = Arc::clone(&service);
        thread::Builder::new()
            .name("layerx-webhooks-dispatch".to_owned())
            .spawn(move || loop {
                if let Err(error) = dispatcher.dispatch(now(), budget) {
                    eprintln!("webhook dispatch error: {error}");
                }
                thread::sleep(interval);
            })
            .map_err(|error| error.to_string())?;
    }
    eprintln!(
        "LayerX webhooks ready on {} with durable state {}",
        config.listen,
        config.root.display()
    );
    http::serve(&config.listen, |request| route(&service, request, budget))
        .map_err(|error| error.to_string())
}

fn main() {
    if let Err(error) = config().and_then(|config| serve(&config)) {
        eprintln!("layerx-webhooks: {error}");
        std::process::exit(2);
    }
}
