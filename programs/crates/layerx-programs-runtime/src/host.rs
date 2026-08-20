//! Concrete `wasmi` bindings for the version-one capability ABI.

use wasmi::core::{Trap, TrapCode};
use wasmi::{Caller, Engine, Linker, Memory};

use crate::abi::{Abi, AbiError, CapabilitySet, ReceiptView, ABI_MODULE};
use crate::calls::{self, call_admission_fuel, Composition, CompositionRefusal};
use crate::execute::ExecutionFault;
use crate::meter::Meter;
use crate::storage::ProgramId;

const STATUS_DENIED: i32 = -1;
const STATUS_INVALID: i32 = -2;
const STATUS_BOUNDS: i32 = -3;
const STATUS_METER: i32 = -4;
const STATUS_EVIDENCE: i32 = -5;
const FUEL_METERING_DISABLED: &str = "programs runtime fuel metering is disabled";
const COMPOSITION_REFUSED: &str = "program composition refused the call graph";

#[derive(Debug)]
pub(crate) struct RuntimeState {
    meter: Meter,
    abi: Option<Abi>,
    composition: Option<Composition>,
    refusal: Option<CompositionRefusal>,
}

impl RuntimeState {
    pub(crate) const fn isolated(meter: Meter) -> Self {
        Self {
            meter,
            abi: None,
            composition: None,
            refusal: None,
        }
    }

    pub(crate) fn composed(meter: Meter, abi: Abi, composition: Composition) -> Self {
        Self {
            meter,
            abi: Some(abi),
            composition: Some(composition),
            refusal: None,
        }
    }

    pub(crate) const fn meter(&self) -> &Meter {
        &self.meter
    }

    pub(crate) fn meter_mut(&mut self) -> &mut Meter {
        &mut self.meter
    }

    pub(crate) fn set_meter(&mut self, meter: Meter) {
        self.meter = meter;
    }

    pub(crate) fn abi(&self) -> Option<&Abi> {
        self.abi.as_ref()
    }

    pub(crate) fn abi_mut(&mut self) -> Option<&mut Abi> {
        self.abi.as_mut()
    }

    pub(crate) fn composition(&self) -> Option<&Composition> {
        self.composition.as_ref()
    }

    pub(crate) fn composition_mut(&mut self) -> Option<&mut Composition> {
        self.composition.as_mut()
    }

    pub(crate) fn record_refusal(&mut self, refusal: CompositionRefusal) {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
    }

    pub(crate) fn refusal(&self) -> Option<&CompositionRefusal> {
        self.refusal.as_ref()
    }

    pub(crate) fn into_parts(self) -> (Meter, Option<Abi>, Option<Composition>) {
        (self.meter, self.abi, self.composition)
    }

