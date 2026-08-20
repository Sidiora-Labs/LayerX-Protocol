#![forbid(unsafe_code)]

use std::fmt::{Debug, Display, Formatter};

use layerx_interop_gateway::adapter::{AdapterDescriptor, AdapterId, ConformanceSuite, PinnedSpec};
use layerx_interop_gateway::error::GatewayError;
use layerx_interop_gateway::gateway::{TranslationKind, TranslationRequest, TranslationStatus};
use layerx_interop_gateway::principal::PrincipalId;
use layerx_interop_gateway::trace::{TraceId, Traced};
use layerx_interop_gateway::GatewayCore;
use layerx_proof::receipt::{verify, AuthorizedBatch, VerifiedReceipt};
use sha2::{Digest as _, Sha256};

const ADAPTER_ID: &str = "migration";
const EVIDENCE_LIMIT: usize = 1024 * 1024;
const HISTORY_PAGE_LIMIT: usize = 256;
const REQUEST_DOMAIN: &[u8] = b"LayerX/interop/migration/request/v1\0";
const IDEMPOTENCY_DOMAIN: &[u8] = b"LayerX/interop/migration/idempotency/v1\0";

/// Source chains supported by the migration boundary. Each value commits to
/// the exact network, preventing an address or transaction from silently
/// crossing between networks.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SourceChain {
    Ethereum { chain_id: u64 },
    Solana { genesis_hash: [u8; 32] },
}

impl SourceChain {
    fn validate(self) -> Result<(), MigrationError> {
        match self {
            Self::Ethereum { chain_id } if chain_id != 0 => Ok(()),
            Self::Solana { genesis_hash } if genesis_hash != [0; 32] => Ok(()),
            Self::Ethereum { .. } | Self::Solana { .. } => Err(MigrationError::InvalidNetwork),
        }
    }

    fn commit(self, hash: &mut Sha256) {
        match self {
            Self::Ethereum { chain_id } => {
                hash.update([1]);
                hash.update(chain_id.to_be_bytes());
            }
            Self::Solana { genesis_hash } => {
                hash.update([2]);
                hash.update(genesis_hash);
            }
        }
    }
}

/// An address whose byte length is fixed by its source chain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExternalAddress {
    Ethereum([u8; 20]),
    Solana([u8; 32]),
}

impl ExternalAddress {
    fn validate_for(self, chain: SourceChain) -> Result<(), MigrationError> {
        match (chain, self) {
            (SourceChain::Ethereum { .. }, Self::Ethereum(address)) if address != [0; 20] => Ok(()),
            (SourceChain::Solana { .. }, Self::Solana(address)) if address != [0; 32] => Ok(()),
            (_, Self::Ethereum(address)) if address == [0; 20] => {
                Err(MigrationError::InvalidAddress)
            }
            (_, Self::Solana(address)) if address == [0; 32] => Err(MigrationError::InvalidAddress),
            (_, Self::Ethereum(_) | Self::Solana(_)) => Err(MigrationError::AddressChainMismatch),
        }
    }

    fn commit(self, hash: &mut Sha256) {
        match self {
            Self::Ethereum(address) => {
                hash.update([1]);
                hash.update(address);
            }
            Self::Solana(address) => {
                hash.update([2]);
                hash.update(address);
            }
        }
    }
}

/// Exact source-chain transaction identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceTransaction([u8; 32]);

