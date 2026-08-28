use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use std::thread;

use layerx_client::lni::transport::{ConnectionGate, Limits};
use serde_json::{json, Map, Value};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroize as _;

use crate::store::{AgentTenantId, PrincipalId};

use super::backend::{
    ApiFailure, HumanApiComponents, PrincipalContext, ScopedRequest, SessionCredentials,
};
use super::schema::{ApiSchema, Operation};

const PREFIX_BYTES: usize = 4;
const SOCKET_MODE: u32 = 0o660;

pub struct ComponentConfig {
    pub socket: PathBuf,
    pub peer_uid: u32,
    pub peer_gid: u32,
    pub limits: Limits,
    pub maintenance_interval: std::time::Duration,
    pub maintenance_maximum_items: usize,
}

impl ComponentConfig {
    fn validate(self) -> Result<Self, ApiFailure> {
        if !self.socket.is_absolute()
            || self.socket.as_os_str().is_empty()
            || self.peer_uid == u32::MAX
            || self.peer_gid == u32::MAX
            || self.maintenance_interval.is_zero()
            || self.maintenance_maximum_items == 0
        {
            return Err(ApiFailure::unavailable());
        }
        self.limits.validate().map_err(|_| ApiFailure::unavailable())?;
        let parent = self.socket.parent().ok_or_else(ApiFailure::unavailable)?;
        let metadata = fs::symlink_metadata(parent).map_err(|_| ApiFailure::unavailable())?;
        if !metadata.is_dir()
            || metadata.uid() != self.peer_uid
            || metadata.gid() != self.peer_gid
            || metadata.mode() & 0o022 != 0
        {
            return Err(ApiFailure::unavailable());
        }
        Ok(self)
    }
}

pub trait ComponentMaintenance: Send + Sync + 'static {
    fn maintain(&self, maximum_items: usize, now: u64) -> Result<usize, ApiFailure>;
    fn set_maintenance_health(&self, healthy: bool);
}

/// Privileged server for one concrete in-process human service graph.
pub struct HumanComponentServer<B: HumanApiComponents + ComponentMaintenance> {
    backend: Arc<B>,
    schema: Arc<ApiSchema>,
    config: ComponentConfig,
    gate: ConnectionGate,
}

impl<B: HumanApiComponents + ComponentMaintenance> HumanComponentServer<B> {
    pub fn new(backend: Arc<B>, config: ComponentConfig) -> Result<Self, ApiFailure> {
        let config = config.validate()?;
        Ok(Self {
            backend,
            schema: Arc::new(ApiSchema::v1().map_err(|_| ApiFailure::unavailable())?),
            gate: ConnectionGate::new(config.limits.maximum_connections),
            config,
        })
    }

