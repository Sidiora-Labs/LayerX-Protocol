//! x402 v2 facilitator role. Verification is a read-only translation and
//! settlement can report success only after the gateway verifies canonical
//! `LayerX` receipt evidence returned by the plane authority.

use std::collections::BTreeMap;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use layerx_interop_gateway::adapter::AdapterId;
use layerx_interop_gateway::gateway::{TranslationKind, TranslationRequest, TranslationStatus};
use layerx_interop_gateway::principal::PrincipalId;
use layerx_interop_gateway::trace::{TraceId, Traced};
use layerx_interop_gateway::GatewayCore;
use layerx_proof::receipt::verify;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use crate::model::{
    PaymentPayload, PaymentRequirements, SettlementResponse, X402Error, X402_VERSION,
};
use crate::seller::ExecutedPayment;

const ADAPTER_ID: &str = "x402";
const VERIFY_DIGEST_DOMAIN: &[u8] = b"LayerX/x402/v2/facilitator/verify/digest\0";
const VERIFY_KEY_DOMAIN: &[u8] = b"LayerX/x402/v2/facilitator/verify/key\0";
const SETTLE_DIGEST_DOMAIN: &[u8] = b"LayerX/x402/v2/facilitator/settle/digest\0";
const SETTLE_KEY_DOMAIN: &[u8] = b"LayerX/x402/v2/facilitator/settle/key\0";

/// Exact request body shared by the standard `/verify` and `/settle`
/// facilitator endpoints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FacilitatorRequest {
    pub x402_version: u8,
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
}

impl FacilitatorRequest {
    pub(crate) fn validate(&self) -> Result<(), X402Error> {
        if self.x402_version != X402_VERSION {
            return Err(X402Error::WrongVersion);
        }
        self.payment_payload.validate()?;
        self.payment_requirements.validate()?;
        if self.payment_payload.accepted != self.payment_requirements {
            return Err(X402Error::RequirementsMismatch);
        }
        Ok(())
    }
}

/// Exact standard response from the facilitator's read-only `/verify` API.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VerifyResponse {
    pub is_valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalid_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

/// One scheme/network capability from the standard `/supported` response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FacilitatorKind {
    pub x402_version: u8,
    pub scheme: String,
    pub network: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

/// Exact standard response from the facilitator's `/supported` API.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportedResponse {
    pub kinds: Vec<FacilitatorKind>,
    pub extensions: Vec<String>,
    pub signers: BTreeMap<String, Vec<String>>,
}

/// Server-led settlement step. The distinction is committed to both the
/// request digest and gateway identity so escrow's two legitimate settle
/// calls cannot alias one another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementStep {
    Single,
    EscrowDeposit,
    EscrowCharge,
}

impl SettlementStep {
    const fn code(self) -> u8 {
        match self {
            Self::Single => 1,
            Self::EscrowDeposit => 2,
            Self::EscrowCharge => 3,
        }
    }
}

/// Typed, digest-bound request handed to the real facilitator plane.
#[derive(Clone, Debug, PartialEq)]
pub struct FacilitatorPaymentRequest {
    pub payment_payload: PaymentPayload,
    pub payment_requirements: PaymentRequirements,
    pub idempotency_key: [u8; 32],
    pub request_digest: [u8; 32],
    pub settlement_step: SettlementStep,
}

/// Honest result of a read-only plane verification.
#[derive(Clone, Debug, PartialEq)]
pub enum PlaneVerifyOutcome {
    Valid {
        payer: Option<String>,
        extra: Option<Value>,
    },
    Invalid {
        reason: &'static str,
        payer: Option<String>,
    },
}

/// Honest result of a state-committing plane settlement.
#[derive(Debug)]
pub enum FacilitatorSettlementOutcome {
    Pending {
        transaction: String,
    },
    Refused {
        reason: &'static str,
        payer: Option<String>,
    },
    Executed(ExecutedPayment),
}

/// The only authority boundary used by the facilitator. Verification and
/// settlement are deliberately separate methods, making a state-changing
/// implementation impossible to call through the read-only route by mistake.
pub trait FacilitatorPlane {
    /// Validates without committing payment state.
    ///
    /// # Errors
    ///
    /// Returns a typed scheme, policy, or network-observation refusal.
    fn verify(
        &mut self,
        request: &FacilitatorPaymentRequest,
        trace: &TraceId,
    ) -> Result<PlaneVerifyOutcome, X402Error>;

