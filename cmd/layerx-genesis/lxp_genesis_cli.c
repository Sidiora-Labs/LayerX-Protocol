#include "layerx/lxp_tools.h"

#include <string.h>

lxp_result lxp_genesis_cli_main(
    lxp_genesis_cli_action action, lxp_byte_span canonical_input,
    lxp_genesis_cli_action_fn execute, void *context,
    uint8_t output[LXP_GENESIS_OUTPUT_BYTES])
{
    uint8_t root[32] = {0U};
    lxp_result status;
    uint32_t encoded_result;
    if ((action != LXP_GENESIS_BUILD &&
         action != LXP_GENESIS_RECONCILE) ||
        canonical_input.bytes == NULL || canonical_input.length == 0U ||
        execute == NULL || output == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = execute(context, action, canonical_input, root);
    encoded_result = (uint32_t)(int32_t)status;
    (void)memcpy(output, "LXGN", 4U);
    output[4] = 1U;
    output[5] = (uint8_t)action;
    output[6] = (uint8_t)(encoded_result >> 24U);
    output[7] = (uint8_t)(encoded_result >> 16U);
    output[8] = (uint8_t)(encoded_result >> 8U);
    output[9] = (uint8_t)encoded_result;
    (void)memcpy(output + 10U, root, 32U);
    return status;
}
