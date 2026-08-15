//! Minimal offline-verifier entry point.

use layerx_proof::export::{verify, OfflineExport};

fn verify_without_network(
    artifact: &OfflineExport,
) -> Result<layerx_proof::export::VerificationReport, layerx_proof::export::ExportVerificationError>
{
    verify(artifact)
}

fn main() {
    let verifier: fn(
        &OfflineExport,
    ) -> Result<
        layerx_proof::export::VerificationReport,
        layerx_proof::export::ExportVerificationError,
    > = verify_without_network;
    let _ = verifier;
}
