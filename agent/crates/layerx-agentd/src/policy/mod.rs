//! Deterministic local policy restrictions for daemon write requests.

mod eval;
#[path = "version.rs"]
mod versioning;

pub use eval::{
    EvaluationFailure, EvaluationInput, PolicyRequest, PolicySet, Rule, RuleConstraints,
    RuleEffect, RuleMatcher, SequenceWindow,
};
pub use versioning::{
    load_policy_source, Activation, PolicyAuditEntry, PolicyRegistry, PolicySnapshot,
    PolicySourceError, PolicyValidationError, MAX_POLICY_SOURCE_BYTES,
};

/// The local policy outcome. An allow is never protocol authorisation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Allow,
    Deny,
}

/// Stable reason for the final policy result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionReason {
    PermittedByRule,
    ExplicitDeny,
    ApprovalRequired,
    NoPermittingRule,
    InvalidContext,
    EvaluationFailure,
}

/// Complete deterministic policy decision, including the loaded version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decision {
    pub outcome: Outcome,
    pub policy_version: String,
    pub matched_rules: Vec<String>,
    pub deciding_rule: Option<String>,
    pub reason: DecisionReason,
}

impl Decision {
    fn deny(policy_version: &str, reason: DecisionReason) -> Self {
        Self {
            outcome: Outcome::Deny,
            policy_version: policy_version.to_owned(),
            matched_rules: Vec::new(),
            deciding_rule: None,
            reason,
        }
    }
}

/// Evaluates a request with no ambient inputs and denies on every failure.
#[must_use]
pub fn evaluate(policy: &PolicySet, input: &EvaluationInput<'_>) -> Decision {
    eval::evaluate_policy(policy, input)
}

/// Evaluates through an alternate matcher while retaining fail-closed containment.
#[must_use]
pub fn evaluate_with_matcher(
    policy: &PolicySet,
    input: &EvaluationInput<'_>,
    matcher: &dyn RuleMatcher,
) -> Decision {
    eval::evaluate_with(policy, input, matcher)
}

/// Validates a policy set before it can become active.
pub fn validate(policy: &PolicySet) -> Result<(), PolicyValidationError> {
    versioning::validate_policy(policy)
}

/// Atomically activates a validated policy version for future requests.
pub fn activate(
    registry: &mut PolicyRegistry,
    policy: PolicySet,
) -> Result<Activation, PolicyValidationError> {
    versioning::activate_policy(registry, policy)
}
