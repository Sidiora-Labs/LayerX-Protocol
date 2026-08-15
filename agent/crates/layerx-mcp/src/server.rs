//! One-tenant, one-scope-set MCP server with daemon-only routing.

use std::collections::BTreeSet;
use std::path::Path;

use layerx_agentd::audit::{AuditError, Log};
use layerx_agentd::capability::{Capability, CapabilityError, CapabilityId};
use layerx_agentd::identity::ProtocolAuthority;
use layerx_agentd::session::{SessionId, SessionRecord, SessionRegistry};
use layerx_agentd::store::{Store, TenantId};
use sha2::{Digest, Sha256};

const MAX_ARGUMENT_BYTES: usize = 1_048_576;
const MAX_TOOL_NAME_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolKind {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub kind: ToolKind,
    pub required_scope: &'static str,
    pub mutation: &'static str,
    pub evidence: &'static str,
}

const TOOL_CATALOGUE: [ToolDefinition; 11] = [
    ToolDefinition {
        name: "balance.get",
        kind: ToolKind::Read,
        required_scope: "read:balance",
        mutation: "none",
        evidence: "core bytes, verification level, freshness",
    },
    ToolDefinition {
        name: "history.list",
        kind: ToolKind::Read,
        required_scope: "read:history",
        mutation: "none",
        evidence: "core records, verification level, stable cursor, freshness",
    },
    ToolDefinition {
        name: "receipt.get",
        kind: ToolKind::Read,
        required_scope: "read:receipt",
        mutation: "none",
        evidence: "core receipt bytes and verification level",
    },
    ToolDefinition {
        name: "checkpoint.get",
        kind: ToolKind::Read,
        required_scope: "read:checkpoint",
        mutation: "none",
        evidence: "checkpoint certificate and verification level",
    },
    ToolDefinition {
        name: "proof.get",
        kind: ToolKind::Read,
        required_scope: "read:proof",
        mutation: "none",
        evidence: "proof bundle and verification level",
    },
    ToolDefinition {
        name: "availability.get",
        kind: ToolKind::Read,
        required_scope: "read:availability",
        mutation: "none",
        evidence: "verified chunks and attributed availability failures",
    },
    ToolDefinition {
        name: "activity.prepare",
        kind: ToolKind::Write,
        required_scope: "write:prepare",
        mutation: "daemon-local preparation only",
        evidence: "canonical bytes and bound disclosure",
    },
    ToolDefinition {
        name: "activity.disclose",
        kind: ToolKind::Write,
        required_scope: "write:disclose",
        mutation: "none",
        evidence: "disclosure decoded from canonical bytes",
    },
    ToolDefinition {
        name: "activity.sign",
        kind: ToolKind::Write,
        required_scope: "write:sign",
        mutation: "daemon-local signed preparation",
        evidence: "signature binding evidence",
    },
    ToolDefinition {
        name: "activity.submit",
        kind: ToolKind::Write,
        required_scope: "write:submit",
        mutation: "core submission through the ordinary daemon path",
        evidence: "verified receipt or honest non-terminal state",
    },
    ToolDefinition {
        name: "activity.track",
        kind: ToolKind::Write,
        required_scope: "write:track",
        mutation: "daemon-local receipt resolution state",
        evidence: "verified receipt or honest non-terminal state",
    },
];

#[must_use]
pub const fn catalogue() -> &'static [ToolDefinition] {
    &TOOL_CATALOGUE
}

/// Authority fixed for the lifetime of one server instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeBinding {
    tenant: TenantId,
    session_id: SessionId,
    capability_id: CapabilityId,
    scopes: BTreeSet<String>,
}