impl SourceTransaction {
    /// Creates a non-zero source-chain transaction identifier.
    ///
    /// # Errors
    ///
    /// Refuses the reserved zero identifier.
    pub fn new(value: [u8; 32]) -> Result<Self, MigrationError> {
        if value == [0; 32] {
            Err(MigrationError::InvalidTransaction)
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Bounded source evidence. Raw bytes never enter debug, display, telemetry,
/// or gateway audit records; those boundaries receive only this digest.
pub struct SourceEvidence {
    canonical: Vec<u8>,
    digest: [u8; 32],
}

impl SourceEvidence {
    /// Commits to one canonical source-chain proof envelope.
    ///
    /// # Errors
    ///
    /// Refuses empty and oversized evidence.
    pub fn new(canonical: Vec<u8>) -> Result<Self, MigrationError> {
        if canonical.is_empty() || canonical.len() > EVIDENCE_LIMIT {
            return Err(MigrationError::InvalidEvidence);
        }
        let digest = Sha256::digest(&canonical).into();
        Ok(Self { canonical, digest })
    }

    #[must_use]
    pub fn canonical(&self) -> &[u8] {
        &self.canonical
    }

    #[must_use]
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

impl Debug for SourceEvidence {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceEvidence")
            .field("digest", &self.digest)
            .field("canonical", &"[REDACTED]")
            .finish()
    }
}

/// Source ownership facts after chain-specific signature verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedOwnership {
    pub chain: SourceChain,
    pub address: ExternalAddress,
    pub layerx_identity: [u8; 32],
    pub evidence_digest: [u8; 32],
}

impl VerifiedOwnership {
    fn validate(self, evidence: &SourceEvidence) -> Result<(), MigrationError> {
        self.chain.validate()?;
        self.address.validate_for(self.chain)?;
        if self.layerx_identity == [0; 32] || self.evidence_digest != evidence.digest() {
            return Err(MigrationError::EvidenceMismatch);
        }
        Ok(())
    }
}

/// Final source-chain custody facts. Construction is delegated to a real
/// chain verifier; the adapter rechecks all closed invariants before credit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedAssetFinality {
    pub chain: SourceChain,
    pub transaction: SourceTransaction,
    pub source: ExternalAddress,
    pub source_asset: [u8; 32],
    pub source_amount: u128,
    pub custody_reference: [u8; 32],
    pub layerx_asset: [u8; 32],
    pub layerx_amount: u128,
    pub destination: [u8; 32],
    pub finality_height: u64,
    pub evidence_digest: [u8; 32],
}

impl VerifiedAssetFinality {
    fn validate(self, evidence: &SourceEvidence) -> Result<(), MigrationError> {
        self.chain.validate()?;
        self.source.validate_for(self.chain)?;
        if self.source_asset == [0; 32]
            || self.source_amount == 0
            || self.custody_reference == [0; 32]
            || self.layerx_asset == [0; 32]
            || self.layerx_amount == 0
            || self.destination == [0; 32]
            || self.finality_height == 0
            || self.evidence_digest != evidence.digest()
        {
            return Err(MigrationError::EvidenceMismatch);
        }
        Ok(())
    }
}

/// A chain-specific verifier is the only boundary allowed to turn canonical
/// source evidence into ownership, finality, or history facts.
pub trait SourceVerifier {
    /// Verifies address ownership under the exact requested network.
    ///
    /// # Errors
    ///
    /// Returns a closed source verification refusal.
    fn verify_ownership(
        &self,
        evidence: &SourceEvidence,
        trace: &TraceId,
    ) -> Result<VerifiedOwnership, MigrationError>;

    /// Verifies source inclusion, execution, custody lock, and finality.
    ///
    /// # Errors
    ///
    /// Returns pending while finality is not established, and a terminal
    /// refusal for displaced, reverted, malformed, or mismatched evidence.
    fn verify_asset_finality(
        &self,
        evidence: &SourceEvidence,
        trace: &TraceId,
    ) -> Result<VerifiedAssetFinality, MigrationError>;

    /// Verifies a bounded page of source history.
    ///
    /// # Errors
    ///
    /// Refuses unauthenticated, unbounded, or internally inconsistent pages.
    fn verify_history(
        &self,
        evidence: &SourceEvidence,
        trace: &TraceId,
    ) -> Result<VerifiedHistoryPage, MigrationError>;
}

/// The source-chain meaning of one imported transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalHistoryKind {
    Incoming,
    Outgoing,
    Contract,
}

/// An imported history record. Its provenance is structurally external and
/// the type contains no field capable of carrying a `LayerX` receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalHistoryRecord {
    pub chain: SourceChain,
    pub transaction: SourceTransaction,
    pub address: ExternalAddress,
    pub kind: ExternalHistoryKind,
    pub timestamp: u64,
    pub source_asset: [u8; 32],
    pub source_amount: u128,
    pub provenance: ExternalProvenance,
}

/// Closed provenance label used by every imported record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalProvenance {
    Ethereum,
    Solana,
}

impl ExternalHistoryRecord {
    fn validate(self) -> Result<(), MigrationError> {
        self.chain.validate()?;
        self.address.validate_for(self.chain)?;
        let provenance_matches = matches!(
            (self.chain, self.provenance),
            (SourceChain::Ethereum { .. }, ExternalProvenance::Ethereum)
                | (SourceChain::Solana { .. }, ExternalProvenance::Solana)
        );
        if !provenance_matches
            || self.timestamp == 0
            || self.source_asset == [0; 32]
            || self.source_amount == 0
        {
            return Err(MigrationError::InvalidHistory);
        }
        Ok(())
    }
}

