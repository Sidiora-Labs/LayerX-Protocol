//! Construction boundary for MCP deployments that cannot emit write invocations.

use std::path::Path;

use layerx_agentd::capability::{Capability, CapabilityId};
use layerx_agentd::session::{SessionId, SessionRecord, SessionRegistry};
use layerx_agentd::store::Store;

use crate::server::{
    CapabilityDeclaration, DaemonInvocation, DeploymentMode, InvocationOutcome, InvocationRecord,
    ScopeBinding, Server, ServerError, ToolDefinition, ToolKind,
};

/// Opaque read-only MCP server. Its inner full server is never exposed.
pub struct ReadOnly {
    server: Server,
}

impl ReadOnly {
    /// Binds a read-only server from daemon-owned records.
    ///
    /// # Errors
    ///
    /// Refuses invalid authority or a scope set containing no reachable read tool.
    pub fn bind(
        store: &Store,
        sessions: &SessionRegistry,
        session_id: SessionId,
        capability_id: CapabilityId,
        core_sequence: u64,
        audit_root: impl AsRef<Path>,
    ) -> Result<Self, ServerError> {
        Server::bind_for_mode(
            store,
            sessions,
            session_id,
            capability_id,
            core_sequence,
            audit_root,
            DeploymentMode::ReadOnly,
        )
        .map(|server| Self { server })
    }

    /// Binds a read-only server from already resolved daemon records.
    ///
    /// # Errors
    ///
    /// Applies normal authority checks and refuses a scope set with no read tool.
    pub fn bind_records(
        session: &SessionRecord,
        capability: &Capability,
        core_sequence: u64,
        audit_root: impl AsRef<Path>,
    ) -> Result<Self, ServerError> {
        Server::bind_records_for_mode(
            session,
            capability,
            core_sequence,
            audit_root,
            DeploymentMode::ReadOnly,
        )
        .map(|server| Self { server })
    }

    #[must_use]
    pub const fn binding(&self) -> &ScopeBinding {
        self.server.binding()
    }

    #[must_use]
    pub fn tools(&self) -> &[ToolDefinition] {
        self.server.tools()
    }

    #[must_use]
    pub fn capability_declaration(&self) -> CapabilityDeclaration {
        self.server.capability_declaration()
    }

    /// Routes a read invocation through the ordinary daemon choke point.
    ///
    /// # Errors
    ///
    /// Refuses every absent or non-read definition before an invocation can be emitted.
    pub fn route(
        &mut self,
        name: &str,
        arguments: Vec<u8>,
    ) -> Result<DaemonInvocation, ServerError> {
        if self
            .server
            .tool(name)
            .is_none_or(|tool| tool.kind != ToolKind::Read || tool.mutation != "none")
        {
            return Err(ServerError::ToolAbsent);
        }
        self.server.route(name, arguments)
    }

    /// Completes an invocation emitted by this exact read-only server binding.
    ///
    /// # Errors
    ///
    /// Refuses invocations from any other binding or deployment mode.
    pub fn complete(
        &mut self,
        invocation: DaemonInvocation,
        outcome: InvocationOutcome,
    ) -> Result<InvocationRecord, ServerError> {
        if invocation.tool().kind != ToolKind::Read || invocation.tool().mutation != "none" {
            return Err(ServerError::ToolAbsent);
        }
        self.server.complete(invocation, outcome)
    }

    #[must_use]
    pub const fn audit_entries(&self) -> u64 {
        self.server.audit_entries()
    }
}