    /// Idempotently commits payment state for one stable request identity.
    ///
    /// # Errors
    ///
    /// Returns a typed scheme, policy, or settlement refusal.
    fn settle(
        &mut self,
        request: FacilitatorPaymentRequest,
        trace: &TraceId,
    ) -> Result<FacilitatorSettlementOutcome, X402Error>;
}

/// Receipt-backed implementation of the x402 v2 facilitator APIs.
#[derive(Clone, Debug)]
pub struct Facilitator {
    supported: SupportedResponse,
}

impl Facilitator {
    /// Creates a facilitator from a closed, bounded support declaration.
    ///
    /// # Errors
    ///
    /// Refuses empty, duplicate, malformed, or oversized declarations.
    pub fn new(supported: SupportedResponse) -> Result<Self, X402Error> {
        validate_supported(&supported)?;
        Ok(Self { supported })
    }

    /// Returns the immutable `/supported` response.
    #[must_use]
    pub fn supported(&self) -> SupportedResponse {
        self.supported.clone()
    }

    /// Implements read-only `/verify`. The gateway record is explicitly
    /// read-only and the plane's settlement method is not reachable here.
    ///
    /// # Errors
    ///
    /// Returns trace-bound wire, support, gateway, or plane failures.
    pub fn verify(
        &self,
        gateway: &mut GatewayCore,
        principal: &PrincipalId,
        request: &FacilitatorRequest,
        plane: &mut impl FacilitatorPlane,
        trace: &TraceId,
        now: u64,
    ) -> Result<VerifyResponse, Traced<X402Error>> {
        let fail = |error| trace.wrap(error);
        request.validate().map_err(fail)?;
        self.require_supported(&request.payment_requirements)
            .map_err(fail)?;
        let canonical = serde_json::to_vec(request).map_err(|_| fail(X402Error::Encode))?;
        let request_digest = digest(VERIFY_DIGEST_DOMAIN, &[&canonical]);
        let idempotency_key = digest(
            VERIFY_KEY_DOMAIN,
            &[principal.as_str().as_bytes(), &request_digest],
        );
        let translation = translation(TranslationKind::ReadOnly, idempotency_key, request_digest)
            .map_err(fail)?;
        let opened = gateway
            .begin_translation(principal, &translation, trace, now)
            .map_err(|error| trace.wrap(X402Error::Gateway(error.into_error())))?;
        if opened == TranslationStatus::Refused {
            return Err(fail(X402Error::PaymentRefused));
        }
        let plane_request = FacilitatorPaymentRequest {
            payment_payload: request.payment_payload.clone(),
            payment_requirements: request.payment_requirements.clone(),
            idempotency_key,
            request_digest,
            settlement_step: SettlementStep::Single,
        };
        let outcome = plane.verify(&plane_request, trace).map_err(fail)?;
        gateway
            .complete_read_only(principal, idempotency_key, trace, now)
            .map_err(|error| trace.wrap(X402Error::Gateway(error.into_error())))?;
        render_verify(outcome).map_err(fail)
    }

