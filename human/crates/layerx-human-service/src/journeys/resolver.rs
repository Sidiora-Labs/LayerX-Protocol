//! Total, deterministic routing from a human movement request to typed intents.

use std::fmt::{Display, Formatter};

use layerx_intents::{
    BridgeDepositCredit, BridgeWithdrawRequest, BudgetCreate, BudgetDefund, BudgetFund, Intent,
    IntentError, IntentKind, LxpReceive, LxpSend,
};
use layerx_types::account::{AccountId, AccountNamespace};
use layerx_types::amount::Amount;
use layerx_types::ids::{AssetId, CheckpointId, IdempotencyKey};
use layerx_types::intent::{
    BudgetId, ContextHash, DepositProofId, EvmAddress, NetworkId, PayerGrantId, PeriodLength,
    ProtocolVersion, PurposeHash, RolloverPolicy, SendAuthorization, Sequence, TimestampSeconds,
    WithdrawalId,
};

/// The only source and destination kinds accepted by the movement API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EndpointKind {
    PaxeerWallet,
    Human,
    Agent,
    AgentBudget,
}

/// One declared movement endpoint. Protocol account identities stay typed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Endpoint {
    PaxeerWallet,
    Human(AccountId),
    Agent(AccountId),
    AgentBudget(AccountId),
}

impl Endpoint {
    /// Returns the closed endpoint discriminator used in typed refusals.
    #[must_use]
    pub const fn kind(&self) -> EndpointKind {
        match self {
            Self::PaxeerWallet => EndpointKind::PaxeerWallet,
            Self::Human(_) => EndpointKind::Human,
            Self::Agent(_) => EndpointKind::Agent,
            Self::AgentBudget(_) => EndpointKind::AgentBudget,
        }
    }

    fn account(&self) -> Option<&AccountId> {
        match self {
            Self::PaxeerWallet => None,
            Self::Human(account) | Self::Agent(account) | Self::AgentBudget(account) => {
                Some(account)
            }
        }
    }

    fn has_declared_namespace(&self) -> bool {
        match self {
            Self::PaxeerWallet => true,
            Self::Human(account) | Self::Agent(account) => {
                account.namespace() == AccountNamespace::AgentMain
            }
            Self::AgentBudget(account) => account.namespace() == AccountNamespace::AgentBudget,
        }
    }
}

/// Exact material needed for an authenticated 402LXP send.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SendRoute {
    pub account_sequence: Sequence,
    pub idempotency_key: IdempotencyKey,
    pub expires_at: TimestampSeconds,
    pub context_hash: ContextHash,
    pub authorization: SendAuthorization,
    pub network_id: NetworkId,
    pub protocol_version: ProtocolVersion,
}

/// Optional first leg when a managed-agent budget does not yet exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetCreation {
    pub per_period_limit: Amount,
    pub period_length: PeriodLength,
    pub rollover: RolloverPolicy,
    pub carry_cap: Amount,
    pub purpose: PurposeHash,
    pub expiry: TimestampSeconds,
}

/// Exact relationship material for funding or returning a managed budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetRoute {
    pub budget_id: BudgetId,
    pub idempotency_key: IdempotencyKey,
    pub revocation_sequence: Sequence,
    pub create: Option<BudgetCreation>,
}

/// Exact material for an agent-to-human payer-grant draw.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayerGrantRoute {
    pub payer_grant: PayerGrantId,
    pub receiver_sequence: Sequence,
    pub idempotency_key: IdempotencyKey,
    pub context_hash: ContextHash,
}

/// Custody-boundary evidence. These are the only variants allowed to use the
/// words deposit and withdrawal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CustodyRoute {
    Deposit {
        deposit_proof: DepositProofId,
        checkpoint: CheckpointId,
        reserve: AccountId,
        idempotency_key: IdempotencyKey,
    },
    Withdrawal {
        withdrawal_id: WithdrawalId,
        withdrawals_account: AccountId,
        payout_address: EvmAddress,
        idempotency_key: IdempotencyKey,
    },
}

/// Declared authority relationship. The resolver consults no ambient state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Relationship {
    Direct(SendRoute),
    ManagedBudget(BudgetRoute),
    AgentAuthorized(SendRoute),
    PayerGrant(PayerGrantRoute),
    Custody(CustodyRoute),
}

/// Complete deterministic input to route resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteRequest {
    pub source: Endpoint,
    pub destination: Endpoint,
    pub relationship: Relationship,
    pub asset: AssetId,
    pub amount: Amount,
}

/// Vocabulary permitted in APIs, logs and user-facing copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MovementTerm {
    Deposit,
    Withdrawal,
    Fund,
    Allocate,
    Return,
    Transfer,
}

