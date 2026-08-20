//! Typed MCP write outcomes over the ordinary daemon write path.

use layerx_agent_api::prepare::CanonicalBytes;
use layerx_agent_api::track::{ReceiptRef, SubmissionRef, SubmissionState, TrackedSubmission};
use layerx_agent_api::verify::Level;
use layerx_types::result::ResultCode;

use crate::server::{InvocationOutcome, Server, ServerError};

/// Mandatory client stages. No MCP-only write stage exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteStage {
    Prepare,
    Disclose,
    Policy,
    Sign,
    Submit,
    Track,
}

pub const ORDINARY_WRITE_STAGES: [WriteStage; 6] = [
    WriteStage::Prepare,
    WriteStage::Disclose,
    WriteStage::Policy,
    WriteStage::Sign,
    WriteStage::Submit,
    WriteStage::Track,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureClass {
    Refused,
    Unavailable,
    InvalidEvidence,
    Protocol,
}

/// Machine-readable failure. There is no success-like prose field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageFailure {
    pub stage: WriteStage,
    pub class: FailureClass,
    pub protocol_result_code: Option<ResultCode>,
}

/// Receipt evidence required before an executed outcome can exist.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedReceipt {
    pub receipt_ref: ReceiptRef,
    pub canonical_receipt: CanonicalBytes,
    pub verification_level: Level,
    pub evidence_ids: Vec<[u8; 32]>,
}

/// Complete daemon transcript consumed by an MCP write tool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteTranscript {
    pub stages: Vec<WriteStage>,
    pub submission: Result<TrackedSubmission, StageFailure>,
    pub receipt: Option<VerifiedReceipt>,
}

/// The only non-error write outcomes exposed to a model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WriteOutcome {
    Executed {
        submission_ref: SubmissionRef,
        receipt: VerifiedReceipt,
    },
    Unknown {
        submission_ref: SubmissionRef,
        age_ms: u64,
    },
    Pending {
        submission_ref: SubmissionRef,
        state: SubmissionState,
    },
}

#[derive(Debug)]
pub enum WriteToolError {
    Server(ServerError),
    Stage(StageFailure),
    InvalidTranscript,
    SuccessWithoutVerifiedReceipt,
    ReceiptMismatch,
}

/// Executes a new write invocation through the scoped server's daemon-only route.
///
/// # Errors
///
/// Returns typed stage, transcript, evidence, or daemon-routing failures.
pub fn execute(
    server: &mut Server,
    validated_arguments: Vec<u8>,
    transcript: WriteTranscript,
    unknown_age_ms: u64,
) -> Result<WriteOutcome, WriteToolError> {
    let invocation = server
        .route("activity.submit", validated_arguments)
        .map_err(WriteToolError::Server)?;
    if !transcript_matches(&transcript.stages, &ORDINARY_WRITE_STAGES) {
        server
            .complete(&invocation, InvocationOutcome::Failed)
            .map_err(WriteToolError::Server)?;
        return Err(WriteToolError::InvalidTranscript);
    }
    finish(server, &invocation, transcript, unknown_age_ms)
}

/// Resolves a prior honest non-terminal result through the same daemon tracking path.
///
/// # Errors
///
/// Returns typed evidence or daemon-routing failures and never manufactures completion.
pub fn track(
    server: &mut Server,
    validated_arguments: Vec<u8>,
    mut transcript: WriteTranscript,
    unknown_age_ms: u64,
) -> Result<WriteOutcome, WriteToolError> {
    let invocation = server
        .route("activity.track", validated_arguments)
        .map_err(WriteToolError::Server)?;
    if transcript.stages != [WriteStage::Track] {
        server
            .complete(&invocation, InvocationOutcome::Failed)
            .map_err(WriteToolError::Server)?;
        return Err(WriteToolError::InvalidTranscript);
    }
    transcript.stages = ORDINARY_WRITE_STAGES.to_vec();
    finish(server, &invocation, transcript, unknown_age_ms)
}

fn finish(
    server: &mut Server,
    invocation: &crate::server::DaemonInvocation,
    transcript: WriteTranscript,
    unknown_age_ms: u64,
) -> Result<WriteOutcome, WriteToolError> {
    let submission = match transcript.submission {
        Ok(submission) => submission,
        Err(failure) => {
            let outcome = if failure.class == FailureClass::Refused {
                InvocationOutcome::Refused
            } else {
                InvocationOutcome::Failed
            };
            server
                .complete(invocation, outcome)
                .map_err(WriteToolError::Server)?;
            return Err(WriteToolError::Stage(failure));
        }
    };
    let result = classify_submission(submission, transcript.receipt, unknown_age_ms);
    let outcome = match &result {
        Ok(WriteOutcome::Executed { .. }) => InvocationOutcome::Completed,
        Ok(WriteOutcome::Unknown { .. } | WriteOutcome::Pending { .. }) => {
            InvocationOutcome::Unknown
        }
        Err(WriteToolError::Stage(failure)) if failure.class == FailureClass::Protocol => {
            InvocationOutcome::Refused
        }
        Err(_) => InvocationOutcome::Failed,
    };
    server
        .complete(invocation, outcome)
        .map_err(WriteToolError::Server)?;
    result
}

fn classify_submission(
    submission: TrackedSubmission,
    receipt: Option<VerifiedReceipt>,
    unknown_age_ms: u64,
) -> Result<WriteOutcome, WriteToolError> {
    let submission_ref = submission.submission_ref.clone();
    match submission.state {
        SubmissionState::Executed { receipt_ref } => {
            let receipt = receipt.ok_or(WriteToolError::SuccessWithoutVerifiedReceipt)?;
            if receipt.receipt_ref != receipt_ref {
                return Err(WriteToolError::ReceiptMismatch);
            }
            if receipt.verification_level == Level::Unverified
                || submission.verification_level == Level::Unverified
                || receipt.verification_level != submission.verification_level
                || receipt.evidence_ids.is_empty()
            {
                return Err(WriteToolError::SuccessWithoutVerifiedReceipt);
            }
            Ok(WriteOutcome::Executed {
                submission_ref,
                receipt,
            })
        }
        SubmissionState::Failed { result } => Err(WriteToolError::Stage(StageFailure {
            stage: WriteStage::Track,
            class: FailureClass::Protocol,
            protocol_result_code: Some(result),
        })),
        SubmissionState::Unknown => Ok(WriteOutcome::Unknown {
            submission_ref,
            age_ms: unknown_age_ms,
        }),
        state => {
            if receipt.is_some() {
                return Err(WriteToolError::InvalidTranscript);
            }
            Ok(WriteOutcome::Pending {
                submission_ref,
                state,
            })
        }
    }
}

fn transcript_matches(actual: &[WriteStage], required: &[WriteStage]) -> bool {
    if actual == required {
        return true;
    }
    let Some(WriteStage::Policy) = actual.last() else {
        return false;
    };
    actual == &required[..3]
}
