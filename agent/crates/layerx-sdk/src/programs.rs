//! Receipt-verified Programs operations shared by direct-node and hosted transports.

use layerx_proof::receipt::{verify_program_outcome, AuthorizedBatch, VerifiedReceipt};
use layerx_wire::receipt::{decode, ProgramOutcome};

pub const MAX_CALLDATA_BYTES: usize = 1_048_576;
pub const MAX_CAPABILITIES: usize = 256;
pub const MAX_CAPABILITY_BYTES: usize = 4_096;
pub const PROGRAMS_RECEIPT_MODULE_ID: u16 = 9;
pub const PROGRAM_CALL_ORDINAL: u8 = 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramCall {
    pub program_id: [u8; 32],
    pub version: u32,
    pub code_hash: [u8; 32],
    pub abi_version: u16,
    pub entrypoint: String,
    pub calldata: Vec<u8>,
    pub fuel: u64,
    pub fee_limit: u128,
    pub capabilities: Vec<Vec<u8>>,
    pub signed_activity: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramOperationError {
    Bounds,
    Transport,
    Decode,
    Verification,
    IdentityMismatch,
    UnknownOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramEvidence {
    pub receipt: Vec<u8>,
    pub authority: AuthorizedBatch,
    pub activity_id: [u8; 32],
    pub terminal_attachments: Vec<Vec<u8>>,
}

pub struct VerifiedProgramExecution {
    receipt: VerifiedReceipt,
    outcome: ProgramOutcome,
    terminal_attachments: Vec<Vec<u8>>,
}

impl VerifiedProgramExecution {
    #[must_use] pub const fn receipt(&self) -> &VerifiedReceipt { &self.receipt }
    #[must_use] pub const fn outcome(&self) -> &ProgramOutcome { &self.outcome }
    #[must_use] pub fn terminal_attachments(&self) -> &[Vec<u8>] { &self.terminal_attachments }
}

pub enum ProgramSubmission {
    Refused(VerifiedProgramExecution),
    Unknown { activity_id: [u8; 32], retained_signed_activity: Vec<u8> },
    Executed(VerifiedProgramExecution),
}

pub trait ProgramTransport {
    type Discovery;
    type Interface;
    fn discover(&self, program: [u8; 32]) -> Result<Self::Discovery, ProgramOperationError>;
    fn interface(&self, program: [u8; 32], version: u32) -> Result<Self::Interface, ProgramOperationError>;
    fn simulate(&self, call: &ProgramCall) -> Result<ProgramEvidence, ProgramOperationError>;
    fn submit(&self, call: &ProgramCall, idempotency_key: [u8; 32]) -> Result<ProgramSubmission, ProgramOperationError>;
    fn receipt(&self, idempotency_key: [u8; 32], expected_activity: [u8; 32]) -> Result<ProgramSubmission, ProgramOperationError>;
    fn activity(&self, activity_id: [u8; 32]) -> Result<ProgramSubmission, ProgramOperationError>;
}

pub fn verify_program_evidence(evidence: ProgramEvidence, call: &ProgramCall) -> Result<VerifiedProgramExecution, ProgramOperationError> {
    validate_call(call)?;
    let verified = verify_program_outcome(&evidence.receipt, &evidence.authority).map_err(|_| ProgramOperationError::Verification)?;
    let receipt = decode(&evidence.receipt).map_err(|_| ProgramOperationError::Decode)?;
    let protocol = receipt.protocol().ok_or(ProgramOperationError::Verification)?;
    if protocol.activity_id() != evidence.activity_id || protocol.module_id() != PROGRAMS_RECEIPT_MODULE_ID || protocol.operation() != PROGRAM_CALL_ORDINAL || protocol.module_version() != u32::from(call.abi_version) {
        return Err(ProgramOperationError::IdentityMismatch);
    }
    let outcome = protocol.program_outcome().cloned().ok_or(ProgramOperationError::Verification)?;
    if outcome.abi_version() != call.abi_version || outcome.result_code() != protocol.result_code() {
        return Err(ProgramOperationError::IdentityMismatch);
    }
    Ok(VerifiedProgramExecution { receipt: verified, outcome, terminal_attachments: evidence.terminal_attachments })
}

fn validate_call(call: &ProgramCall) -> Result<(), ProgramOperationError> {
    if call.version == 0 || call.abi_version == 0 || call.entrypoint.is_empty() || call.entrypoint.len() > 255 || call.calldata.len() > MAX_CALLDATA_BYTES || call.capabilities.len() > MAX_CAPABILITIES || call.capabilities.iter().any(|item| item.is_empty() || item.len() > MAX_CAPABILITY_BYTES) || call.signed_activity.is_empty() {
        return Err(ProgramOperationError::Bounds);
    }
    Ok(())
}

#[must_use]
pub const fn platform_sdk_programs() -> &'static str { "receipt-verified-program-operations-v1" }
