//! Observability output is constrained by the same tenant redaction seam as audit.

use crate::audit::{self, DataClass, OutputSurface, RedactionError, RenderedOutput};
use crate::store::TenantId;
use crate::tenant::Config;

pub fn redact(
    config: &Config,
    tenant: &TenantId,
    surface: OutputSurface,
    class: DataClass,
    value: &[u8],
    current_sequence: u64,
) -> Result<RenderedOutput, RedactionError> {
    audit::redact(config, tenant, surface, class, value, current_sequence)
}
