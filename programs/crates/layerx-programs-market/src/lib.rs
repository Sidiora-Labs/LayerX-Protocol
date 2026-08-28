//! Protocol state and transition laws for the off-platform compute market.

#![deny(unsafe_code)]

pub mod attest;

pub use attest::{
    Attestation, AttestationRefusal, AttestedInput, Attester, AttesterName,
    AttesterSet, EvidenceClass, ExternalInputSource, InputCommitment,
    InputCommitmentBook, LeaseId,
};
