//! The registry routes the developer CLI calls.
//!
//! Reading a program returns the registry projection only after the deployment
//! receipt behind its latest version has been re-verified against the
//! canonical journal. Verifying a program's source rebuilds mirrored source in
//! the pinned toolchain environment and compares the rebuilt artifact with the
//! registered on-chain code hash, so a mismatch is reported as a mismatch and
//! never as a verified source.

use std::collections::{BTreeMap, HashMap};

use layerx_programs::{
    hex, programs_source_verification, BuildPlan, BuildRefusal, DeploymentRecord,
    JournalReadAuthority, LifecycleReceipt, ObservedHead, ProgramId, ProgramLifecycle, Registry,
    RegistryError, RegistryVersion, ReproducibleBuild, SourceArchive, SourceStatus, SourceVerifier,
    UpgradePolicy, VerifiedRegistryRead, WindDownStateAccess,
};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use crate::builder::HermeticBuilder;
use crate::journal::FileDeploymentJournal;
use crate::mirror::{MirrorRefusal, SourceMirror};
use crate::verified::{VerifiedSource, VerifiedSourceStore};
use crate::Config;

const IDEMPOTENCY_DOMAIN: &[u8] = b"LayerX/platform/registry/idempotency/v1\0";
const REQUEST_DOMAIN: &[u8] = b"LayerX/platform/registry/source-request/v1\0";
const MAX_IDEMPOTENCY_RECORDS: usize = 4_096;
const ROUTE_PREFIX: &str = "/v1/programs/registry/";

/// One parsed request.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/// One rendered response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

#[derive(Clone, Debug)]
struct Completed {
    request_digest: [u8; 32],
    status: u16,
    body: String,
    at: u64,
}

/// The hosted registry. It owns the durable evidence the routes answer from
/// and never answers a read from its own projection alone.
pub struct Registrar {
    registry: Registry,
    journal: FileDeploymentJournal,
    mirror: SourceMirror,
    verified: VerifiedSourceStore,
    verifier: SourceVerifier<HermeticBuilder>,
    staleness_seconds: u64,
    idempotency: BTreeMap<String, Completed>,
}

impl Registrar {
    /// Opens every durable store and rebuilds the registry projection from the
    /// canonical journal and the completed rebuilds recorded beside it.
    ///
    /// # Errors
    ///
    /// Returns unusable directories, a corrupt journal, an inadmissible
    /// declared build environment and stored verifications that no longer
    /// decode.
    pub fn open(config: &Config) -> Result<Self, String> {
        let builder = HermeticBuilder::new(
            config.workspace.clone(),
            config.builder_image_digest,
            config.builder_path.clone(),
            config.build_timeout_seconds,
        )?;
        let verifier = SourceVerifier::new(builder, config.attempts)
            .map_err(|refused| format!("the build pipeline is not admissible: {refused}"))?;
        if config.staleness_seconds == 0 {
            return Err("a registry read freshness bound is required".to_owned());
        }
        let mut registrar = Self {
            registry: Registry::new(),
            journal: FileDeploymentJournal::open(config.journal.clone())?,
            mirror: SourceMirror::open(config.mirror.clone())?,
            verified: VerifiedSourceStore::open(config.verified.clone())?,
            verifier,
            staleness_seconds: config.staleness_seconds,
            idempotency: BTreeMap::new(),
        };
        registrar.rebuild()?;
        Ok(registrar)
    }

