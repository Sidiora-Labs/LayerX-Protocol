//! Execution-context host-function registration.
//!
//! The single `context_read` host function answers one field-addressed query
//! per call. Every field is derived from host-fixed protocol state: the
//! executing program and its immediate caller from the call graph, the
//! principal from the fixed authority, the remaining fuel from the meter, and
//! the ambient sequence, height and versions from the composition context.
//! Guest code has no channel to supply an identity, so the caller field cannot
//! be forged; an unknown field identifier is refused rather than answered with
//! a zero; and every returned byte is metered.

use wasmi::{Caller, Linker};

use crate::abi::context::ContextField;
use crate::abi::response::CANDIDATE_ABI_MODULE;
use crate::execute::ExecutionFault;

use super::memory::{nonnegative, write_guest};
use super::{linker_fault, RuntimeState, STATUS_BOUNDS, STATUS_DENIED, STATUS_INVALID, STATUS_METER};

pub(super) fn register_candidate(linker: &mut Linker<RuntimeState>) -> Result<(), ExecutionFault> {
    linker
        .func_wrap(
            CANDIDATE_ABI_MODULE,
            "context_read",
            |mut caller: Caller<'_, RuntimeState>,
             field_id: i32,
             output_pointer: i32,
             output_capacity: i32|
             -> i32 {
                let Ok(raw) = u32::try_from(field_id) else {
                    return STATUS_INVALID;
                };
                let Some(field) = ContextField::from_id(raw) else {
                    return STATUS_INVALID;
                };
                let Some(consumed) = caller.fuel_consumed() else {
                    return STATUS_METER;
                };
                let remaining = {
                    let meter = caller.data().meter();
                    meter
                        .cpu_budget()
                        .saturating_sub(meter.cpu_carried().saturating_add(consumed))
                };
                let Some(bytes) = caller.data().context_field_bytes(field, remaining) else {
                    return STATUS_DENIED;
                };
                let capacity = match nonnegative(output_capacity) {
                    Ok(capacity) => capacity,
                    Err(status) => return status,
                };
                if bytes.len() > capacity {
                    return STATUS_BOUNDS;
                }
                let metered = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
                if caller
                    .data_mut()
                    .meter_mut()
                    .charge_storage_read(metered)
                    .is_err()
                {
                    return STATUS_METER;
                }
                if let Err(status) = write_guest(&mut caller, output_pointer, &bytes) {
                    return status;
                }
                i32::try_from(bytes.len()).unwrap_or(STATUS_BOUNDS)
            },
        )
        .map_err(|error| linker_fault(&error))?;
    Ok(())
}
