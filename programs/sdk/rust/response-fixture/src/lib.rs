#![no_std]

use layerx_program_sdk::{
    call, trap_on_panic, CallInput, CallResult, CapabilitySet, EntryResponse, Field, ProgramError,
    ProgramId, Reason,
};

trap_on_panic!();

fn echo(input: &[u8]) -> Result<EntryResponse<'_>, ProgramError> {
    if input.first() == Some(&0x7f) {
        return Ok(EntryResponse::new(CallResult::OK, input));
    }
    let callee = ProgramId::new([7; 32])?;
    let capabilities = CapabilitySet::<0>::empty();
    let mut capability_scratch = [0u8; 2];
    let mut nested_output = [0u8; 32];
    let nested_input = [0x7f, 0, 0xff];
    let response = call::invoke_response_with(
        callee,
        CallInput::new(&nested_input)?,
        &capabilities,
        &mut capability_scratch,
        &mut nested_output,
    )?;
    if response.code() != CallResult::OK.code() || response.bytes() != nested_input {
        return Err(ProgramError::value(Field::Buffer, Reason::Malformed));
    }
    Ok(EntryResponse::new(CallResult::OK, input))
}

layerx_program_sdk::response_entrypoint!(echo);