    /// Answers one request at the supplied wall-clock second.
    pub fn route(&mut self, request: &Request, now: u64) -> Response {
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/healthz") => Response {
                status: 200,
                body: json!({"status": "ready", "service": "program-registry"}).to_string(),
            },
            ("POST", "/__registry/deployments") => self.ingest_deployment(&request.body),
            ("POST", "/__registry/head") => self.ingest_head(&request.body),
            ("POST", "/__registry/sources") => self.ingest_source(&request.body),
            (
                _,
                "/healthz" | "/__registry/deployments" | "/__registry/head" | "/__registry/sources",
            ) => refusal(
                405,
                "method_not_allowed",
                "method is not supported for this route",
            ),
            _ => self.program_route(request, now),
        }
    }

    fn program_route(&mut self, request: &Request, now: u64) -> Response {
        let Some(rest) = request.path.strip_prefix(ROUTE_PREFIX) else {
            return refusal(404, "not_found", "route does not exist");
        };
        match rest.split_once('/') {
            None if request.method == "GET" => self.read(rest, now),
            Some((program, "source")) if request.method == "POST" => {
                self.verify(program, request, now)
            }
            None | Some((_, "source")) => refusal(
                405,
                "method_not_allowed",
                "method is not supported for this route",
            ),
            Some(_) => refusal(404, "not_found", "route does not exist"),
        }
    }

    fn read(&self, program: &str, now: u64) -> Response {
        let Some(program) = program_id(program) else {
            return refusal(
                400,
                "invalid_argument",
                "program id must be thirty-two hexadecimal-encoded bytes",
            );
        };
        let authority = match JournalReadAuthority::new(&self.journal, now, self.staleness_seconds)
        {
            Ok(authority) => authority,
            Err(error) => return refusal(503, "read_unverifiable", &error.to_string()),
        };
        match self.registry.read(program, &authority) {
            Ok(read) => Response {
                status: 200,
                body: registry_read_json(&read).to_string(),
            },
            Err(RegistryError::UnknownProgram | RegistryError::UnknownVersion) => {
                refusal(404, "not_found", "program is not registered")
            }
            Err(error @ RegistryError::StaleRead) => {
                refusal(503, "stale_read", &error.to_string())
            }
            Err(error) => refusal(502, "unverified_read", &error.to_string()),
        }
    }

    fn verify(&mut self, program: &str, request: &Request, now: u64) -> Response {
        let Some(program) = program_id(program) else {
            return refusal(
                400,
                "invalid_argument",
                "program id must be thirty-two hexadecimal-encoded bytes",
            );
        };
        let Some(key) = request.headers.get("idempotency-key") else {
            return refusal(
                400,
                "idempotency_key_required",
                "source verification requires an Idempotency-Key header",
            );
        };
        if !valid_idempotency_key(key) {
            return refusal(
                400,
                "invalid_argument",
                "idempotency key must be 16-128 ASCII letters, digits, dashes, or underscores",
            );
        }
        let Some((source_uri, source_digest)) = source_request(&request.body) else {
            return refusal(
                400,
                "invalid_argument",
                "request must carry source_uri and a thirty-two byte hexadecimal source_digest",
            );
        };
        let scoped = scoped_key(program, key);
        let digest = request_digest(program, &source_uri, &source_digest);
        if let Some(record) = self.idempotency.get(&scoped) {
            if record.request_digest != digest {
                return refusal(
                    409,
                    "idempotency_conflict",
                    "idempotency key was already used for a different request",
                );
            }
            return Response {
                status: record.status,
                body: record.body.clone(),
            };
        }
        let response = self.reproduce(program, &source_uri, source_digest);
        if response.status != 503 {
            self.remember(scoped, digest, &response, now);
        }
        response
    }

    fn reproduce(&mut self, program: ProgramId, uri: &str, source_digest: [u8; 32]) -> Response {
        let version = match self.registry.latest_version(program) {
            Ok(version) => version,
            Err(error) => return refusal(404, "not_found", &error.to_string()),
        };
        let mirrored = match self.mirror.fetch(uri, source_digest) {
            Ok(mirrored) => mirrored,
            Err(refused @ MirrorRefusal::NotMirrored) => {
                return refusal(404, "source_not_mirrored", &refused.to_string())
            }
            Err(refused) => return refusal(422, "source_unverifiable", &refused.to_string()),
        };
        let build = match self.verifier.reproduce(&mirrored.source, &mirrored.plan) {
            Ok(build) => build,
            Err(refused) => return rebuild_refusal(&refused),
        };
        let status = match self.registry.verify_source(program, version, &build) {
            Ok(status) => status,
            Err(error) => return refusal(404, "not_found", &error.to_string()),
        };
        if let Err(error) = self.verified.record(&VerifiedSource {
            program,
            version,
            source_uri: build.source_uri.clone(),
            source_digest: build.source_digest,
            artifact_digest: build.artifact_digest,
            plan: mirrored.plan.clone(),
        }) {
            return refusal(503, "persistence_unavailable", &error);
        }
        verification_response(program, version, &build, status)
    }

    fn remember(&mut self, scoped: String, request_digest: [u8; 32], response: &Response, now: u64) {
        if self.idempotency.len() >= MAX_IDEMPOTENCY_RECORDS {
            let oldest = self
                .idempotency
                .iter()
                .min_by_key(|(_, record)| record.at)
                .map(|(key, _)| key.clone());
            if let Some(key) = oldest {
                self.idempotency.remove(&key);
            }
        }
        self.idempotency.insert(
            scoped,
            Completed {
                request_digest,
                status: response.status,
                body: response.body.clone(),
                at: now,
            },
        );
    }

    fn rebuild(&mut self) -> Result<(), String> {
        let mut registry = Registry::new();
        registry
            .replay_journal(&self.journal.records()?)
            .map_err(|error| format!("the canonical deployment journal is not replayable: {error}"))?;
        for record in self.verified.records()? {
            let build = ReproducibleBuild::from_record(
                record.source_uri.clone(),
                record.source_digest,
                record.plan.environment.clone(),
                record.artifact_digest,
            )
            .map_err(|error| format!("a stored verification is not admissible: {error}"))?;
            match registry.verify_source(record.program, record.version, &build) {
                Ok(_) | Err(RegistryError::UnknownProgram | RegistryError::UnknownVersion) => {}
                Err(error) => {
                    return Err(format!("a stored verification is not replayable: {error}"))
                }
            }
        }
        self.registry = registry;
        Ok(())
    }

    fn ingest_deployment(&mut self, body: &[u8]) -> Response {
        let Some(bytes) = field(body, "record_hex").and_then(|text| hex::decode(&text).ok()) else {
            return refusal(
                400,
                "invalid_argument",
                "request must carry the canonical record as record_hex",
            );
        };
        let record = match DeploymentRecord::decode(&bytes) {
            Ok(record) => record,
            Err(error) => return refusal(400, "invalid_argument", &error.to_string()),
        };
        let digest = match self.journal.append(&record) {
            Ok(digest) => digest,
            Err(error) => return refusal(503, "persistence_unavailable", &error),
        };
        if let Err(error) = self.rebuild() {
            let mut rollback = self.journal.discard(digest).err();
            if rollback.is_none() {
                rollback = self.rebuild().err();
            }
            let detail = rollback.map_or_else(
                || format!("{error}; the journal entry was rolled back"),
                |failure| format!("{error}; the journal entry could not be rolled back: {failure}"),
            );
            return refusal(409, "registry_conflict", &detail);
        }
        Response {
            status: 200,
            body: json!({
                "recorded": true,
                "program_id": hex::encode(&record.program.bytes()),
                "version": record.version,
                "deployment_receipt_digest": hex::encode(&digest),
            })
            .to_string(),
        }
    }

    fn ingest_head(&mut self, body: &[u8]) -> Response {
        let Ok(document) = serde_json::from_slice::<Value>(body) else {
            return refusal(400, "invalid_argument", "request body is not JSON");
        };
        let (Some(sequence), Some(observed_at)) = (
            document["sequence"].as_u64(),
            document["observed_at"].as_u64(),
        ) else {
            return refusal(
                400,
                "invalid_argument",
                "request must carry sequence and observed_at",
            );
        };
        let head = ObservedHead {
            sequence,
            observed_at,
        };
        match self.journal.refresh_head(head) {
            Ok(()) => Response {
                status: 200,
                body: json!({"observed": true, "sequence": sequence, "observed_at": observed_at})
                    .to_string(),
            },
            Err(error) => refusal(400, "invalid_argument", &error),
        }
    }

    fn ingest_source(&mut self, body: &[u8]) -> Response {
        let Ok(document) = serde_json::from_slice::<Value>(body) else {
            return refusal(400, "invalid_argument", "request body is not JSON");
        };
        let (Some(uri), Some(plan), Some(archive)) = (
            document["source_uri"].as_str(),
            document["plan"].as_str(),
            document["archive_hex"]
                .as_str()
                .and_then(|text| hex::decode(text).ok()),
        ) else {
            return refusal(
                400,
                "invalid_argument",
                "request must carry source_uri, plan and archive_hex",
            );
        };
        let archive = match SourceArchive::decode(&archive) {
            Ok(archive) => archive,
            Err(error) => return refusal(400, "invalid_argument", &error.to_string()),
        };
        let plan = match BuildPlan::parse(plan) {
            Ok(plan) => plan,
            Err(error) => return refusal(400, "invalid_argument", &error.to_string()),
        };
        match self.mirror.publish(uri, &plan, &archive) {
            Ok(digest) => Response {
                status: 200,
                body: json!({
                    "mirrored": true,
                    "source_uri": uri,
                    "source_digest": hex::encode(&digest),
                })
                .to_string(),
            },
            Err(error) => refusal(400, "invalid_argument", &error),
        }
    }
}

