use std::collections::BTreeSet;

use layerx_types::error::{ErrorClass, LayerError};
use layerx_types::result::{KnownResult, ResultCode, ResultDomain, Retriability};

#[test]
fn every_layer_error_has_one_distinct_class() {
    let errors = [
        LayerError::TransportFailure { code: 1 },
        LayerError::Deadline,
        LayerError::ProtocolIncompatibility { local: 1, peer: 2 },
        LayerError::UnavailableCapability {
            capability: "proof".into(),
        },
        LayerError::CoreRejection {
            result: ResultCode::from_raw(-3),
        },
        LayerError::VerificationFailure {
            check: "signature".into(),
        },
        LayerError::PolicyRefusal {
            rule: "default-deny".into(),
        },
        LayerError::CapabilityRefusal {
            dimension: "asset".into(),
        },
        LayerError::BudgetRefusal {
            budget: "daily".into(),
        },
        LayerError::RateLimit {
            bucket: "submit".into(),
        },
        LayerError::InternalFault {
            invariant: "state".into(),
        },
    ];
    let classes: BTreeSet<ErrorClass> = errors.iter().map(LayerError::class).collect();
    assert_eq!(classes.len(), errors.len());
    assert!(classes.contains(&ErrorClass::TransportFailure));
    assert!(classes.contains(&ErrorClass::VerificationFailure));
}

#[test]
fn protocol_mapping_is_total_unique_and_lossless() {
    let mut raw_values = BTreeSet::new();
    for known in KnownResult::ALL {
        assert!(raw_values.insert(known.raw()), "duplicate result number");
        let code = ResultCode::from(*known);
        assert_eq!(code.known(), Some(*known));
        assert_eq!(code.raw(), known.raw());
        assert!(matches!(
            code.retriability(),
            Retriability::Terminal | Retriability::Retriable
        ));
    }
}

#[test]
fn unknown_numbers_round_trip_verbatim_and_fail_closed() {
    let unknown = ResultCode::from_raw(-7777);
    assert_eq!(unknown.raw(), -7777);
    assert_eq!(unknown.known(), None);
    assert_eq!(unknown.domain(), ResultDomain::Fatal);
    assert_eq!(unknown.retriability(), Retriability::Terminal);
}

#[test]
fn domain_partition_matches_the_core() {
    assert_eq!(ResultCode::from_raw(0).domain(), ResultDomain::Success);
    assert_eq!(ResultCode::from_raw(-1).domain(), ResultDomain::Codec);
    assert_eq!(ResultCode::from_raw(-100).domain(), ResultDomain::Envelope);
    assert_eq!(ResultCode::from_raw(-200).domain(), ResultDomain::Authority);
    assert_eq!(
        ResultCode::from_raw(-300).domain(),
        ResultDomain::Sequencing
    );
    assert_eq!(ResultCode::from_raw(-400).domain(), ResultDomain::Ledger);
    assert_eq!(
        ResultCode::from_raw(-500).domain(),
        ResultDomain::Arithmetic
    );
    assert_eq!(ResultCode::from_raw(-600).domain(), ResultDomain::Metering);
    assert_eq!(ResultCode::from_raw(-700).domain(), ResultDomain::Module);
    assert_eq!(ResultCode::from_raw(-800).domain(), ResultDomain::Batch);
    assert_eq!(ResultCode::from_raw(-900).domain(), ResultDomain::Storage);
    assert_eq!(ResultCode::from_raw(-1000).domain(), ResultDomain::Fatal);
    assert_eq!(ResultCode::from_raw(1).domain(), ResultDomain::Unknown);
}