impl MovementTerm {
    /// Returns the exact lowercase contract term for APIs and logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deposit => "deposit",
            Self::Withdrawal => "withdrawal",
            Self::Fund => "fund",
            Self::Allocate => "allocate",
            Self::Return => "return",
            Self::Transfer => "transfer",
        }
    }

    /// Returns the single user action covering every movement route.
    #[must_use]
    pub const fn user_action() -> &'static str {
        "Move money"
    }

    /// Whether this term is reserved to a custody crossing.
    #[must_use]
    pub const fn is_custody_boundary(self) -> bool {
        matches!(self, Self::Deposit | Self::Withdrawal)
    }
}

impl Display for MovementTerm {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Exact protocol mechanism selected for one leg.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mechanism {
    BudgetCreate,
    BudgetFund,
    Send,
    BudgetDefund,
    ReceiveUnderPayerGrant,
    BridgeDepositCredit,
    BridgeWithdrawRequest,
}

/// One automatically derived, executable journey leg.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteLeg {
    term: MovementTerm,
    mechanism: Mechanism,
    intent: Intent,
}

impl RouteLeg {
    /// Returns the movement vocabulary term for APIs, logs and copy.
    #[must_use]
    pub const fn term(&self) -> MovementTerm {
        self.term
    }

    /// Returns the selected protocol mechanism.
    #[must_use]
    pub const fn mechanism(&self) -> Mechanism {
        self.mechanism
    }

    /// Returns the typed intent derived for this leg.
    #[must_use]
    pub const fn intent(&self) -> &Intent {
        &self.intent
    }
}

/// A complete route. Construction is atomic: no route exists on refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Route {
    legs: Vec<RouteLeg>,
}

impl Route {
    /// Returns the non-empty ordered leg list.
    #[must_use]
    pub fn legs(&self) -> &[RouteLeg] {
        &self.legs
    }

    /// Returns the product-level action name shared by every route.
    #[must_use]
    pub const fn user_action() -> &'static str {
        MovementTerm::user_action()
    }
}

/// Total resolver refusal. No variant contains or exposes a partial route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteError {
    Unavailable {
        source: EndpointKind,
        destination: EndpointKind,
        relationship: &'static str,
    },
    InvalidIntent(IntentError),
}

impl Display for RouteError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable {
                source,
                destination,
                relationship,
            } => write!(
                formatter,
                "movement route unavailable: {source:?} to {destination:?} via {relationship}"
            ),
            Self::InvalidIntent(error) => write!(formatter, "movement intent refused: {error:?}"),
        }
    }
}

impl std::error::Error for RouteError {}

impl From<IntentError> for RouteError {
    fn from(value: IntentError) -> Self {
        Self::InvalidIntent(value)
    }
}

/// Human-changeable settings surfaces named by typed movement refusals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeSurface {
    Policy,
    Budget,
    Capability,
}

impl ChangeSurface {
    /// Returns the stable UI link for the setting the human can change.
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Policy => "/settings/policies",
            Self::Budget => "/agents/budgets",
            Self::Capability => "/agents/capabilities",
        }
    }
}

/// Authority that refused a movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitSource {
    Policy,
    Budget,
    Capability,
    Protocol,
}

/// Plain-language refusal with the exact limit and an actionable link only
/// where the limit belongs to the human.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LimitRefusal {
    source: LimitSource,
    limit: String,
    change_surface: Option<ChangeSurface>,
}

impl LimitRefusal {
    /// Constructs a typed refusal, rejecting empty limit names and mismatched
    /// change links.
    ///
    /// # Errors
    ///
    /// Refuses an empty limit or a change surface that does not own the limit.
    pub fn new(
        source: LimitSource,
        limit: impl Into<String>,
        change_surface: Option<ChangeSurface>,
    ) -> Result<Self, LimitRefusalError> {
        let limit = limit.into();
        if limit.trim().is_empty() {
            return Err(LimitRefusalError::EmptyLimit);
        }
        let link_matches = matches!(
            (source, change_surface),
            (LimitSource::Policy, Some(ChangeSurface::Policy))
                | (LimitSource::Budget, Some(ChangeSurface::Budget))
                | (LimitSource::Capability, Some(ChangeSurface::Capability))
                | (LimitSource::Protocol, None)
        );
        if !link_matches {
            return Err(LimitRefusalError::MismatchedChangeSurface);
        }
        Ok(Self {
            source,
            limit,
            change_surface,
        })
    }

    /// Returns the refusing authority.
    #[must_use]
    pub const fn source(&self) -> LimitSource {
        self.source
    }