/// One independently verified, bounded history page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedHistoryPage {
    pub records: Vec<ExternalHistoryRecord>,
    pub next_cursor: Option<[u8; 32]>,
    pub evidence_digest: [u8; 32],
}

impl VerifiedHistoryPage {
    fn validate(&self, evidence: &SourceEvidence) -> Result<(), MigrationError> {
        if self.records.is_empty()
            || self.records.len() > HISTORY_PAGE_LIMIT
            || self.evidence_digest != evidence.digest()
        {
            return Err(MigrationError::InvalidHistory);
        }
        for record in &self.records {
            record.validate()?;
        }
        Ok(())
    }
}

/// Storage boundary that accepts only the external-provenance record type.
pub trait ExternalHistorySink {
    /// Persists a verified external history page under the caller principal.
    /// Implementations deduplicate records by chain, transaction, and address
    /// so overlapping source pages remain idempotent.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal without relabelling any record as protocol
    /// activity or a receipt.
    fn store_external(
        &mut self,
        principal: &PrincipalId,
        page: &VerifiedHistoryPage,
        trace: &TraceId,
    ) -> Result<(), MigrationError>;
}

/// State-changing operations routed to the plane's existing binding and
/// custody-credit authorities. The adapter never constructs payload bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationIntent {
    BindAccount(VerifiedOwnership),
    CreditCustody(VerifiedAssetFinality),
}

/// Real plane outcome. An executed operation must carry canonical receipt
/// bytes and independently obtained batch authority evidence.
#[derive(Debug)]
pub enum MigrationPlaneResult {
    Pending,
    Refused,
    Executed {
        canonical_receipt: Vec<u8>,
        authorised_batch: AuthorizedBatch,
    },
}

/// Existing protocol authority boundary for binding and custody credit.
pub trait MigrationPlane {
    /// Executes an already verified migration intent under the supplied key.
    ///
    /// # Errors
    ///
    /// Returns a typed plane refusal and never invents a receipt.
    fn execute(
        &mut self,
        intent: &MigrationIntent,
        idempotency_key: [u8; 32],
        trace: &TraceId,
    ) -> Result<MigrationPlaneResult, MigrationError>;
}

/// Protocol-owned verification for address-binding receipts. This indirection
/// is required because Ethereum and Solana use distinct binding mechanisms;
/// an adapter cannot define either mechanism or grant itself authority.
pub trait BindingReceiptPolicy {
    /// Confirms that the verified protocol receipt binds the exact external
    /// address and `LayerX` identity named by the ownership evidence.
    ///
    /// # Errors
    ///
    /// Refuses an unrelated or mismatched protocol receipt.
    fn verify_binding(
        &self,
        ownership: &VerifiedOwnership,
        receipt: &VerifiedReceipt,
    ) -> Result<(), MigrationError>;
}

/// Honest adapter result. Pending source finality is a typed error; pending
/// here means the source was verified and the protocol operation is open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationState {
    ProtocolPending,
    AccountMapped { receipt_digest: [u8; 32] },
    AssetCredited { receipt_digest: [u8; 32] },
    HistoryImported { record_count: usize },
    Refused,
}

/// Stateless migration orchestrator using the principal-scoped gateway for
/// idempotency, audit, trace, and receipt-gated terminal state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationAdapter;

