use std::any::TypeId;

use layerx_types::account::{AccountError, AccountId, AccountNamespace};
use layerx_types::amount::{Amount, ArithmeticError, U256};
use layerx_types::ids::{ActivityId, AssetId, BatchId, Did, IdempotencyKey};
use layerx_types::limits::{MAX_ACCOUNT_NAME_BYTES, MAX_DID_BYTES};

#[test]
fn identifier_types_are_not_interchangeable() {
    assert_ne!(TypeId::of::<ActivityId>(), TypeId::of::<BatchId>());
    assert_ne!(TypeId::of::<ActivityId>(), TypeId::of::<AssetId>());
    assert_ne!(TypeId::of::<AssetId>(), TypeId::of::<IdempotencyKey>());
    assert_eq!(ActivityId::new([7; 32]).bytes(), [7; 32]);
}

#[test]
fn did_and_account_bounds_are_constructor_enforced() {
    assert!(Did::new(&vec![1; MAX_DID_BYTES]).is_ok());
    assert!(Did::new(&vec![1; MAX_DID_BYTES + 1]).is_err());
    assert!(Did::new(&[]).is_err());

    let oversized = format!("agent:{}:main", "a".repeat(MAX_ACCOUNT_NAME_BYTES));
    assert_eq!(AccountId::parse(&oversized), Err(AccountError::Length));
}

#[test]
fn only_declared_account_namespaces_construct() {
    let accepted = [
        ("agent:did:key:alice:main", AccountNamespace::AgentMain),
        (
            "agent:did:key:alice:budget:daily",
            AccountNamespace::AgentBudget,
        ),
        (
            "agent:did:key:alice:escrow:order-7",
            AccountNamespace::AgentEscrow,
        ),
        (
            "agent:did:key:alice:margin:btc-usd",
            AccountNamespace::AgentMargin,
        ),
        (
            "system:liquidity:btc-usd",
            AccountNamespace::SystemLiquidity,
        ),
        ("system:insurance", AccountNamespace::SystemInsurance),
        ("system:fees", AccountNamespace::SystemFees),
        (
            "system:paxeer-reserve",
            AccountNamespace::SystemPaxeerReserve,
        ),
        (
            "system:paxeer-withdrawals",
            AccountNamespace::SystemPaxeerWithdrawals,
        ),
    ];
    for (canonical, namespace) in accepted {
        let Ok(account) = AccountId::parse(canonical) else {
            panic!("declared namespace rejected: {canonical}");
        };
        assert_eq!(account.namespace(), namespace);
        assert_eq!(account.canonical(), canonical);
    }
    for rejected in [
        "agent:did:key:alice:stream:salary",
        "system:funding:btc-usd:long",
        "system:mint",
        "agent::main",
        "agent:did:key:alice:budget:",
    ] {
        assert!(AccountId::parse(rejected).is_err(), "accepted {rejected}");
    }
}

#[test]
fn amounts_use_checked_integer_arithmetic_only() {
    assert_eq!(
        Amount::from_u128(9).checked_add(Amount::from_u128(3)),
        Ok(Amount::from_u128(12))
    );
    assert_eq!(
        Amount::from_u128(u128::MAX).checked_add(Amount::from_u128(1)),
        Err(ArithmeticError::Overflow)
    );
    assert_eq!(
        Amount::ZERO.checked_sub(Amount::from_u128(1)),
        Err(ArithmeticError::Underflow)
    );
    assert_eq!(
        U256::from_words([u64::MAX; 4]).checked_add(U256::from_words([1, 0, 0, 0])),
        Err(ArithmeticError::Overflow)
    );
}