    /// Implements state-committing `/settle`. The caller's stable identity is
    /// mandatory and a successful response is unreachable without matching,
    /// locally verified protocol evidence.
    ///
    /// # Errors
    ///
    /// Returns trace-bound wire, support, gateway, plane, or evidence failures.
    #[allow(clippy::too_many_arguments)]
    pub fn settle(
        &self,
        gateway: &mut GatewayCore,
        principal: &PrincipalId,
        request: &FacilitatorRequest,
        stable_identity: [u8; 32],
        step: SettlementStep,
        plane: &mut impl FacilitatorPlane,
        trace: &TraceId,
        now: u64,
    ) -> Result<SettlementResponse, Traced<X402Error>> {
        let fail = |error| trace.wrap(error);
        if stable_identity == [0; 32] {
            return Err(fail(X402Error::InvalidPayload));
        }
        request.validate().map_err(fail)?;
        self.require_supported(&request.payment_requirements)
            .map_err(fail)?;
        let canonical = serde_json::to_vec(request).map_err(|_| fail(X402Error::Encode))?;
        let step_code = [step.code()];
        let request_digest = digest(SETTLE_DIGEST_DOMAIN, &[&step_code, &canonical]);
        let idempotency_key = digest(
            SETTLE_KEY_DOMAIN,
            &[principal.as_str().as_bytes(), &stable_identity, &step_code],
        );
        let translation = translation(
            TranslationKind::StateChanging,
            idempotency_key,
            request_digest,
        )
        .map_err(fail)?;
        let opened = gateway
            .begin_translation(principal, &translation, trace, now)
            .map_err(|error| trace.wrap(X402Error::Gateway(error.into_error())))?;
        if opened == TranslationStatus::Refused {
            return render_refused(&request.payment_requirements, "payment_refused", None)
                .map_err(fail);
        }
        let plane_request = FacilitatorPaymentRequest {
            payment_payload: request.payment_payload.clone(),
            payment_requirements: request.payment_requirements.clone(),
            idempotency_key,
            request_digest,
            settlement_step: step,
        };
        match plane.settle(plane_request, trace).map_err(fail)? {
            FacilitatorSettlementOutcome::Pending { transaction } => {
                if matches!(opened, TranslationStatus::ReceiptVerified { .. }) {
                    return Err(fail(X402Error::EvidenceMissing));
                }
                render_pending(&request.payment_requirements, transaction).map_err(fail)
            }
            FacilitatorSettlementOutcome::Refused { reason, payer } => {
                if matches!(opened, TranslationStatus::ReceiptVerified { .. }) {
                    return Err(fail(X402Error::EvidenceMismatch));
                }
                gateway
                    .refuse_translation(principal, idempotency_key, trace, now)
                    .map_err(|error| trace.wrap(X402Error::Gateway(error.into_error())))?;
                render_refused(&request.payment_requirements, reason, payer).map_err(fail)
            }
            FacilitatorSettlementOutcome::Executed(executed) => settle_executed(
                gateway,
                principal,
                idempotency_key,
                &request.payment_requirements,
                &executed,
                trace,
                now,
            )
            .map_err(fail),
        }
    }

    fn require_supported(&self, requirements: &PaymentRequirements) -> Result<(), X402Error> {
        if self.supported.kinds.iter().any(|kind| {
            kind.x402_version == X402_VERSION
                && kind.scheme == requirements.scheme
                && kind.network == requirements.network
        }) {
            Ok(())
        } else {
            Err(X402Error::UnsupportedOffer)
        }
    }
}

fn translation(
    kind: TranslationKind,
    idempotency_key: [u8; 32],
    request_digest: [u8; 32],
) -> Result<TranslationRequest, X402Error> {
    let adapter = AdapterId::new(ADAPTER_ID).map_err(|error| X402Error::Gateway(error.into()))?;
    TranslationRequest::new(adapter, kind, idempotency_key, request_digest)
        .map_err(X402Error::Gateway)
}

fn render_verify(outcome: PlaneVerifyOutcome) -> Result<VerifyResponse, X402Error> {
    let response = match outcome {
        PlaneVerifyOutcome::Valid { payer, extra } => VerifyResponse {
            is_valid: true,
            invalid_reason: None,
            payer,
            extra,
        },
        PlaneVerifyOutcome::Invalid { reason, payer } => VerifyResponse {
            is_valid: false,
            invalid_reason: Some(safe_reason(reason)),
            payer,
            extra: None,
        },
    };
    validate_verify(&response)?;
    Ok(response)
}

fn render_pending(
    requirements: &PaymentRequirements,
    transaction: String,
) -> Result<SettlementResponse, X402Error> {
    let response = SettlementResponse {
        success: false,
        error_reason: Some("settlement_pending".to_owned()),
        payer: None,
        transaction,
        network: requirements.network.clone(),
        amount: None,
        extensions: BTreeMap::new(),
    };
    response.validate_wire()?;
    Ok(response)
}

fn render_refused(
    requirements: &PaymentRequirements,
    reason: &str,
    payer: Option<String>,
) -> Result<SettlementResponse, X402Error> {
    let response = SettlementResponse {
        success: false,
        error_reason: Some(safe_reason(reason)),
        payer,
        transaction: String::new(),
        network: requirements.network.clone(),
        amount: None,
        extensions: BTreeMap::new(),
    };
    response.validate_wire()?;
    Ok(response)
}