    pub fn run(self) -> Result<(), ComponentServerError> {
        let initial_now = epoch_seconds()?;
        self.backend.maintain(self.config.maintenance_maximum_items, initial_now)
            .map_err(|_| ComponentServerError::Protocol)?;
        refuse_existing(&self.config.socket)?;
        let listener = UnixListener::bind(&self.config.socket).map_err(ComponentServerError::Io)?;
        fs::set_permissions(&self.config.socket, fs::Permissions::from_mode(SOCKET_MODE))
            .map_err(ComponentServerError::Io)?;
        validate_bound_path(&self.config.socket, self.config.peer_uid, self.config.peer_gid)?;
        let server = Arc::new(self);
        let maintenance = Arc::clone(&server);
        thread::spawn(move || {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                loop {
                    let Ok(now) = epoch_seconds() else { break };
                    if maintenance.backend.maintain(maintenance.config.maintenance_maximum_items, now).is_err() {
                        break;
                    }
                    thread::sleep(maintenance.config.maintenance_interval);
                }
            }));
            maintenance.backend.set_maintenance_health(false);
        });
        loop {
            let (stream, _) = listener.accept().map_err(ComponentServerError::Io)?;
            let permit = match server.gate.acquire() {
                Ok(permit) => permit,
                Err(_) => continue,
            };
            let server = Arc::clone(&server);
            thread::spawn(move || {
                let _permit = permit;
                let _ = server.serve_one(stream);
            });
        }
    }

    fn serve_one(&self, mut stream: UnixStream) -> Result<(), ComponentServerError> {
        authenticate_peer(&stream, self.config.peer_uid, self.config.peer_gid)?;
        stream.set_read_timeout(Some(self.config.limits.deadline)).map_err(ComponentServerError::Io)?;
        stream.set_write_timeout(Some(self.config.limits.deadline)).map_err(ComponentServerError::Io)?;
        let mut bytes = read_frame(&mut stream, self.config.limits.maximum_frame_bytes)?;
        let request = serde_json::from_slice::<Value>(&bytes);
        bytes.zeroize();
        let response = match request {
            Ok(mut request) => {
                let result = self.dispatch(&request);
                zeroize_value(&mut request);
                match result {
                    Ok(result) => result,
                    Err(failure) => json!({"ok": false, "error": component_error(&failure)}),
                }
            }
            Err(_) => {
                let failure = ApiFailure::invalid_request(None);
                json!({"ok": false, "error": component_error(&failure)})
            }
        };
        let mut encoded = serde_json::to_vec(&response)
            .map_err(|_| ComponentServerError::Protocol)?;
        let result = write_frame(&mut stream, &encoded, self.config.limits.maximum_frame_bytes);
        encoded.zeroize();
        result
    }

    fn dispatch(&self, request: &Value) -> Result<Value, ApiFailure> {
        let object = request.as_object().ok_or_else(|| ApiFailure::invalid_request(None))?;
        match text(object, "kind", 64)? {
            "session.authorize" => self.authorize(object),
            "human-api.execute" => self.execute(object),
            "readiness" => self.readiness(object),
            _ => Err(ApiFailure::not_found()),
        }
    }

    fn authorize(&self, object: &Map<String, Value>) -> Result<Value, ApiFailure> {
        exact_fields(object, &[
            "kind", "operation", "access_token", "csrf_token", "intended_destination",
            "refresh", "request_digest", "disclosure_digest", "trace",
            "path_parameters", "body", "idempotency_key",
        ])?;
        let operation = self.operation(text(object, "operation", 128)?)?;
        let trace = text(object, "trace", 255)?;
        let path_parameters = parse_parameters(object.get("path_parameters"))?;
        let body = object.get("body").ok_or_else(|| ApiFailure::invalid_request(Some("body")))?;
        let idempotency_key = optional_text(object, "idempotency_key", 255)?;
        let disclosure_digest = json_digest(body)?;
        let request_digest = authorized_request_digest(
            operation,
            text(object, "intended_destination", 2_048)?,
            &path_parameters,
            body,
            idempotency_key,
            trace,
        )?;
        if parse_digest(object, "request_digest")? != request_digest
            || parse_digest(object, "disclosure_digest")? != disclosure_digest
        {
            return Err(ApiFailure::unauthenticated());
        }
        let credentials = SessionCredentials {
            access_token: text(object, "access_token", 4_096)?,
            csrf_token: optional_text(object, "csrf_token", 4_096)?,
            intended_destination: text(object, "intended_destination", 2_048)?,
            refresh: object.get("refresh").and_then(Value::as_bool)
                .ok_or_else(|| ApiFailure::invalid_request(Some("refresh")))?,
            request_digest,
            disclosure_digest,
            path_parameters: &path_parameters,
            body,
            idempotency_key,
        };
        let context = self.backend.authorize(operation, credentials, trace)?;
        Ok(json!({
            "ok": true,
            "result": context_value(&context)
        }))
    }

    fn execute(&self, object: &Map<String, Value>) -> Result<Value, ApiFailure> {
        exact_fields(object, &[
            "kind", "component", "operation", "principal", "path_parameters", "body",
            "idempotency_key", "trace",
        ])?;
        let operation = self.operation(text(object, "operation", 128)?)?;
        let supplied_component = text(object, "component", 64)?;
        if supplied_component != super::backend::component_owner(&operation.name)? {
            return Err(ApiFailure::forbidden());
        }
        let trace = text(object, "trace", 255)?.to_owned();
        let principal = match object.get("principal") {
            None | Some(Value::Null) if operation.is_public_bootstrap() => None,
            Some(value) => Some(parse_context(value)?),
            _ => return Err(ApiFailure::unauthenticated()),
        };
        let path_parameters = parse_parameters(object.get("path_parameters"))?;
        let body = object.get("body").cloned().ok_or_else(|| ApiFailure::invalid_request(None))?;
        let idempotency_key = optional_text(object, "idempotency_key", 255)?.map(str::to_owned);
        let response = self.backend.execute(ScopedRequest {
            operation,
            principal,
            path_parameters,
            body,
            idempotency_key,
            trace,
        })?;
        let mut envelope = json!({"ok": true, "result": response.result});
        if let Some(session) = response.session {
            envelope.as_object_mut().ok_or_else(ApiFailure::upstream_degraded)?.insert(
                "session".to_owned(),
                json!({
                    "access_token": session.access_token,
                    "refresh_token": session.refresh_token,
                    "csrf_token": session.csrf_token,
                    "access_max_age_seconds": session.access_max_age_seconds,
                    "refresh_max_age_seconds": session.refresh_max_age_seconds
                }),
            );
        }
        Ok(envelope)
    }

    fn readiness(&self, object: &Map<String, Value>) -> Result<Value, ApiFailure> {
        exact_fields(object, &["kind", "trace"])?;
        let readiness = self.backend.readiness(text(object, "trace", 255)?)?;
        Ok(json!({
            "ok": true,
            "result": {
                "human_service": readiness.human_service.as_str(),
                "custody": readiness.custody.as_str(),
                "agent": readiness.agent.as_str(),
                "core": readiness.core.as_str(),
                "paxeer": readiness.paxeer.as_str()
            }
        }))
    }

    fn operation(&self, name: &str) -> Result<&Operation, ApiFailure> {
        self.schema.operations().iter().find(|operation| operation.name == name)
            .ok_or_else(ApiFailure::not_found)
    }
}