impl ScopeBinding {
    fn derive(
        session: &SessionRecord,
        capability: &Capability,
        core_sequence: u64,
    ) -> Result<Self, ServerError> {
        if !session.open {
            return Err(ServerError::ClosedSession);
        }
        if session.request.tenant != capability.tenant {
            return Err(ServerError::TenantMismatch);
        }
        if session.request.authority != ProtocolAuthority::CapabilityGrant(capability.id.0) {
            return Err(ServerError::CapabilityMismatch);
        }
        if core_sequence >= session.request.expiry_sequence
            || core_sequence >= capability.dimensions.expiry_sequence
        {
            return Err(ServerError::ExpiredAuthority);
        }
        if session.request.scopes.is_empty() {
            return Err(ServerError::NoScope);
        }
        let permitted = session
            .request
            .permitted_activity_types
            .iter()
            .all(|activity| capability.dimensions.activity_types.contains(activity));
        if !permitted {
            return Err(ServerError::CapabilityMismatch);
        }
        let scopes = session
            .request
            .scopes
            .iter()
            .filter(|scope| {
                TOOL_CATALOGUE
                    .iter()
                    .any(|tool| tool.required_scope == scope.as_str())
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        if scopes.is_empty() {
            return Err(ServerError::NoScope);
        }
        Ok(Self {
            tenant: session.request.tenant.clone(),
            session_id: session.request.session_id,
            capability_id: capability.id,
            scopes,
        })
    }

    #[must_use]
    pub const fn tenant(&self) -> &TenantId {
        &self.tenant
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    #[must_use]
    pub const fn capability_id(&self) -> CapabilityId {
        self.capability_id
    }

    #[must_use]
    pub const fn scopes(&self) -> &BTreeSet<String> {
        &self.scopes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DaemonGate {
    Policy,
    Capability,
    Budget,
    RateLimit,
    Audit,
}

pub const REQUIRED_DAEMON_GATES: [DaemonGate; 5] = [
    DaemonGate::Policy,
    DaemonGate::Capability,
    DaemonGate::Budget,
    DaemonGate::RateLimit,
    DaemonGate::Audit,
];

/// The only request produced by the MCP server. A daemon must consume it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonInvocation {
    server_binding: [u8; 32],
    invocation_id: u64,
    tool: ToolDefinition,
    arguments: Vec<u8>,
    arguments_digest: [u8; 32],
    gates: [DaemonGate; 5],
}

impl DaemonInvocation {
    #[must_use]
    pub const fn invocation_id(&self) -> u64 {
        self.invocation_id
    }

    #[must_use]
    pub const fn tool(&self) -> ToolDefinition {
        self.tool
    }

    #[must_use]
    pub fn arguments(&self) -> &[u8] {
        &self.arguments
    }

    #[must_use]
    pub const fn arguments_digest(&self) -> [u8; 32] {
        self.arguments_digest
    }

    #[must_use]
    pub const fn gates(&self) -> [DaemonGate; 5] {
        self.gates
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvocationOutcome {
    Completed,
    Refused,
    Unknown,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvocationRecord {
    pub invocation_id: u64,
    pub arguments_digest: [u8; 32],
    pub outcome: InvocationOutcome,
}

/// A server has no boundary handle: it can only create daemon invocations.
pub struct Server {
    binding: ScopeBinding,
    binding_digest: [u8; 32],
    tools: Vec<ToolDefinition>,
    audit: Log,
    next_invocation_id: u64,
}

impl Server {
    /// Binds from the active session registry and its persisted capability.
    ///
    /// # Errors
    ///
    /// Refuses missing, closed, expired, cross-tenant, unlinked, or scope-empty authority.
    pub fn bind(
        store: &Store,
        sessions: &SessionRegistry,
        session_id: SessionId,
        capability_id: CapabilityId,
        core_sequence: u64,
        audit_root: impl AsRef<Path>,
    ) -> Result<Self, ServerError> {
        let session = sessions
            .get(session_id)
            .ok_or(ServerError::MissingSession)?;
        let capability = Capability::restore(store, session.request.tenant.clone(), capability_id)
            .map_err(ServerError::Capability)?
            .ok_or(ServerError::MissingCapability)?;
        Self::bind_records(session, &capability, core_sequence, audit_root)
    }

    /// Binds from already-resolved daemon records without accepting ambient scopes.
    ///
    /// # Errors
    ///
    /// Applies the same linkage, tenant, expiry, and scope validation as [`Self::bind`].
    pub fn bind_records(
        session: &SessionRecord,
        capability: &Capability,
        core_sequence: u64,
        audit_root: impl AsRef<Path>,
    ) -> Result<Self, ServerError> {
        let binding = ScopeBinding::derive(session, capability, core_sequence)?;
        let tools = TOOL_CATALOGUE
            .iter()
            .filter(|tool| binding.scopes.contains(tool.required_scope))
            .copied()
            .collect::<Vec<_>>();
        if tools.is_empty() {
            return Err(ServerError::NoScope);
        }
        let binding_digest = binding_digest(&binding);
        let audit = Log::open(audit_root, binding.tenant()).map_err(ServerError::Audit)?;
        Ok(Self {
            binding,
            binding_digest,
            tools,
            audit,
            next_invocation_id: 0,
        })
    }

    #[must_use]
    pub const fn binding(&self) -> &ScopeBinding {
        &self.binding
    }

    #[must_use]
    pub fn tools(&self) -> &[ToolDefinition] {
        &self.tools
    }

    #[must_use]
    pub fn tool(&self, name: &str) -> Option<ToolDefinition> {
        self.tools.iter().find(|tool| tool.name == name).copied()
    }

    /// Audits a scoped invocation and emits a request that declares every daemon gate.
    ///
    /// # Errors
    ///
    /// Refuses invalid arguments, overflow, and any tool absent from this binding.
    pub fn route(
        &mut self,
        name: &str,
        arguments: Vec<u8>,
    ) -> Result<DaemonInvocation, ServerError> {
        if name.is_empty()
            || name.len() > MAX_TOOL_NAME_BYTES
            || name.as_bytes().contains(&0)
            || arguments.len() > MAX_ARGUMENT_BYTES
        {
            return Err(ServerError::InvalidInvocation);
        }
        let arguments_digest: [u8; 32] = Sha256::digest(&arguments).into();
        let tool = self.tool(name);
        let decision = if tool.is_some() { 1 } else { 2 };
        let attempt = invocation_audit(
            self.binding_digest,
            self.next_invocation_id,
            name,
            arguments_digest,
            decision,
            0,
        )?;
        self.audit
            .before_operation(&attempt, || ())
            .map_err(ServerError::Audit)?;
        let tool = tool.ok_or(ServerError::ToolAbsent)?;
        let invocation_id = self.next_invocation_id;
        self.next_invocation_id = self
            .next_invocation_id
            .checked_add(1)
            .ok_or(ServerError::Arithmetic)?;
        Ok(DaemonInvocation {
            server_binding: self.binding_digest,
            invocation_id,
            tool,
            arguments,
            arguments_digest,
            gates: REQUIRED_DAEMON_GATES,
        })
    }

    /// Records the daemon's typed result and consumes the pending invocation.
    ///
    /// # Errors
    ///
    /// Refuses an invocation from a different server and fails on audit persistence errors.
    pub fn complete(
        &mut self,
        invocation: DaemonInvocation,
        outcome: InvocationOutcome,
    ) -> Result<InvocationRecord, ServerError> {
        if invocation.server_binding != self.binding_digest {
            return Err(ServerError::WrongServer);
        }
        let completion = invocation_audit(
            self.binding_digest,
            invocation.invocation_id,
            invocation.tool.name,
            invocation.arguments_digest,
            1,
            outcome_code(outcome),
        )?;
        self.audit
            .before_operation(&completion, || ())
            .map_err(ServerError::Audit)?;
        Ok(InvocationRecord {
            invocation_id: invocation.invocation_id,
            arguments_digest: invocation.arguments_digest,
            outcome,
        })
    }

    #[must_use]
    pub const fn audit_entries(&self) -> u64 {
        self.audit.entries()
    }
}

#[derive(Debug)]
pub enum ServerError {
    MissingSession,
    MissingCapability,
    ClosedSession,
    TenantMismatch,
    CapabilityMismatch,
    ExpiredAuthority,
    NoScope,
    InvalidInvocation,
    ToolAbsent,
    WrongServer,
    Arithmetic,
    Capability(CapabilityError),
    Audit(AuditError),
}

fn binding_digest(binding: &ScopeBinding) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(binding.tenant.as_str().as_bytes());
    hasher.update(binding.session_id.0);
    hasher.update(binding.capability_id.0);
    for scope in &binding.scopes {
        hasher.update((scope.len() as u64).to_be_bytes());
        hasher.update(scope.as_bytes());
    }
    hasher.finalize().into()
}

fn invocation_audit(
    binding: [u8; 32],
    invocation_id: u64,
    name: &str,
    arguments_digest: [u8; 32],
    decision: u8,
    outcome: u8,
) -> Result<Vec<u8>, ServerError> {
    let name_length = u16::try_from(name.len()).map_err(|_| ServerError::InvalidInvocation)?;
    let mut payload = Vec::with_capacity(82 + name.len());
    payload.extend_from_slice(b"LXMI");
    payload.push(1);
    payload.extend_from_slice(&binding);
    payload.extend_from_slice(&invocation_id.to_be_bytes());
    payload.extend_from_slice(&name_length.to_be_bytes());
    payload.extend_from_slice(name.as_bytes());
    payload.extend_from_slice(&arguments_digest);
    payload.push(decision);
    payload.push(outcome);
    Ok(payload)
}

const fn outcome_code(outcome: InvocationOutcome) -> u8 {
    match outcome {
        InvocationOutcome::Completed => 1,
        InvocationOutcome::Refused => 2,
        InvocationOutcome::Unknown => 3,
        InvocationOutcome::Failed => 4,
    }
}
