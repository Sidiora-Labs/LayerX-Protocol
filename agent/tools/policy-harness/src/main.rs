use std::collections::BTreeSet;

use layerx_agentd::policy::{evaluate, DecisionReason, EvaluationInput, Outcome, PolicySet};

#[path = "../../../tests/policy/adversarial.rs"]
mod adversarial;

use adversarial::{
    agent_policy_adversarial_corpus, required_constraint_coverage, ConstraintDimension,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct HarnessDecision {
    case: &'static str,
    outcome: Outcome,
    reason: DecisionReason,
    policy_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HarnessReport {
    decisions: Vec<HarnessDecision>,
    coverage: BTreeSet<ConstraintDimension>,
}

fn agent_policy_harness() -> Result<HarnessReport, String> {
    let corpus = agent_policy_adversarial_corpus();
    let mut decisions = Vec::new();
    let mut coverage = BTreeSet::new();
    for case in &corpus {
        coverage.extend(case.coverage.iter().copied());
        let input = EvaluationInput {
            request: &case.request,
            session: &case.session,
            capability: &case.capability,
            budget: &case.budget,
        };
        let decision = evaluate(&case.policy, &input);
        if decision.outcome != case.expected {
            return Err(format!(
                "case {} produced {:?}, expected {:?}",
                case.name, decision.outcome, case.expected
            ));
        }
        let deny_by_default = evaluate(
            &PolicySet {
                version: case.policy.version.clone(),
                rules: Vec::new(),
                evaluation_step_limit: 1,
            },
            &input,
        );
        if deny_by_default.outcome != Outcome::Deny {
            return Err(format!("deny-by-default escaped for {}", case.name));
        }
        decisions.push(HarnessDecision {
            case: case.name,
            outcome: decision.outcome,
            reason: decision.reason,
            policy_version: decision.policy_version,
        });
    }
    let required = required_constraint_coverage();
    if coverage != required {
        let missing: Vec<_> = required.difference(&coverage).copied().collect();
        return Err(format!("constraint coverage missing: {missing:?}"));
    }
    Ok(HarnessReport {
        decisions,
        coverage,
    })
}

fn main() {
    let report = agent_policy_harness().unwrap_or_else(|error| panic!("policy harness: {error}"));
    for decision in &report.decisions {
        println!(
            "case={} outcome={:?} reason={:?} policy_version={}",
            decision.case, decision.outcome, decision.reason, decision.policy_version
        );
    }
    println!(
        "cases={} constraints_covered={}",
        report.decisions.len(),
        report.coverage.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adversarial_corpus_has_no_unintended_allow_and_complete_coverage() {
        let report = agent_policy_harness().unwrap_or_else(|error| panic!("harness: {error}"));
        assert_eq!(report.coverage, required_constraint_coverage());
        assert_eq!(
            report
                .decisions
                .iter()
                .filter(|decision| decision.outcome == Outcome::Allow)
                .map(|decision| decision.case)
                .collect::<Vec<_>>(),
            vec!["intended-control"]
        );
    }
}
