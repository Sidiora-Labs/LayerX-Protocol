use std::collections::BTreeMap;

use layerx_types::ids::Did;

use crate::session::{SessionError, SessionId, Token};
use crate::store::TenantId;

#[path = "tenant/errors.rs"]
mod errors;
#[path = "tenant/isolation.rs"]
mod isolation;

pub use errors::{
    BoundedMetricKey, BoundedMetrics, ErrorClass, InternalError, MetricKind, MetricLabel,
    NormalizedError, SanitizedTrace, TIMING_MITIGATION,
};
pub use isolation::{
    ChannelBinding, ChannelKind, Config, IsolationError, RedactionPolicy, Retention, SignerBinding,
    SignerMaterial, TenantIsolation,
};

/// Produces the only public error, trace, and metric representation for an internal failure.
pub fn normalize_error(
    error: &InternalError,
    tenant: &TenantId,
    surface: Surface,
    metrics: &mut BoundedMetrics,
) -> NormalizedError {
    errors::normalize(error, tenant, surface, metrics)
}

/// Every public surface that must use the same authenticated tenant resolution.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Surface {
    Contract,
    RustSdk,
    TypeScriptSdk,
    PythonSdk,
    Mcp,
    Subscription,
    Export,
}

/// Untrusted request metadata and the trusted owner loaded for its target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestContext {
    pub surface: Surface,
    pub operation: String,
    pub core_sequence: u64,
    pub supplied_header_tenant: Option<TenantId>,
    pub supplied_body_tenant: Option<TenantId>,
    pub target_owner: Option<ObjectOwner>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectOwner {
    pub tenant: TenantId,
    pub agent: Option<Did>,
}

/// Principal claims copied only from an authenticated daemon token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPrincipal {
    pub tenant: TenantId,
    pub agent: Did,
    pub session_id: SessionId,
    pub surface: Surface,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AuthorizationOutcome {
    Allowed,
    NotAuthorized,
    ScopeDenied,
    Expired,
    InvalidRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorizationError {
    NotAuthorized,
    ScopeDenied,
    Expired,
    InvalidRequest,
}

impl AuthorizationError {
    const fn outcome(&self) -> AuthorizationOutcome {
        match self {
            Self::NotAuthorized => AuthorizationOutcome::NotAuthorized,
            Self::ScopeDenied => AuthorizationOutcome::ScopeDenied,
            Self::Expired => AuthorizationOutcome::Expired,
            Self::InvalidRequest => AuthorizationOutcome::InvalidRequest,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantAuditEntry {
    pub tenant: TenantId,
    pub token_id: [u8; 32],
    pub surface: Surface,
    pub operation: String,
    pub outcome: AuthorizationOutcome,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MetricKey {
    pub tenant: TenantId,
    pub surface: Surface,
    pub outcome: AuthorizationOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TenantTrace {
    pub tenant: TenantId,
    pub surface: Surface,
    pub operation: String,
    pub outcome: AuthorizationOutcome,
}

/// Observable evidence emitted by the common tenant gate.
#[derive(Debug, Default)]
pub struct TenantObservability {
    audit: Vec<TenantAuditEntry>,
    metrics: BTreeMap<MetricKey, u64>,
    traces: Vec<TenantTrace>,
}

impl TenantObservability {
    #[must_use]
    pub fn audit(&self) -> &[TenantAuditEntry] {
        &self.audit
    }

    #[must_use]
    pub fn metrics(&self) -> &BTreeMap<MetricKey, u64> {
        &self.metrics
    }

    #[must_use]
    pub fn traces(&self) -> &[TenantTrace] {
        &self.traces
    }

    fn record(
        &mut self,
        token: &Token,
        surface: Surface,
        operation: &str,
        outcome: AuthorizationOutcome,
    ) {
        let tenant = token.tenant().clone();
        self.audit.push(TenantAuditEntry {
            tenant: tenant.clone(),
            token_id: token.token_id(),
            surface,
            operation: operation.to_owned(),
            outcome,
        });
        let counter = self
            .metrics
            .entry(MetricKey {
                tenant: tenant.clone(),
                surface,
                outcome,
            })
            .or_default();
        *counter = counter.saturating_add(1);
        self.traces.push(TenantTrace {
            tenant,
            surface,
            operation: operation.to_owned(),
            outcome,
        });
    }
}

/// Resolves the tenant and agent exclusively from the authenticated token.
pub fn resolve(
    token: &Token,
    request: &RequestContext,
    observability: &mut TenantObservability,
) -> Result<ResolvedPrincipal, AuthorizationError> {
    if request.operation.is_empty()
        || request.operation.len() > 255
        || request.operation.as_bytes().contains(&0)
    {
        let error = AuthorizationError::InvalidRequest;
        observability.record(token, request.surface, &request.operation, error.outcome());
        return Err(error);
    }
    let authorization = token.authorize(
        token.tenant(),
        token.agent(),
        &request.operation,
        request.core_sequence,
    );
    let session_id = match authorization {
        Ok(session_id) => session_id,
        Err(SessionError::ScopeDenied) => {
            let error = AuthorizationError::ScopeDenied;
            observability.record(token, request.surface, &request.operation, error.outcome());
            return Err(error);
        }
        Err(SessionError::Expired) => {
            let error = AuthorizationError::Expired;
            observability.record(token, request.surface, &request.operation, error.outcome());
            return Err(error);
        }
        Err(_) => {
            let error = AuthorizationError::NotAuthorized;
            observability.record(token, request.surface, &request.operation, error.outcome());
            return Err(error);
        }
    };
    if let Some(owner) = &request.target_owner {
        if let Err(error) = require_owner(token.tenant(), token.agent(), owner) {
            observability.record(token, request.surface, &request.operation, error.outcome());
            return Err(error);
        }
    }
    observability.record(
        token,
        request.surface,
        &request.operation,
        AuthorizationOutcome::Allowed,
    );
    Ok(ResolvedPrincipal {
        tenant: token.tenant().clone(),
        agent: token.agent().clone(),
        session_id,
        surface: request.surface,
    })
}

/// Applies the same non-enumerating owner check to every target surface.
pub fn require_owner(
    tenant: &TenantId,
    agent: &Did,
    owner: &ObjectOwner,
) -> Result<(), AuthorizationError> {
    if &owner.tenant != tenant || owner.agent.as_ref().is_some_and(|value| value != agent) {
        Err(AuthorizationError::NotAuthorized)
    } else {
        Ok(())
    }
}