    fn with_abi<T>(
        &mut self,
        operation: impl FnOnce(&mut Abi, &mut Meter) -> Result<T, AbiError>,
    ) -> Result<T, AbiError> {
        let abi = self.abi.as_mut().ok_or(AbiError::CapabilityDenied)?;
        operation(abi, &mut self.meter)
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn linker(engine: &Engine) -> Result<Linker<RuntimeState>, ExecutionFault> {
    let mut linker = Linker::new(engine);
    linker
        .func_wrap(
            ABI_MODULE,
            "storage_read",
            |mut caller: Caller<'_, RuntimeState>,
             key_pointer: i32,
             key_length: i32,
             output_pointer: i32,
             output_capacity: i32|
             -> i32 {
                let key = match read_guest(&caller, key_pointer, key_length, 256) {
                    Ok(key) => key,
                    Err(status) => return status,
                };
                let value = match caller
                    .data_mut()
                    .with_abi(|abi, meter| abi.storage_read(meter, &key))
                {
                    Ok(value) => value,
                    Err(error) => return error_status(error),
                };
                let Some(value) = value else {
                    return 0;
                };
                let capacity = match nonnegative(output_capacity) {
                    Ok(capacity) => capacity,
                    Err(status) => return status,
                };
                if value.len() > capacity {
                    return STATUS_BOUNDS;
                }
                if let Err(status) = write_guest(&mut caller, output_pointer, &value) {
                    return status;
                }
                i32::try_from(value.len())
                    .ok()
                    .and_then(|length| length.checked_add(1))
                    .unwrap_or(STATUS_BOUNDS)
            },
        )
        .map_err(|error| linker_fault(&error))?;
    linker
        .func_wrap(
            ABI_MODULE,
            "storage_write",
            |mut caller: Caller<'_, RuntimeState>,
             key_pointer: i32,
             key_length: i32,
             value_pointer: i32,
             value_length: i32|
             -> i32 {
                let key = match read_guest(&caller, key_pointer, key_length, 256) {
                    Ok(key) => key,
                    Err(status) => return status,
                };
                let value = match read_guest(&caller, value_pointer, value_length, 1_048_576) {
                    Ok(value) => value,
                    Err(status) => return status,
                };
                match caller
                    .data_mut()
                    .with_abi(|abi, meter| abi.storage_write(meter, &key, &value))
                {
                    Ok(()) => 0,
                    Err(error) => error_status(error),
                }
            },
        )
        .map_err(|error| linker_fault(&error))?;
    linker
        .func_wrap(
            ABI_MODULE,
            "storage_delete",
            |mut caller: Caller<'_, RuntimeState>, key_pointer: i32, key_length: i32| -> i32 {
                let key = match read_guest(&caller, key_pointer, key_length, 256) {
                    Ok(key) => key,
                    Err(status) => return status,
                };
                match caller
                    .data_mut()
                    .with_abi(|abi, meter| abi.storage_delete(meter, &key))
                {
                    Ok(()) => 0,
                    Err(error) => error_status(error),
                }
            },
        )
        .map_err(|error| linker_fault(&error))?;
    linker
        .func_wrap(
            ABI_MODULE,
            "event_emit",
            |mut caller: Caller<'_, RuntimeState>,
             topic_pointer: i32,
             topic_length: i32,
             data_pointer: i32,
             data_length: i32|
             -> i32 {
                let topic = match read_guest(&caller, topic_pointer, topic_length, 64) {
                    Ok(topic) => topic,
                    Err(status) => return status,
                };
                let data = match read_guest(&caller, data_pointer, data_length, 65_536) {
                    Ok(data) => data,
                    Err(status) => return status,
                };
                match caller
                    .data_mut()
                    .with_abi(|abi, _| abi.emit_event(&topic, &data))
                {
                    Ok(()) => 0,
                    Err(error) => error_status(error),
                }
            },
        )
        .map_err(|error| linker_fault(&error))?;
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
                if caller.data().abi().is_none() || caller.data().composition().is_none() {
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
                let outcome = calls::execute_nested_call(
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
    linker
        .func_wrap(
            ABI_MODULE,
            "transfer_402",
            |mut caller: Caller<'_, RuntimeState>,
             amount_high: i64,
             amount_low: i64,
             asset_pointer: i32,
             asset_length: i32,
             recipient_pointer: i32,
             recipient_length: i32|
             -> i32 {
                let asset = match read_fixed::<32>(&caller, asset_pointer, asset_length) {
                    Ok(asset) => asset,
                    Err(status) => return status,
                };
                let recipient = match read_fixed::<32>(&caller, recipient_pointer, recipient_length)
                {
                    Ok(recipient) => recipient,
                    Err(status) => return status,
                };
                let high = u64::from_be_bytes(amount_high.to_be_bytes());
                let low = u64::from_be_bytes(amount_low.to_be_bytes());
                let amount = u128::from(high) << 64 | u128::from(low);
                match caller
                    .data_mut()
                    .with_abi(|abi, _| abi.request_transfer(asset, recipient, amount))
                {
                    Ok(()) => 0,
                    Err(error) => error_status(error),
                }
            },
        )
        .map_err(|error| linker_fault(&error))?;
    linker
        .func_wrap(
            ABI_MODULE,
            "receipt_read",
            |mut caller: Caller<'_, RuntimeState>,
             digest_pointer: i32,
             digest_length: i32,
             output_pointer: i32,
             output_capacity: i32|
             -> i32 {
                let digest = match read_fixed::<32>(&caller, digest_pointer, digest_length) {
                    Ok(digest) => digest,
                    Err(status) => return status,
                };
                let view = match caller
                    .data_mut()
                    .with_abi(|abi, _| abi.receipt_read(digest))
                {
                    Ok(view) => view,
                    Err(error) => return error_status(error),
                };
                let encoded = encode_receipt(&view);
                let capacity = match nonnegative(output_capacity) {
                    Ok(capacity) => capacity,
                    Err(status) => return status,
                };
                if encoded.len() > capacity {
                    return STATUS_BOUNDS;
                }
                if let Err(status) = write_guest(&mut caller, output_pointer, &encoded) {
                    return status;
                }
                i32::try_from(encoded.len()).unwrap_or(STATUS_BOUNDS)
            },
        )
        .map_err(|error| linker_fault(&error))?;
    Ok(linker)
}

fn memory(caller: &Caller<'_, RuntimeState>) -> Result<Memory, i32> {
    caller
        .get_export("memory")
        .and_then(wasmi::Extern::into_memory)
        .ok_or(STATUS_INVALID)
}

fn read_guest(
    caller: &Caller<'_, RuntimeState>,
    pointer: i32,
    length: i32,
    maximum: usize,
) -> Result<Vec<u8>, i32> {
    let pointer = nonnegative(pointer)?;
    let length = nonnegative(length)?;
    if length > maximum {
        return Err(STATUS_BOUNDS);
    }
    let mut bytes = vec![0u8; length];
    memory(caller)?
        .read(caller, pointer, &mut bytes)
        .map_err(|_| STATUS_BOUNDS)?;
    Ok(bytes)
}

fn read_fixed<const N: usize>(
    caller: &Caller<'_, RuntimeState>,
    pointer: i32,
    length: i32,
) -> Result<[u8; N], i32> {
    if length != i32::try_from(N).map_err(|_| STATUS_BOUNDS)? {
        return Err(STATUS_INVALID);
    }
    let bytes = read_guest(caller, pointer, length, N)?;
    let mut output = [0u8; N];
    output.copy_from_slice(&bytes);
    Ok(output)
}

fn write_guest(
    caller: &mut Caller<'_, RuntimeState>,
    pointer: i32,
    bytes: &[u8],
) -> Result<(), i32> {
    let pointer = nonnegative(pointer)?;
    memory(caller)?
        .write(caller, pointer, bytes)
        .map_err(|_| STATUS_BOUNDS)
}

fn nonnegative(value: i32) -> Result<usize, i32> {
    usize::try_from(value).map_err(|_| STATUS_INVALID)
}

fn encode_receipt(view: &ReceiptView) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(116);
    encoded.extend_from_slice(&view.receipt_digest);
    encoded.extend_from_slice(&view.result_code.to_be_bytes());
    encoded.extend_from_slice(&view.asset);
    encoded.extend_from_slice(&view.amount.to_be_bytes());
    encoded.extend_from_slice(&view.state_root);
    encoded
}

const fn error_status(error: AbiError) -> i32 {
    match error {
        AbiError::CapabilityDenied | AbiError::CapabilityEscalation => STATUS_DENIED,
        AbiError::Meter(_) => STATUS_METER,
        AbiError::ReceiptMismatch => STATUS_EVIDENCE,
        AbiError::EventBounds
        | AbiError::CallBounds
        | AbiError::AmountBounds
        | AbiError::Storage(_) => STATUS_BOUNDS,
        AbiError::WrongVersion
        | AbiError::InvalidCapability
        | AbiError::DuplicateCapability
        | AbiError::InvalidEncoding => STATUS_INVALID,
    }
}

fn linker_fault(error: &wasmi::errors::LinkerError) -> ExecutionFault {
    ExecutionFault::EngineFault {
        reason: error.to_string(),
    }
}