/// Renders one refusal in the platform's refusal envelope.
#[must_use]
pub fn refusal(status: u16, code: &str, detail: &str) -> Response {
    Response {
        status,
        body: json!({"error": {"code": code, "retry": "never", "detail": detail}}).to_string(),
    }
}

fn rebuild_refusal(refused: &BuildRefusal) -> Response {
    match refused {
        BuildRefusal::SandboxUnavailable { reason } => {
            refusal(503, "builder_unavailable", reason)
        }
        BuildRefusal::BuilderFailed { reason } => refusal(422, "build_failed", reason),
        BuildRefusal::NondeterministicBuild { .. } => {
            refusal(422, "build_not_reproducible", &refused.to_string())
        }
        _ => refusal(422, "source_unverifiable", &refused.to_string()),
    }
}

fn verification_response(
    program: ProgramId,
    version: u32,
    build: &ReproducibleBuild,
    status: SourceStatus,
) -> Response {
    let verified = matches!(status, SourceStatus::Verified { .. });
    let outcome = json!({
        "program_id": hex::encode(&program.bytes()),
        "version": version,
        "source_uri": build.source_uri,
        "source_digest": hex::encode(&build.source_digest),
        "environment_digest": hex::encode(&build.environment_digest),
        "reproduced_artifact_digest": hex::encode(&build.artifact_digest),
        "source": source_json(status),
        "pipeline": programs_source_verification(),
    });
    if verified {
        return Response {
            status: 200,
            body: outcome.to_string(),
        };
    }
    Response {
        status: 409,
        body: json!({
            "error": {
                "code": "source_mismatch",
                "retry": "never",
                "detail": "the rebuilt artifact does not hash to the registered code hash",
            },
            "verification": outcome,
        })
        .to_string(),
    }
}