impl MigrationAdapter {
    /// Verifies ownership and binds the external account through the protocol.
    ///
    /// # Errors
    ///
    /// Returns source, plane, binding-policy, gateway, or receipt refusals.
    #[allow(clippy::too_many_arguments)]
    pub fn map_account(
        gateway: &mut GatewayCore,
        principal: &PrincipalId,
        evidence: &SourceEvidence,
        verifier: &impl SourceVerifier,
        plane: &mut impl MigrationPlane,
        binding_policy: &impl BindingReceiptPolicy,
        trace: &TraceId,
        now: u64,
    ) -> Result<MigrationState, Traced<MigrationError>> {
        let fail = |error| trace.wrap(error);
        let ownership = verifier.verify_ownership(evidence, trace).map_err(fail)?;
        ownership.validate(evidence).map_err(fail)?;
        let key = ownership_key(&ownership);
        let request = translation_request(key, ownership_digest(&ownership), trace)?;
        let opened = begin(gateway, principal, &request, trace, now)?;
        match opened {
            TranslationStatus::ReceiptVerified { receipt_digest } => {
                return Ok(MigrationState::AccountMapped { receipt_digest });
            }
            TranslationStatus::Refused => return Ok(MigrationState::Refused),
            TranslationStatus::Pending => {}
            TranslationStatus::Translated => {
                return Err(fail(MigrationError::Gateway(GatewayError::Corrupt(
                    "state-changing migration has a read-only completion",
                ))));
            }
        }
        let outcome = plane
            .execute(&MigrationIntent::BindAccount(ownership), key, trace)
            .map_err(fail)?;
        settle(
            gateway,
            principal,
            key,
            outcome,
            trace,
            now,
            |receipt| binding_policy.verify_binding(&ownership, receipt),
            |receipt_digest| MigrationState::AccountMapped { receipt_digest },
        )
    }

    /// Verifies source finality and credits only through the custody boundary.
    ///
    /// # Errors
    ///
    /// Returns pending until finality is proven and refuses every evidence or
    /// receipt mismatch without crediting value.
    #[allow(clippy::too_many_arguments)]
    pub fn migrate_asset(
        gateway: &mut GatewayCore,
        principal: &PrincipalId,
        evidence: &SourceEvidence,
        verifier: &impl SourceVerifier,
        plane: &mut impl MigrationPlane,
        trace: &TraceId,
        now: u64,
    ) -> Result<MigrationState, Traced<MigrationError>> {
        let fail = |error| trace.wrap(error);
        let finality = verifier
            .verify_asset_finality(evidence, trace)
            .map_err(fail)?;
        finality.validate(evidence).map_err(fail)?;
        let key = asset_key(&finality);
        let request = translation_request(key, finality_digest(&finality), trace)?;
        let opened = begin(gateway, principal, &request, trace, now)?;
        match opened {
            TranslationStatus::ReceiptVerified { receipt_digest } => {
                return Ok(MigrationState::AssetCredited { receipt_digest });
            }
            TranslationStatus::Refused => return Ok(MigrationState::Refused),
            TranslationStatus::Pending => {}
            TranslationStatus::Translated => {
                return Err(fail(MigrationError::Gateway(GatewayError::Corrupt(
                    "state-changing migration has a read-only completion",
                ))));
            }
        }
        let outcome = plane
            .execute(&MigrationIntent::CreditCustody(finality), key, trace)
            .map_err(fail)?;
        settle(
            gateway,
            principal,
            key,
            outcome,
            trace,
            now,
            |receipt| verify_credit_receipt(&finality, receipt),
            |receipt_digest| MigrationState::AssetCredited { receipt_digest },
        )
    }

