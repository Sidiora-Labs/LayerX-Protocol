//! Guest linear-memory access with stable bounds/status mapping.

use wasmi::{Caller, Memory};

use super::{RuntimeState, STATUS_BOUNDS, STATUS_INVALID};

pub(super) fn memory(caller: &Caller<'_, RuntimeState>) -> Result<Memory, i32> {
    caller
        .get_export("memory")
        .and_then(wasmi::Extern::into_memory)
        .ok_or(STATUS_INVALID)
}

pub(super) fn read_guest(
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

pub(super) fn read_fixed<const N: usize>(
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

pub(super) fn write_guest(
    caller: &mut Caller<'_, RuntimeState>,
    pointer: i32,
    bytes: &[u8],
) -> Result<(), i32> {
    let pointer = nonnegative(pointer)?;
    memory(caller)?
        .write(caller, pointer, bytes)
        .map_err(|_| STATUS_BOUNDS)
}

pub(super) fn nonnegative(value: i32) -> Result<usize, i32> {
    usize::try_from(value).map_err(|_| STATUS_INVALID)
}