fn registry_read_json(read: &VerifiedRegistryRead) -> Value {
    json!({
        "program_id": hex::encode(&read.entry.program.bytes()),
        "upgrade_policy": policy_json(read.entry.upgrade_policy),
        "lifecycle": lifecycle_name(read.entry.lifecycle),
        "latest_version": read.entry.versions.last().map(|version| version.number),
        "versions": read
            .entry
            .versions
            .iter()
            .map(version_json)
            .collect::<Vec<Value>>(),
        "lifecycle_history": read
            .entry
            .lifecycle_history
            .iter()
            .map(lifecycle_json)
            .collect::<Vec<Value>>(),
        "receipt": {
            "deployment_receipt_digest": hex::encode(&read.receipt_digest),
            "observed_sequence": read.freshness.observed_sequence,
            "observed_at": read.freshness.observed_at,
            "verification": "receipt-verified",
        },
    })
}

fn version_json(version: &RegistryVersion) -> Value {
    json!({
        "version": version.number,
        "code_hash": hex::encode(&version.code_hash),
        "abi_version": version.abi_version,
        "deployment_receipt_digest": hex::encode(&version.deployment_receipt_digest),
        "source": source_json(version.source),
    })
}

fn source_json(status: SourceStatus) -> Value {
    match status {
        SourceStatus::Unpublished => json!({"status": "unpublished"}),
        SourceStatus::Verified {
            source_digest,
            environment_digest,
        } => json!({
            "status": "verified",
            "source_digest": hex::encode(&source_digest),
            "environment_digest": hex::encode(&environment_digest),
            "pipeline": programs_source_verification(),
        }),
        SourceStatus::Mismatch {
            expected,
            reproduced,
        } => json!({
            "status": "mismatch",
            "expected_code_hash": hex::encode(&expected),
            "reproduced_artifact_digest": hex::encode(&reproduced),
        }),
    }
}