    /// Returns the exact limit name supplied by the refusing boundary.
    #[must_use]
    pub fn limit(&self) -> &str {
        &self.limit
    }

    /// Returns the settings link only when the human owns the limit.
    #[must_use]
    pub fn change_path(&self) -> Option<&'static str> {
        self.change_surface.map(ChangeSurface::path)
    }

    /// Renders the refusal without protocol result-code leakage.
    #[must_use]
    pub fn plain_language(&self) -> String {
        let owner = match self.source {
            LimitSource::Policy => "policy",
            LimitSource::Budget => "budget",
            LimitSource::Capability => "capability",
            LimitSource::Protocol => "protocol",
        };
        format!("The {owner} limit '{}' refused this movement.", self.limit)
    }
}

/// Invalid typed refusal construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitRefusalError {
    EmptyLimit,
    MismatchedChangeSurface,
}

impl Display for LimitRefusalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyLimit => formatter.write_str("movement refusal limit is empty"),
            Self::MismatchedChangeSurface => {
                formatter.write_str("movement refusal change surface does not own the limit")
            }
        }
    }
}

impl std::error::Error for LimitRefusalError {}

/// Stateless deterministic resolver.
#[derive(Clone, Copy, Debug, Default)]
pub struct RouteResolver;

impl RouteResolver {
    /// Resolves every declared input combination to a complete route or one
    /// typed unavailable refusal. No I/O, time, randomness or global state is
    /// consulted.
    ///
    /// # Errors
    ///
    /// Returns a typed unavailable route or the exact typed-intent refusal.
    #[allow(clippy::too_many_lines)]
    pub fn resolve(request: &RouteRequest) -> Result<Route, RouteError> {
        if !request.source.has_declared_namespace() || !request.destination.has_declared_namespace()
        {
            return Err(unavailable(request));
        }
        match (
            &request.source,
            &request.destination,
            request.relationship.clone(),
        ) {
            (
                Endpoint::PaxeerWallet,
                Endpoint::Human(recipient),
                Relationship::Custody(CustodyRoute::Deposit {
                    deposit_proof,
                    checkpoint,
                    reserve,
                    idempotency_key,
                }),
            ) if reserve.namespace() == AccountNamespace::SystemPaxeerReserve => Ok(one_leg(
                MovementTerm::Deposit,
                Mechanism::BridgeDepositCredit,
                BridgeDepositCredit::new(
                    deposit_proof,
                    checkpoint,
                    reserve,
                    recipient.clone(),
                    request.asset,
                    request.amount,
                    idempotency_key,
                )?,
                IntentKind::BridgeDepositCredit,
            )),
            (
                Endpoint::Human(owner),
                Endpoint::PaxeerWallet,
                Relationship::Custody(CustodyRoute::Withdrawal {
                    withdrawal_id,
                    withdrawals_account,
                    payout_address,
                    idempotency_key,
                }),
            ) if withdrawals_account.namespace() == AccountNamespace::SystemPaxeerWithdrawals => {
                Ok(one_leg(
                    MovementTerm::Withdrawal,
                    Mechanism::BridgeWithdrawRequest,
                    BridgeWithdrawRequest::new(
                        withdrawal_id,
                        owner.clone(),
                        withdrawals_account,
                        payout_address,
                        request.asset,
                        request.amount,
                        idempotency_key,
                    )?,
                    IntentKind::BridgeWithdrawRequest,
                ))
            }
            (
                Endpoint::Human(owner),
                Endpoint::AgentBudget(budget_account),
                Relationship::ManagedBudget(route),
            ) => budget_funding(request, owner, budget_account, route),
            (
                Endpoint::AgentBudget(budget_account),
                Endpoint::Human(owner),
                Relationship::ManagedBudget(route),
            ) => Ok(one_leg(
                MovementTerm::Return,
                Mechanism::BudgetDefund,
                BudgetDefund::new(
                    route.budget_id,
                    budget_account.clone(),
                    owner.clone(),
                    request.asset,
                    request.amount,
                    route.revocation_sequence,
                    route.idempotency_key,
                )?,
                IntentKind::BudgetDefund,
            )),
            (Endpoint::Agent(from), Endpoint::Human(to), Relationship::PayerGrant(route)) => {
                Ok(one_leg(
                    MovementTerm::Return,
                    Mechanism::ReceiveUnderPayerGrant,
                    LxpReceive::new(
                        from.clone(),
                        to.clone(),
                        request.asset,
                        request.amount,
                        route.payer_grant,
                        route.receiver_sequence,
                        route.idempotency_key,
                        route.context_hash,
                    )?,
                    IntentKind::LxpReceive,
                ))
            }
            (source, destination, Relationship::Direct(route))
                if direct_term(source.kind(), destination.kind()).is_some() =>
            {
                send(request, source, destination, route, false)
            }
            (source, destination, Relationship::AgentAuthorized(route))
                if agent_authorized_term(source.kind(), destination.kind()).is_some() =>
            {
                send(request, source, destination, route, true)
            }
            _ => Err(unavailable(request)),
        }
    }
}

