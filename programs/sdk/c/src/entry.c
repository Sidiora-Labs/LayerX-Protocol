#include "layerx/program.h"

#include "internal.h"

/*
 * The single call-input region a program owns. Linear memory is zero
 * initialised, the region is never resized, and the reservation hands out the
 * same address every time, so a caller cannot steer a program at an offset of
 * its own choosing.
 */
static uint8_t call_input_region[LXP_PROGRAM_CALL_INPUT_CAPACITY];

int32_t lxp_program_reserve_call_input(int32_t length)
{
    if (length < 0) return LXP_PROGRAM_RESERVATION_REFUSED;
    if ((uint32_t)length > (uint32_t)LXP_PROGRAM_CALL_INPUT_CAPACITY)
        return LXP_PROGRAM_RESERVATION_REFUSED;
    return lxp_program_pointer(call_input_region);
}

lxp_program_status lxp_program_call_input(int32_t pointer, int32_t length,
                                          const uint8_t **out,
                                          size_t *out_length)
{
    if (out == NULL || out_length == NULL) return LXP_PROGRAM_ERR_NULL_ARGUMENT;
    if (length < 0) return LXP_PROGRAM_ERR_INVALID;
    if ((uint32_t)length > (uint32_t)LXP_PROGRAM_CALL_INPUT_CAPACITY)
        return LXP_PROGRAM_ERR_INPUT_TOO_LARGE;
    if (length > 0 && pointer != lxp_program_pointer(call_input_region))
        return LXP_PROGRAM_ERR_INVALID;
    *out = call_input_region;
    *out_length = (size_t)(uint32_t)length;
    return LXP_PROGRAM_OK;
}
