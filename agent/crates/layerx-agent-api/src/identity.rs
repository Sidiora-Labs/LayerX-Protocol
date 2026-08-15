//! Agent identity, session, capability, and budget contract types.

use crate::{Amount, BudgetLimit, TimestampSeconds};

/// Contract construction failure before a request can cross the daemon boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    Empty(&'static str),
    Zero(&'static str),
    DaemonLimitFunding,
}

macro_rules! required_text {
    ($name:ident, $field:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Constructs a non-empty contract identifier.
            ///
            /// # Errors
            /// Returns [`ContractError::Empty`] when the required value is empty.
            pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(ContractError::Empty($field));
                }
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

required_text!(TenantId, "tenant");
required_text!(AgentDid, "agent_did");
required_text!(AuthorityRef, "authority_ref");
required_text!(ClientId, "client");
required_text!(PolicyVersion, "policy_version");
required_text!(SessionId, "session_id");
required_text!(CapabilityId, "capability_id");
required_text!(BudgetId, "budget_id");
required_text!(Counterparty, "counterparty");
required_text!(Asset, "asset");
required_text!(Purpose, "purpose");

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActivityType(pub u16);

/// A restriction dimension that a caller must supply explicitly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitSet<T>(Vec<T>);

impl<T> ExplicitSet<T> {
    #[must_use]
    pub const fn deny_all() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub const fn allow(values: Vec<T>) -> Self {
        Self(values)
    }

    #[must_use]
    pub fn values(&self) -> &[T] {
        &self.0
    }
}

/// Complete session authority context; no operation may synthesize defaults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionContext {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
    pub authority_ref: AuthorityRef,
    pub permitted_activity_types: ExplicitSet<ActivityType>,
    pub expiry: TimestampSeconds,
    pub client: ClientId,
    pub policy_version: PolicyVersion,
}

impl SessionContext {
    /// Creates a context only when expiry is explicitly nonzero.
    ///
    /// # Errors
    /// Returns [`ContractError::Zero`] when expiry is zero.
    pub fn new(
        tenant: TenantId,
        agent_did: AgentDid,
        authority_ref: AuthorityRef,
        permitted_activity_types: ExplicitSet<ActivityType>,
        expiry: TimestampSeconds,
        client: ClientId,
        policy_version: PolicyVersion,
    ) -> Result<Self, ContractError> {
        if expiry.0 == 0 {
            return Err(ContractError::Zero("expiry"));
        }
        Ok(Self {
            tenant,
            agent_did,
            authority_ref,
            permitted_activity_types,
            expiry,
            client,
            policy_version,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRegistration {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
    pub authority_ref: AuthorityRef,
    pub client: ClientId,
    pub policy_version: PolicyVersion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionOpen(pub SessionContext);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRefresh {
    pub session_id: SessionId,
    pub context: SessionContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionClose {
    pub session_id: SessionId,
    pub context: SessionContext,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionList(pub SessionContext);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmountCeiling {
    pub asset: Asset,
    pub amount: Amount,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RateCeiling {
    pub window_seconds: TimestampSeconds,
    pub maximum_actions: u64,
}

/// Every capability dimension is present, including explicitly empty deny sets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityDimensions {
    pub activity_types: ExplicitSet<ActivityType>,
    pub counterparties: ExplicitSet<Counterparty>,
    pub assets: ExplicitSet<Asset>,
    pub amount_ceilings: ExplicitSet<AmountCeiling>,
    pub rate_ceilings: ExplicitSet<RateCeiling>,
    pub purpose_constraints: ExplicitSet<Purpose>,
    pub expiry: TimestampSeconds,
}

impl CapabilityDimensions {
    /// Validates the only scalar dimension that can be malformed.
    ///
    /// # Errors
    /// Returns [`ContractError::Zero`] when expiry is zero.
    pub fn validate(self) -> Result<Self, ContractError> {
        if self.expiry.0 == 0 {
            return Err(ContractError::Zero("expiry"));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityCreate {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
    pub dimensions: CapabilityDimensions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityAttenuate {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
    pub parent_id: CapabilityId,
    pub dimensions: CapabilityDimensions,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityList {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityRevoke {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
    pub capability_id: CapabilityId,
}

/// States who actually enforces a budget-like restriction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetEnforcement {
    ProtocolBudget,
    DaemonLimit,
}

impl BudgetEnforcement {
    pub const DAEMON_LIMIT_NOTICE: &'static str =
        "Bypassing the daemon bypasses this limit. It is not equivalent to a protocol budget.";

    #[must_use]
    pub const fn guarantee(self) -> &'static str {
        match self {
            Self::ProtocolBudget => "protocol_enforced",
            Self::DaemonLimit => "daemon_enforced",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetCreate {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
    pub asset: Asset,
    pub limit: BudgetLimit,
    pub enforcement: BudgetEnforcement,
    pub expiry: TimestampSeconds,
}

impl BudgetCreate {
    /// Validates finite limit and expiry.
    ///
    /// # Errors
    /// Returns [`ContractError::Zero`] for a zero limit or expiry.
    pub fn validate(self) -> Result<Self, ContractError> {
        if self.limit.0 == 0 {
            return Err(ContractError::Zero("limit"));
        }
        if self.expiry.0 == 0 {
            return Err(ContractError::Zero("expiry"));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetFund {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
    pub budget_id: BudgetId,
    pub amount: Amount,
    pub enforcement: BudgetEnforcement,
}

impl BudgetFund {
    /// Protocol funding cannot be represented for a daemon-only limit.
    ///
    /// # Errors
    /// Returns [`ContractError::DaemonLimitFunding`] for daemon-only limits.
    pub fn validate(self) -> Result<Self, ContractError> {
        if matches!(self.enforcement, BudgetEnforcement::DaemonLimit) {
            return Err(ContractError::DaemonLimitFunding);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetList {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetTarget {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
    pub budget_id: BudgetId,
}

/// Exact authority attached to every successful or refused response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityDescription {
    pub tenant: TenantId,
    pub agent_did: AgentDid,
    pub authority_ref: AuthorityRef,
    pub protocol_authority: Vec<u8>,
}

impl AuthorityDescription {
    /// Creates an authority description with non-empty canonical protocol bytes.
    ///
    /// # Errors
    /// Returns [`ContractError::Empty`] when protocol authority bytes are absent.
    pub fn new(
        tenant: TenantId,
        agent_did: AgentDid,
        authority_ref: AuthorityRef,
        protocol_authority: Vec<u8>,
    ) -> Result<Self, ContractError> {
        if protocol_authority.is_empty() {
            return Err(ContractError::Empty("protocol_authority"));
        }
        Ok(Self {
            tenant,
            agent_did,
            authority_ref,
            protocol_authority,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityResponse<T> {
    pub authority: AuthorityDescription,
    pub value: T,
}