fn lifecycle_json(receipt: &LifecycleReceipt) -> Value {
    let state_access = match receipt.wind_down.state_access {
        WindDownStateAccess::ReadOnly => "read-only",
    };
    json!({
        "prior": lifecycle_name(receipt.prior),
        "current": lifecycle_name(receipt.current),
        "authority": hex::encode(&receipt.authority),
        "effective_sequence": receipt.effective_sequence,
        "live_value_accounts": receipt.live_value_accounts,
        "wind_down": {
            "exit_program": hex::encode(&receipt.wind_down.exit_program),
            "deadline": receipt.wind_down.deadline,
            "state_access": state_access,
        },
    })
}

fn policy_json(policy: UpgradePolicy) -> Value {
    match policy {
        UpgradePolicy::Immutable => json!({"kind": "immutable"}),
        UpgradePolicy::Authority(authority) => {
            json!({"kind": "upgradeable", "authority": hex::encode(&authority)})
        }
    }
}

const fn lifecycle_name(lifecycle: ProgramLifecycle) -> &'static str {
    match lifecycle {
        ProgramLifecycle::Active => "active",
        ProgramLifecycle::Deprecated => "deprecated",
        ProgramLifecycle::Tombstoned => "tombstoned",
    }
}

fn program_id(text: &str) -> Option<ProgramId> {
    hex::decode_digest(text)
        .ok()
        .and_then(|bytes| ProgramId::new(bytes).ok())
}

fn source_request(body: &[u8]) -> Option<(String, [u8; 32])> {
    let document: Value = serde_json::from_slice(body).ok()?;
    let uri = document["source_uri"].as_str()?.to_owned();
    let digest = hex::decode_digest(document["source_digest"].as_str()?).ok()?;
    Some((uri, digest))
}

fn field(body: &[u8], name: &str) -> Option<String> {
    let document: Value = serde_json::from_slice(body).ok()?;
    Some(document[name].as_str()?.to_owned())
}

fn valid_idempotency_key(value: &str) -> bool {
    (16..=128).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn scoped_key(program: ProgramId, key: &str) -> String {
    let digest: [u8; 32] =
        Sha256::digest([IDEMPOTENCY_DOMAIN, &program.bytes(), b"\0", key.as_bytes()].concat())
            .into();
    hex::encode(&digest)
}

fn request_digest(program: ProgramId, uri: &str, source_digest: &[u8; 32]) -> [u8; 32] {
    Sha256::digest(
        [
            REQUEST_DOMAIN,
            &program.bytes(),
            b"\0",
            uri.as_bytes(),
            b"\0",
            source_digest,
        ]
        .concat(),
    )
    .into()
}