    /// Imports a verified source history page as external provenance only.
    ///
    /// # Errors
    ///
    /// Refuses invalid evidence, storage failures, and conflicting reuse of a
    /// history page identity.
    #[allow(clippy::too_many_arguments)]
    pub fn import_history(
        gateway: &mut GatewayCore,
        principal: &PrincipalId,
        evidence: &SourceEvidence,
        verifier: &impl SourceVerifier,
        sink: &mut impl ExternalHistorySink,
        trace: &TraceId,
        now: u64,
    ) -> Result<MigrationState, Traced<MigrationError>> {
        let fail = |error| trace.wrap(error);
        let page = verifier.verify_history(evidence, trace).map_err(fail)?;
        page.validate(evidence).map_err(fail)?;
        let key = history_key(&page);
        let request = TranslationRequest::new(
            adapter_id().map_err(fail)?,
            TranslationKind::ReadOnly,
            key,
            history_digest(&page),
        )
        .map_err(|error| fail(MigrationError::Gateway(error)))?;
        let opened = begin(gateway, principal, &request, trace, now)?;
        match opened {
            TranslationStatus::Translated => {
                return Ok(MigrationState::HistoryImported {
                    record_count: page.records.len(),
                });
            }
            TranslationStatus::Refused => return Ok(MigrationState::Refused),
            TranslationStatus::Pending => {}
            TranslationStatus::ReceiptVerified { .. } => {
                return Err(fail(MigrationError::Gateway(GatewayError::Corrupt(
                    "read-only history import has a settlement receipt",
                ))));
            }
        }
        sink.store_external(principal, &page, trace).map_err(fail)?;
        gateway
            .complete_read_only(principal, key, trace, now)
            .map_err(|error| trace.wrap(MigrationError::Gateway(error.into_error())))?;
        Ok(MigrationState::HistoryImported {
            record_count: page.records.len(),
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn settle(
    gateway: &mut GatewayCore,
    principal: &PrincipalId,
    key: [u8; 32],
    outcome: MigrationPlaneResult,
    trace: &TraceId,
    now: u64,
    verify_receipt: impl FnOnce(&VerifiedReceipt) -> Result<(), MigrationError>,
    completed: impl FnOnce([u8; 32]) -> MigrationState,
) -> Result<MigrationState, Traced<MigrationError>> {
    let fail = |error| trace.wrap(error);
    match outcome {
        MigrationPlaneResult::Pending => Ok(MigrationState::ProtocolPending),
        MigrationPlaneResult::Refused => {
            gateway
                .refuse_translation(principal, key, trace, now)
                .map_err(|error| trace.wrap(MigrationError::Gateway(error.into_error())))?;
            Ok(MigrationState::Refused)
        }
        MigrationPlaneResult::Executed {
            canonical_receipt,
            authorised_batch,
        } => {
            let verified = verify(&canonical_receipt, &authorised_batch)
                .map_err(|_| fail(MigrationError::ReceiptMismatch))?;
            verify_receipt(&verified).map_err(fail)?;
            let status = gateway
                .settle_with_receipt(
                    principal,
                    key,
                    &canonical_receipt,
                    &authorised_batch,
                    trace,
                    now,
                )
                .map_err(|error| trace.wrap(MigrationError::Gateway(error.into_error())))?;
            let TranslationStatus::ReceiptVerified { receipt_digest } = status else {
                return Err(fail(MigrationError::ReceiptRequired));
            };
            Ok(completed(receipt_digest))
        }
    }
}

fn begin(
    gateway: &mut GatewayCore,
    principal: &PrincipalId,
    request: &TranslationRequest,
    trace: &TraceId,
    now: u64,
) -> Result<TranslationStatus, Traced<MigrationError>> {
    gateway
        .begin_translation(principal, request, trace, now)
        .map_err(|error| trace.wrap(MigrationError::Gateway(error.into_error())))
}

fn translation_request(
    key: [u8; 32],
    digest: [u8; 32],
    trace: &TraceId,
) -> Result<TranslationRequest, Traced<MigrationError>> {
    let fail = |error| trace.wrap(error);
    TranslationRequest::new(
        adapter_id().map_err(fail)?,
        TranslationKind::StateChanging,
        key,
        digest,
    )
    .map_err(|error| fail(MigrationError::Gateway(error)))
}

fn verify_credit_receipt(
    finality: &VerifiedAssetFinality,
    receipt: &VerifiedReceipt,
) -> Result<(), MigrationError> {
    let protocol = receipt
        .receipt()
        .protocol()
        .ok_or(MigrationError::ReceiptMismatch)?;
    if protocol.asset() != finality.layerx_asset
        || protocol.amount() != finality.layerx_amount
        || protocol.to() != finality.destination
    {
        return Err(MigrationError::ReceiptMismatch);
    }
    Ok(())
}

fn adapter_id() -> Result<AdapterId, MigrationError> {
    AdapterId::new(ADAPTER_ID).map_err(|error| MigrationError::Gateway(error.into()))
}

/// Declares the migration boundary against pinned Ethereum/Solana source
/// specifications and a real conformance suite supplied by deployment.
///
/// # Errors
///
/// Returns an adapter declaration refusal if the stable identifier is invalid.
pub fn migration_adapter_descriptor(
    spec: PinnedSpec,
    conformance: ConformanceSuite,
) -> Result<AdapterDescriptor, MigrationError> {
    Ok(AdapterDescriptor::new(adapter_id()?, spec, conformance))
}

fn ownership_key(ownership: &VerifiedOwnership) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(IDEMPOTENCY_DOMAIN);
    hash.update(b"ownership\0");
    ownership.chain.commit(&mut hash);
    ownership.address.commit(&mut hash);
    hash.update(ownership.layerx_identity);
    hash.finalize().into()
}

fn asset_key(finality: &VerifiedAssetFinality) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(IDEMPOTENCY_DOMAIN);
    hash.update(b"asset\0");
    finality.chain.commit(&mut hash);
    hash.update(finality.transaction.bytes());
    hash.update(finality.custody_reference);
    hash.finalize().into()
}

fn history_key(page: &VerifiedHistoryPage) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(IDEMPOTENCY_DOMAIN);
    hash.update(b"history\0");
    hash.update(page.evidence_digest);
    hash.finalize().into()
}

fn ownership_digest(ownership: &VerifiedOwnership) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(REQUEST_DOMAIN);
    hash.update(b"ownership\0");
    ownership.chain.commit(&mut hash);
    ownership.address.commit(&mut hash);
    hash.update(ownership.layerx_identity);
    hash.update(ownership.evidence_digest);
    hash.finalize().into()
}

