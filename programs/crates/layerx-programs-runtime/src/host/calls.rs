//! Program-call host-function registration.

use wasmi::core::{Trap, TrapCode};
use wasmi::{Caller, Linker};

use crate::abi::CapabilitySet;
use crate::calls::{self as runtime_calls, call_admission_fuel};
use crate::execute::ExecutionFault;
use crate::storage::ProgramId;

use super::memory::{read_fixed, read_guest};
use super::{
    error_status, linker_fault, RuntimeState, ABI_MODULE, COMPOSITION_REFUSED,
    FUEL_METERING_DISABLED, STATUS_DENIED, STATUS_INVALID,
};

pub(super) fn register(linker: &mut Linker<RuntimeState>) -> Result<(), ExecutionFault> {
    linker
        .func_wrap(
            ABI_MODULE,
            "program_call",
            |mut caller: Caller<'_, RuntimeState>,
             program_pointer: i32,
             program_length: i32,
             input_pointer: i32,
             input_length: i32,
             capabilities_pointer: i32,
             capabilities_length: i32|
             -> Result<i32, Trap> {
                let program = match read_fixed::<32>(&caller, program_pointer, program_length) {
                    Ok(program) => program,
                    Err(status) => return Ok(status),
                };
                let Ok(program) = ProgramId::new(program) else {
                    return Ok(STATUS_INVALID);
                };
                let input = match read_guest(&caller, input_pointer, input_length, 1_048_576) {
                    Ok(input) => input,
                    Err(status) => return Ok(status),
                };
                let encoded =
                    match read_guest(&caller, capabilities_pointer, capabilities_length, 16_384) {
                        Ok(encoded) => encoded,
                        Err(status) => return Ok(status),
                    };
                let capabilities = match CapabilitySet::decode_canonical(&encoded) {
                    Ok(capabilities) => capabilities,
                    Err(error) => return Ok(error_status(error)),
                };
                if caller.data().authorization_abi().is_none()
                    || caller.data().composition().is_none()
                {
                    return Ok(STATUS_DENIED);
                }
                if caller
                    .consume_fuel(call_admission_fuel(input.len()))
                    .is_err()
                {
                    caller.data_mut().meter_mut().mark_cpu_exhausted();
                    return Err(Trap::from(TrapCode::OutOfFuel));
                }
                let Some(consumed) = caller.fuel_consumed() else {
                    return Err(Trap::new(FUEL_METERING_DISABLED));
                };
                let outcome = runtime_calls::execute_nested_call(
                    caller.data_mut(),
                    consumed,
                    program,
                    &input,
                    capabilities,
                );
                match outcome {
                    Ok(outcome) => {
                        if caller.consume_fuel(outcome.subtree_fuel).is_err() {
                            caller.data_mut().meter_mut().mark_cpu_exhausted();
                            return Err(Trap::from(TrapCode::OutOfFuel));
                        }
                        Ok(outcome.code)
                    }
                    Err(refusal) => {
                        caller.data_mut().record_refusal(refusal);
                        Err(Trap::new(COMPOSITION_REFUSED))
                    }
                }
            },
        )
        .map_err(|error| linker_fault(&error))?;
    Ok(())
}