fn budget_funding(
    request: &RouteRequest,
    owner: &AccountId,
    budget_account: &AccountId,
    route: BudgetRoute,
) -> Result<Route, RouteError> {
    let mut legs = Vec::with_capacity(if route.create.is_some() { 2 } else { 1 });
    if let Some(create) = route.create {
        let intent = BudgetCreate::new(
            route.budget_id,
            owner.clone(),
            budget_account.clone(),
            request.asset,
            create.per_period_limit,
            create.period_length,
            create.rollover,
            create.carry_cap,
            create.purpose,
            create.expiry,
        )?;
        legs.push(RouteLeg {
            term: MovementTerm::Fund,
            mechanism: Mechanism::BudgetCreate,
            intent: Intent::v1(IntentKind::BudgetCreate(intent)),
        });
    }
    let fund = BudgetFund::new(
        route.budget_id,
        owner.clone(),
        budget_account.clone(),
        request.asset,
        request.amount,
        route.idempotency_key,
    )?;
    legs.push(RouteLeg {
        term: MovementTerm::Fund,
        mechanism: Mechanism::BudgetFund,
        intent: Intent::v1(IntentKind::BudgetFund(fund)),
    });
    Ok(Route { legs })
}

fn send(
    request: &RouteRequest,
    source: &Endpoint,
    destination: &Endpoint,
    route: SendRoute,
    agent_authorized: bool,
) -> Result<Route, RouteError> {
    let from = source.account().ok_or_else(|| unavailable(request))?;
    let to = destination.account().ok_or_else(|| unavailable(request))?;
    let term = if agent_authorized {
        agent_authorized_term(source.kind(), destination.kind())
    } else {
        direct_term(source.kind(), destination.kind())
    }
    .ok_or_else(|| unavailable(request))?;
    Ok(one_leg(
        term,
        Mechanism::Send,
        LxpSend::new(
            from.clone(),
            to.clone(),
            request.asset,
            request.amount,
            route.account_sequence,
            route.idempotency_key,
            route.expires_at,
            route.context_hash,
            route.authorization,
            route.network_id,
            route.protocol_version,
        )?,
        IntentKind::LxpSend,
    ))
}

const fn direct_term(source: EndpointKind, destination: EndpointKind) -> Option<MovementTerm> {
    match (source, destination) {
        (EndpointKind::Human | EndpointKind::Agent, EndpointKind::Agent)
        | (EndpointKind::Human, EndpointKind::Human) => Some(MovementTerm::Transfer),
        (EndpointKind::AgentBudget, EndpointKind::Agent) => Some(MovementTerm::Allocate),
        _ => None,
    }
}

const fn agent_authorized_term(
    source: EndpointKind,
    destination: EndpointKind,
) -> Option<MovementTerm> {
    match (source, destination) {
        (EndpointKind::Agent, EndpointKind::Human) => Some(MovementTerm::Return),
        (EndpointKind::Agent, EndpointKind::Agent) => Some(MovementTerm::Transfer),
        (EndpointKind::AgentBudget, EndpointKind::Agent) => Some(MovementTerm::Allocate),
        _ => None,
    }
}

fn one_leg<T>(
    term: MovementTerm,
    mechanism: Mechanism,
    value: T,
    wrap: fn(T) -> IntentKind,
) -> Route {
    Route {
        legs: vec![RouteLeg {
            term,
            mechanism,
            intent: Intent::v1(wrap(value)),
        }],
    }
}

fn unavailable(request: &RouteRequest) -> RouteError {
    RouteError::Unavailable {
        source: request.source.kind(),
        destination: request.destination.kind(),
        relationship: match request.relationship {
            Relationship::Direct(_) => "direct",
            Relationship::ManagedBudget(_) => "managed-budget",
            Relationship::AgentAuthorized(_) => "agent-authorized",
            Relationship::PayerGrant(_) => "payer-grant",
            Relationship::Custody(CustodyRoute::Deposit { .. }) => "custody-deposit",
            Relationship::Custody(CustodyRoute::Withdrawal { .. }) => "custody-withdrawal",
        },
    }
}