fn epoch_seconds() -> Result<u64, ComponentServerError> {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
        .map(|value| value.as_secs()).map_err(|_| ComponentServerError::Protocol)
}

fn json_digest(value: &Value) -> Result<[u8; 32], ApiFailure> {
    let encoded = serde_json::to_vec(value).map_err(|_| ApiFailure::invalid_request(None))?;
    Ok(Sha256::digest(encoded).into())
}

fn authorized_request_digest(
    operation: &Operation,
    destination: &str,
    path_parameters: &BTreeMap<String, String>,
    body: &Value,
    idempotency_key: Option<&str>,
    trace: &str,
) -> Result<[u8; 32], ApiFailure> {
    let mut digest = Sha256::new();
    digest.update(b"layerx-human/authorized-operation/v1\0");
    digest_field(&mut digest, operation.name.as_bytes());
    digest_field(&mut digest, operation.method.as_bytes());
    digest_field(&mut digest, destination.as_bytes());
    for (name, value) in path_parameters {
        digest_field(&mut digest, name.as_bytes());
        digest_field(&mut digest, value.as_bytes());
    }
    let body = serde_json::to_vec(body).map_err(|_| ApiFailure::invalid_request(None))?;
    digest_field(&mut digest, &body);
    digest_field(&mut digest, idempotency_key.unwrap_or_default().as_bytes());
    digest_field(&mut digest, trace.as_bytes());
    Ok(digest.finalize().into())
}

fn digest_field(digest: &mut Sha256, value: &[u8]) {
    digest.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    digest.update(value);
}

