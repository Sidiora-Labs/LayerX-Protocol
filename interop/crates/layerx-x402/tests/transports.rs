use std::collections::BTreeMap;

use layerx_interop_gateway::principal::PrincipalId;
use layerx_x402::facilitator::{
    FacilitatorKind, FacilitatorRequest, SettlementIdentity, SettlementStep, SupportedResponse,
    VerifyResponse,
};
use layerx_x402::model::{
    AtomicAmount, PaymentPayload, PaymentRequired, PaymentRequirements, ResourceInfo,
    SettlementResponse, X402_VERSION,
};
use layerx_x402::transport::{
    decode_facilitator_request, decode_facilitator_settlement, decode_payment_payload,
    decode_payment_required, decode_settlement, decode_supported_response, decode_verify_response,
    encode_facilitator_request, encode_facilitator_settlement, encode_payment_payload,
    encode_payment_required, encode_settlement, encode_supported_response, encode_verify_response,
    TransportKind, TRANSPORT_MATRIX,
};
use serde_json::json;

const TRANSPORTS: [TransportKind; 3] =
    [TransportKind::Http, TransportKind::Mcp, TransportKind::A2a];

fn requirements() -> PaymentRequirements {
    PaymentRequirements {
        scheme: "exact".to_owned(),
        network: "layerx:testnet".to_owned(),
        amount: AtomicAmount::from_u128(25),
        asset: "05".repeat(32),
        pay_to: "07".repeat(32),
        max_timeout_seconds: 60,
        extra: None,
    }
}

fn required() -> PaymentRequired {
    PaymentRequired {
        x402_version: X402_VERSION,
        error: None,
        resource: ResourceInfo {
            url: "https://merchant.example/resource".to_owned(),
            description: Some("Paid resource".to_owned()),
            mime_type: Some("application/json".to_owned()),
            service_name: Some("Merchant".to_owned()),
            tags: vec!["api".to_owned()],
            icon_url: None,
        },
        accepts: vec![requirements()],
        extensions: BTreeMap::new(),
    }
}

fn payload() -> PaymentPayload {
    PaymentPayload {
        x402_version: X402_VERSION,
        resource: None,
        payload: json!({"authorization": "opaque-signed-payment"}),
        accepted: requirements(),
        extensions: BTreeMap::new(),
    }
}

fn facilitator_request() -> FacilitatorRequest {
    FacilitatorRequest {
        x402_version: X402_VERSION,
        payment_payload: payload(),
        payment_requirements: requirements(),
    }
}

#[test]
fn every_role_round_trips_on_every_transport() {
    let required = required();
    let payload = payload();
    let request = facilitator_request();
    let verify = VerifyResponse {
        is_valid: true,
        invalid_reason: None,
        payer: Some("did:layerx:payer".to_owned()),
        extra: Some(json!({"scheme": "exact"})),
    };
    let settlement = SettlementResponse {
        success: false,
        error_reason: Some("settlement_pending".to_owned()),
        payer: None,
        transaction: "pending:provider-reference".to_owned(),
        network: "layerx:testnet".to_owned(),
        amount: None,
        extensions: BTreeMap::new(),
    };
    let supported = SupportedResponse {
        kinds: vec![FacilitatorKind {
            x402_version: X402_VERSION,
            scheme: "exact".to_owned(),
            network: "layerx:testnet".to_owned(),
            extra: None,
        }],
        extensions: Vec::new(),
        signers: BTreeMap::from([(
            "layerx:*".to_owned(),
            vec!["did:layerx:facilitator".to_owned()],
        )]),
    };

    for transport in TRANSPORTS {
        let encoded = encode_payment_required(transport, &required)
            .unwrap_or_else(|error| panic!("required encode: {error}"));
        assert_eq!(
            decode_payment_required(transport, &encoded),
            Ok(required.clone())
        );
        let encoded = encode_payment_payload(transport, &payload)
            .unwrap_or_else(|error| panic!("payload encode: {error}"));
        assert_eq!(
            decode_payment_payload(transport, &encoded),
            Ok(payload.clone())
        );
        let encoded = encode_settlement(transport, &settlement)
            .unwrap_or_else(|error| panic!("settlement encode: {error}"));
        assert_eq!(
            decode_settlement(transport, &encoded),
            Ok(settlement.clone())
        );
        let encoded = encode_facilitator_request(transport, &request)
            .unwrap_or_else(|error| panic!("facilitator request encode: {error}"));
        assert_eq!(
            decode_facilitator_request(transport, &encoded),
            Ok(request.clone())
        );
        let encoded = encode_verify_response(transport, &verify)
            .unwrap_or_else(|error| panic!("verify encode: {error}"));
        assert_eq!(
            decode_verify_response(transport, &encoded),
            Ok(verify.clone())
        );
        let encoded = encode_facilitator_settlement(transport, &settlement)
            .unwrap_or_else(|error| panic!("facilitator settlement encode: {error}"));
        assert_eq!(
            decode_facilitator_settlement(transport, &encoded),
            Ok(settlement.clone())
        );
        let encoded = encode_supported_response(transport, &supported)
            .unwrap_or_else(|error| panic!("supported encode: {error}"));
        assert_eq!(
            decode_supported_response(transport, &encoded),
            Ok(supported.clone())
        );
    }
    assert!(TRANSPORT_MATRIX
        .iter()
        .all(|row| row.buyer && row.seller && row.facilitator));
}

#[test]
fn settlement_identity_is_transport_independent_and_step_separated() {
    let principal =
        PrincipalId::new("merchant-a").unwrap_or_else(|error| panic!("principal: {error:?}"));
    let request = facilitator_request();
    let stable = [9; 32];
    let baseline = SettlementIdentity::derive(&principal, &request, stable, SettlementStep::Single)
        .unwrap_or_else(|error| panic!("identity: {error}"));
    for transport in TRANSPORTS {
        let encoded = encode_facilitator_request(transport, &request)
            .unwrap_or_else(|error| panic!("encode: {error}"));
        let decoded = decode_facilitator_request(transport, &encoded)
            .unwrap_or_else(|error| panic!("decode: {error}"));
        assert_eq!(
            SettlementIdentity::derive(&principal, &decoded, stable, SettlementStep::Single),
            Ok(baseline)
        );
    }
    let deposit =
        SettlementIdentity::derive(&principal, &request, stable, SettlementStep::EscrowDeposit)
            .unwrap_or_else(|error| panic!("deposit identity: {error}"));
    let charge =
        SettlementIdentity::derive(&principal, &request, stable, SettlementStep::EscrowCharge)
            .unwrap_or_else(|error| panic!("charge identity: {error}"));
    assert_ne!(baseline.idempotency_key, deposit.idempotency_key);
    assert_ne!(deposit.idempotency_key, charge.idempotency_key);
    assert_ne!(deposit.request_digest, charge.request_digest);
}