#[allow(clippy::too_many_arguments)]
fn settle_executed(
    gateway: &mut GatewayCore,
    principal: &PrincipalId,
    idempotency_key: [u8; 32],
    requirements: &PaymentRequirements,
    executed: &ExecutedPayment,
    trace: &TraceId,
    now: u64,
) -> Result<SettlementResponse, X402Error> {
    let verified = verify(&executed.canonical_receipt, &executed.authorised_batch)
        .map_err(|_| X402Error::EvidenceMismatch)?;
    let protocol = verified
        .receipt()
        .protocol()
        .ok_or(X402Error::EvidenceMismatch)?;
    let (asset, recipient) = requirements.layerx_facts()?;
    if protocol.asset() != asset
        || protocol.to() != recipient
        || protocol.amount() != requirements.amount.value()
    {
        return Err(X402Error::EvidenceMismatch);
    }
    let status = gateway
        .settle_with_receipt(
            principal,
            idempotency_key,
            &executed.canonical_receipt,
            &executed.authorised_batch,
            trace,
            now,
        )
        .map_err(|error| X402Error::Gateway(error.into_error()))?;
    let TranslationStatus::ReceiptVerified { receipt_digest } = status else {
        return Err(X402Error::EvidenceMissing);
    };
    let digest_text = hex(&receipt_digest);
    let mut extensions = BTreeMap::new();
    extensions.insert(
        "layerx".to_owned(),
        json!({
            "receipt": STANDARD.encode(&executed.canonical_receipt),
            "receiptDigest": digest_text,
            "verificationLevel": "sequencer-signed"
        }),
    );
    let response = SettlementResponse {
        success: true,
        error_reason: None,
        payer: Some(hex(&protocol.from())),
        transaction: format!("lxp:{digest_text}"),
        network: requirements.network.clone(),
        amount: Some(requirements.amount),
        extensions,
    };
    response.validate_wire()?;
    Ok(response)
}

pub(crate) fn validate_verify(response: &VerifyResponse) -> Result<(), X402Error> {
    if response
        .payer
        .as_ref()
        .is_some_and(|payer| !bounded(payer, 256))
        || response
            .invalid_reason
            .as_ref()
            .is_some_and(|reason| !bounded(reason, 512))
        || response
            .extra
            .as_ref()
            .is_some_and(|extra| !extra.is_object())
        || response.is_valid == response.invalid_reason.is_some()
    {
        return Err(X402Error::InvalidPayload);
    }
    Ok(())
}

pub(crate) fn validate_supported(response: &SupportedResponse) -> Result<(), X402Error> {
    if response.kinds.is_empty()
        || response.kinds.len() > 64
        || response.extensions.len() > 32
        || response.signers.len() > 64
    {
        return Err(X402Error::UnsupportedOffer);
    }
    for (index, kind) in response.kinds.iter().enumerate() {
        if kind.x402_version != X402_VERSION
            || !identifier(&kind.scheme, 32)
            || !caip2(&kind.network)
            || kind.extra.as_ref().is_some_and(|extra| !extra.is_object())
            || response.kinds[..index].iter().any(|prior| {
                prior.x402_version == kind.x402_version
                    && prior.scheme == kind.scheme
                    && prior.network == kind.network
            })
        {
            return Err(X402Error::UnsupportedOffer);
        }
    }
    for (index, extension) in response.extensions.iter().enumerate() {
        if !identifier(extension, 32) || response.extensions[..index].contains(extension) {
            return Err(X402Error::UnsupportedOffer);
        }
    }
    for (pattern, signers) in &response.signers {
        if !network_pattern(pattern)
            || signers.is_empty()
            || signers.len() > 64
            || signers.iter().any(|signer| !bounded(signer, 256))
        {
            return Err(X402Error::UnsupportedOffer);
        }
    }
    Ok(())
}

fn network_pattern(value: &str) -> bool {
    let Some((namespace, reference)) = value.split_once(':') else {
        return false;
    };
    identifier(namespace, 32) && (reference == "*" || identifier(reference, 64))
}

fn caip2(value: &str) -> bool {
    let Some((namespace, reference)) = value.split_once(':') else {
        return false;
    };
    identifier(namespace, 32) && identifier(reference, 64)
}

fn identifier(value: &str, limit: usize) -> bool {
    bounded(value, limit)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn bounded(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.contains('\0')
}

fn safe_reason(reason: &str) -> String {
    if !reason.is_empty()
        && reason.len() <= 64
        && reason
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
    {
        reason.to_owned()
    } else {
        "payment_refused".to_owned()
    }
}

fn digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update(part);
    }
    hash.finalize().into()
}

fn hex(bytes: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}