fn context_value(context: &PrincipalContext) -> Value {
    json!({
        "principal_id": context.principal.as_str(), "tenant_id": context.tenant.as_str(),
        "session_id": context.session_id.as_str(), "capability": context.capability(),
        "request_digest": hex(&context.request_digest()),
        "disclosure_digest": hex(&context.disclosure_digest()),
        "operation": context.operation(), "destination": context.destination(),
        "trace": context.trace(), "issued_at": context.issued_at(), "expires_at": context.expires_at()
        ,"refresh_token": context.refresh_credentials().map(|value| value.0),
        "refresh_csrf": context.refresh_credentials().map(|value| value.1)
    })
}

fn parse_context(value: &Value) -> Result<PrincipalContext, ApiFailure> {
    let object = value.as_object().ok_or_else(ApiFailure::unauthenticated)?;
    exact_fields(object, &[
        "principal_id", "tenant_id", "session_id", "capability", "request_digest",
        "disclosure_digest", "operation", "destination", "trace", "issued_at", "expires_at",
        "refresh_token", "refresh_csrf",
    ])?;
    let context = PrincipalContext::authorized(
        PrincipalId::new(text(object, "principal_id", 128)?).map_err(|_| ApiFailure::unauthenticated())?,
        AgentTenantId::new(text(object, "tenant_id", 128)?).map_err(|_| ApiFailure::unauthenticated())?,
        text(object, "session_id", 255)?.to_owned(), text(object, "capability", 4_096)?.to_owned(),
        parse_digest(object, "request_digest")?, parse_digest(object, "disclosure_digest")?,
        text(object, "operation", 128)?.to_owned(), text(object, "destination", 2_048)?.to_owned(),
        text(object, "trace", 255)?.to_owned(),
        object.get("issued_at").and_then(Value::as_u64).ok_or_else(ApiFailure::unauthenticated)?,
        object.get("expires_at").and_then(Value::as_u64).ok_or_else(ApiFailure::unauthenticated)?,
    )?;
    match (optional_text(object, "refresh_token", 4_096)?, optional_text(object, "refresh_csrf", 4_096)?) {
        (Some(token), Some(csrf)) => context.with_refresh(token.to_owned(), csrf.to_owned()),
        (None, None) => Ok(context),
        _ => Err(ApiFailure::unauthenticated()),
    }
}

fn parse_parameters(value: Option<&Value>) -> Result<BTreeMap<String, String>, ApiFailure> {
    let object = value.and_then(Value::as_object)
        .ok_or_else(|| ApiFailure::invalid_request(Some("path_parameters")))?;
    if object.len() > 16 { return Err(ApiFailure::invalid_request(Some("path_parameters"))); }
    object.iter().map(|(name, value)| {
        let value = value.as_str().filter(|value| value.len() <= 512)
            .ok_or_else(|| ApiFailure::invalid_request(Some("path_parameters")))?;
        Ok((name.clone(), value.to_owned()))
    }).collect()
}

fn exact_fields(object: &Map<String, Value>, fields: &[&str]) -> Result<(), ApiFailure> {
    if object.len() != fields.len() || object.keys().any(|key| !fields.contains(&key.as_str())) {
        Err(ApiFailure::invalid_request(None))
    } else { Ok(()) }
}

fn text<'a>(object: &'a Map<String, Value>, field: &str, maximum: usize) -> Result<&'a str, ApiFailure> {
    object.get(field).and_then(Value::as_str).filter(|value| !value.is_empty() && value.len() <= maximum)
        .ok_or_else(|| ApiFailure::invalid_request(Some(field)))
}

fn optional_text<'a>(object: &'a Map<String, Value>, field: &str, maximum: usize) -> Result<Option<&'a str>, ApiFailure> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() && value.len() <= maximum => Ok(Some(value)),
        _ => Err(ApiFailure::invalid_request(Some(field))),
    }
}

