use layerx_agentd::budget::{
    reconcile, LocalAccounting, ProtocolBudgetState, VerifiedSpendReceipt,
};

fn protocol(consumed: u128, start: u64) -> ProtocolBudgetState {
    ProtocolBudgetState {
        consumed,
        remaining: 1_000 - consumed,
        window_start_sequence: start,
        window_end_sequence: start + 99,
        observed_head_sequence: start + 20,
        verified: true,
    }
}

#[test]
fn rollover_comes_only_from_protocol_state() {
    let mut local = LocalAccounting {
        consumed: 900,
        window_start_sequence: 1,
        last_receipt: Some([1; 32]),
    };
    let state = reconcile(&mut local, protocol(0, 100), &[])
        .unwrap_or_else(|error| panic!("reconcile: {error:?}"));
    assert_eq!(state.window_start_sequence, 100);
    assert_eq!(state.window_end_sequence, 199);
    assert_eq!(state.remaining, 1_000);
    assert_eq!(local.consumed, 0);
    assert_eq!(state.divergence, Some(-900));
}

#[test]
fn missed_receipt_divergence_is_exposed_and_cache_is_corrected() {
    let mut local = LocalAccounting {
        consumed: 200,
        window_start_sequence: 100,
        last_receipt: Some([1; 32]),
    };
    let receipts = [VerifiedSpendReceipt {
        receipt_id: [2; 32],
        amount: 200,
        window_start_sequence: 100,
        verified: true,
    }];
    let state = reconcile(&mut local, protocol(350, 100), &receipts)
        .unwrap_or_else(|error| panic!("reconcile: {error:?}"));
    assert_eq!(state.local_before, 200);
    assert_eq!(state.protocol_consumed, 350);
    assert_eq!(state.local_after, 350);
    assert_eq!(state.divergence, Some(150));
    assert_eq!(state.last_verified_receipt, Some([2; 32]));
}

#[test]
fn unverified_inputs_never_correct_the_cache() {
    let mut local = LocalAccounting {
        consumed: 200,
        window_start_sequence: 100,
        last_receipt: None,
    };
    let mut unverified = protocol(350, 100);
    unverified.verified = false;
    assert!(reconcile(&mut local, unverified, &[]).is_err());
    assert_eq!(local.consumed, 200);
}
