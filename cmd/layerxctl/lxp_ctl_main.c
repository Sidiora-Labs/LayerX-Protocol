#include "layerx/lxp_tools.h"

#include <string.h>

static void put_u64(uint8_t out[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i)
        out[7U - i] = (uint8_t)(value >> (i * 8U));
}

static void result_output(
    uint8_t operation, lxp_result result, uint64_t sequence,
    const uint8_t root[32], uint8_t output[LXP_CTL_OUTPUT_BYTES])
{
    uint32_t encoded_result = (uint32_t)(int32_t)result;
    (void)memcpy(output, "LXCT", 4U);
    output[4] = 1U;
    output[5] = operation;
    output[6] = (uint8_t)(encoded_result >> 24U);
    output[7] = (uint8_t)(encoded_result >> 16U);
    output[8] = (uint8_t)(encoded_result >> 8U);
    output[9] = (uint8_t)encoded_result;
    put_u64(output + 10U, sequence);
    (void)memcpy(output + 18U, root, 32U);
}

lxp_result lxp_ctl_submit_activity(
    const lxp_ctl_context *context,
    const uint8_t *activity, size_t activity_length,
    uint8_t output[LXP_CTL_OUTPUT_BYTES])
{
    uint8_t root[32] = {0U};
    uint64_t sequence = 0U;
    lxp_result status;
    if (context == NULL || context->ordered_submit == NULL ||
        activity == NULL || activity_length == 0U || output == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = context->ordered_submit(
        context->context, activity, activity_length, &sequence, root);
    result_output((uint8_t)LXP_CTL_SUBMIT, status, sequence, root, output);
    return status;
}

lxp_result lxp_ctl_main(
    lxp_ctl_command command, const lxp_ctl_context *context,
    const uint8_t *input, size_t input_length,
    uint8_t output[LXP_CTL_OUTPUT_BYTES])
{
    uint8_t root[32] = {0U};
    uint64_t sequence = 0U;
    lxp_result status;
    if (context == NULL || output == NULL) return LXP_ERR_NON_CANONICAL;
    if (command == LXP_CTL_SUBMIT)
        return lxp_ctl_submit_activity(
            context, input, input_length, output);
    if (command != LXP_CTL_READ_STATE || input != NULL ||
        input_length != 0U || context->read_state == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = context->read_state(context->context, &sequence, root);
    result_output((uint8_t)LXP_CTL_READ_STATE,
                  status, sequence, root, output);
    return status;
}