fn finality_digest(finality: &VerifiedAssetFinality) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(REQUEST_DOMAIN);
    hash.update(b"asset\0");
    finality.chain.commit(&mut hash);
    finality.source.commit(&mut hash);
    hash.update(finality.transaction.bytes());
    hash.update(finality.source_asset);
    hash.update(finality.source_amount.to_be_bytes());
    hash.update(finality.custody_reference);
    hash.update(finality.layerx_asset);
    hash.update(finality.layerx_amount.to_be_bytes());
    hash.update(finality.destination);
    hash.update(finality.finality_height.to_be_bytes());
    hash.update(finality.evidence_digest);
    hash.finalize().into()
}

fn history_digest(page: &VerifiedHistoryPage) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(REQUEST_DOMAIN);
    hash.update(b"history\0");
    hash.update(page.evidence_digest);
    hash.update((page.records.len() as u64).to_be_bytes());
    if let Some(cursor) = page.next_cursor {
        hash.update([1]);
        hash.update(cursor);
    } else {
        hash.update([0]);
    }
    for record in &page.records {
        record.chain.commit(&mut hash);
        hash.update(record.transaction.bytes());
        record.address.commit(&mut hash);
        hash.update([match record.kind {
            ExternalHistoryKind::Incoming => 1,
            ExternalHistoryKind::Outgoing => 2,
            ExternalHistoryKind::Contract => 3,
        }]);
        hash.update(record.timestamp.to_be_bytes());
        hash.update(record.source_asset);
        hash.update(record.source_amount.to_be_bytes());
    }
    hash.finalize().into()
}

/// Stable, redaction-safe migration refusals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationError {
    InvalidNetwork,
    InvalidAddress,
    AddressChainMismatch,
    InvalidTransaction,
    InvalidEvidence,
    EvidenceMismatch,
    SourcePending,
    SourceReverted,
    SourceDisplaced,
    InvalidHistory,
    StorageRefused,
    PlaneRefused,
    ReceiptRequired,
    ReceiptMismatch,
    Gateway(GatewayError),
}

impl Display for MigrationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidNetwork => formatter.write_str("source network is invalid"),
            Self::InvalidAddress => formatter.write_str("source address is invalid"),
            Self::AddressChainMismatch => {
                formatter.write_str("source address does not belong to the declared chain")
            }
            Self::InvalidTransaction => formatter.write_str("source transaction is invalid"),
            Self::InvalidEvidence => formatter.write_str("source evidence is invalid"),
            Self::EvidenceMismatch => formatter.write_str("source evidence facts do not match"),
            Self::SourcePending => formatter.write_str("source finality is still pending"),
            Self::SourceReverted => formatter.write_str("source transaction reverted"),
            Self::SourceDisplaced => formatter.write_str("source transaction was displaced"),
            Self::InvalidHistory => formatter.write_str("external history page is invalid"),
            Self::StorageRefused => formatter.write_str("external history storage refused"),
            Self::PlaneRefused => formatter.write_str("protocol migration operation refused"),
            Self::ReceiptRequired => formatter.write_str("a verified protocol receipt is required"),
            Self::ReceiptMismatch => {
                formatter.write_str("protocol receipt does not match the migration")
            }
            Self::Gateway(error) => write!(formatter, "gateway translation failed: {error}"),
        }
    }
}

impl std::error::Error for MigrationError {}

/// Codify anchor for Ethereum migration tooling.
#[must_use]
pub const fn interop_migrate_ethereum() -> &'static str {
    "verified-finality-ethereum-migration"
}

/// Codify anchor for Solana migration tooling.
#[must_use]
pub const fn interop_migrate_solana() -> &'static str {
    "verified-finality-solana-migration"
}