fn parse_digest(object: &Map<String, Value>, field: &str) -> Result<[u8; 32], ApiFailure> {
    let value = text(object, field, 64)?;
    if value.len() != 64 { return Err(ApiFailure::invalid_request(Some(field))); }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = nibble(pair[0]).ok_or_else(|| ApiFailure::invalid_request(Some(field)))?;
        let low = nibble(pair[1]).ok_or_else(|| ApiFailure::invalid_request(Some(field)))?;
        digest[index] = high << 4 | low;
    }
    Ok(digest)
}

fn nibble(value: u8) -> Option<u8> { match value { b'0'..=b'9' => Some(value-b'0'), b'a'..=b'f' => Some(value-b'a'+10), _ => None } }
fn hex(value: &[u8; 32]) -> String { value.iter().fold(String::with_capacity(64), |mut out, byte| { use std::fmt::Write as _; let _ = write!(out, "{byte:02x}"); out }) }

fn component_error(failure: &ApiFailure) -> Value {
    let mut value = failure.envelope();
    if let Some(object) = value.as_object_mut() { object.insert("status".to_owned(), Value::from(failure.status)); }
    value
}

fn authenticate_peer(stream: &UnixStream, uid: u32, gid: u32) -> Result<(), ComponentServerError> {
    let credentials = rustix::net::sockopt::socket_peercred(stream)
        .map_err(|_| ComponentServerError::PeerAuthentication)?;
    if credentials.uid.as_raw() == uid && credentials.gid.as_raw() == gid { Ok(()) }
    else { Err(ComponentServerError::PeerAuthentication) }
}

fn validate_bound_path(path: &Path, uid: u32, gid: u32) -> Result<(), ComponentServerError> {
    let metadata = fs::symlink_metadata(path).map_err(ComponentServerError::Io)?;
    if metadata.file_type().is_socket() && metadata.uid() == uid && metadata.gid() == gid
        && metadata.mode() & 0o7777 == SOCKET_MODE { Ok(()) }
    else { Err(ComponentServerError::PeerAuthentication) }
}

fn refuse_existing(path: &Path) -> Result<(), ComponentServerError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(ComponentServerError::SocketExists),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ComponentServerError::Io(error)),
    }
}

fn read_frame(stream: &mut UnixStream, maximum: usize) -> Result<Vec<u8>, ComponentServerError> {
    let mut prefix = [0_u8; PREFIX_BYTES]; stream.read_exact(&mut prefix).map_err(ComponentServerError::Io)?;
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > maximum { return Err(ComponentServerError::Frame); }
    let mut body = vec![0_u8; length]; stream.read_exact(&mut body).map_err(ComponentServerError::Io)?; Ok(body)
}
fn write_frame(stream: &mut UnixStream, body: &[u8], maximum: usize) -> Result<(), ComponentServerError> {
    if body.is_empty() || body.len() > maximum { return Err(ComponentServerError::Frame); }
    let length = u32::try_from(body.len()).map_err(|_| ComponentServerError::Frame)?;
    stream.write_all(&length.to_be_bytes()).map_err(ComponentServerError::Io)?;
    stream.write_all(body).map_err(ComponentServerError::Io)
}
fn zeroize_value(value: &mut Value) { match value { Value::String(value) => value.zeroize(), Value::Array(values) => values.iter_mut().for_each(zeroize_value), Value::Object(values) => values.values_mut().for_each(zeroize_value), Value::Null | Value::Bool(_) | Value::Number(_) => {} } }

#[derive(Debug)]
pub enum ComponentServerError { Io(io::Error), Frame, SocketExists, PeerAuthentication, Protocol }
impl std::fmt::Display for ComponentServerError { fn fmt(&self, f:&mut std::fmt::Formatter<'_>)->std::fmt::Result { match self { Self::Io(error)=>write!(f,"component server I/O failure: {error}"), Self::Frame=>f.write_str("component frame refused"), Self::SocketExists=>f.write_str("component socket already exists"), Self::PeerAuthentication=>f.write_str("component peer refused"), Self::Protocol=>f.write_str("component protocol failure") } } }
impl std::error::Error for ComponentServerError {}
