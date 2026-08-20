#include "layerx/program.h"

#include "host.h"
#include "internal.h"

/*
 * A call carries at most the caller's own authority. The encoded capability
 * list is built by lxp_program_capability_set_encode, and the runtime refuses
 * any grant the caller does not itself hold and any raised transfer limit.
 */

lxp_program_status lxp_program_call(lxp_program_id callee,
                                    const uint8_t *input, size_t input_length,
                                    const uint8_t *capabilities,
                                    size_t capabilities_length)
{
    if (lxp_program_bytes32_is_zero(callee.bytes))
        return LXP_PROGRAM_ERR_RESERVED_IDENTIFIER;
    if (input_length > 0U && input == NULL)
        return LXP_PROGRAM_ERR_NULL_ARGUMENT;
    if (input_length > (size_t)LXP_PROGRAM_MAX_CALL_INPUT_BYTES)
        return LXP_PROGRAM_ERR_INPUT_TOO_LARGE;
    if (capabilities == NULL) return LXP_PROGRAM_ERR_NULL_ARGUMENT;
    if (capabilities_length < 2U) return LXP_PROGRAM_ERR_INVALID;
    if (capabilities_length > (size_t)LXP_PROGRAM_MAX_CAPABILITY_BYTES)
        return LXP_PROGRAM_ERR_CAPABILITY_BYTES;
    return lxp_program_host_program_call(
        lxp_program_pointer(callee.bytes),
        lxp_program_length((size_t)LXP_PROGRAM_ID_BYTES),
        lxp_program_pointer(input), lxp_program_length(input_length),
        lxp_program_pointer(capabilities),
        lxp_program_length(capabilities_length));
}
