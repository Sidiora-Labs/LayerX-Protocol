//! Concrete `wasmi` bindings for the version-one capability ABI.

mod calls;
mod events;
mod memory;
mod storage;
mod transfer;

use wasmi::{Caller, Engine, Linker};

use crate::abi::{Abi, AbiError, ReceiptView, ABI_MODULE};
use crate::calls::{Composition, CompositionRefusal};
use crate::execute::ExecutionFault;
use crate::meter::Meter;

use self::memory::{nonnegative, read_fixed, write_guest};

pub(super) const STATUS_DENIED: i32 = -1;
pub(super) const STATUS_INVALID: i32 = -2;
pub(super) const STATUS_BOUNDS: i32 = -3;
pub(super) const STATUS_METER: i32 = -4;
pub(super) const STATUS_EVIDENCE: i32 = -5;
pub(super) const FUEL_METERING_DISABLED: &str = "programs runtime fuel metering is disabled";
pub(super) const COMPOSITION_REFUSED: &str = "program composition refused the call graph";

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

    pub(crate) fn authorization_abi(&self) -> Option<&Abi> {
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

    pub(super) fn with_abi<T>(
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
    storage::register(&mut linker)?;
    events::register(&mut linker)?;
    calls::register(&mut linker)?;
    transfer::register(&mut linker)?;
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

fn encode_receipt(view: &ReceiptView) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(116);
    encoded.extend_from_slice(&view.receipt_digest);
    encoded.extend_from_slice(&view.result_code.to_be_bytes());
    encoded.extend_from_slice(&view.asset);
    encoded.extend_from_slice(&view.amount.to_be_bytes());
    encoded.extend_from_slice(&view.state_root);
    encoded
}

pub(super) const fn error_status(error: AbiError) -> i32 {
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

pub(super) fn linker_fault(error: &wasmi::errors::LinkerError) -> ExecutionFault {
    ExecutionFault::EngineFault {
        reason: error.to_string(),
    }
}
