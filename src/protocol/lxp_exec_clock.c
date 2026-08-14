#include "layerx/lxp_batch.h"

lxp_result lxp_exec_clock_bind(lxp_exec_clock *clock,
                               const lxp_batch_header *sealed_header)
{
    if (clock == NULL || sealed_header == NULL ||
        sealed_header->timestamp_ms == 0U) return LXP_ERR_NON_CANONICAL;
    clock->sealed_timestamp_ms = sealed_header->timestamp_ms;
    clock->bound = 1U;
    return LXP_OK;
}

lxp_result lxp_exec_clock_read(const lxp_exec_clock *clock,
                               uint64_t *timestamp_ms)
{
    if (clock == NULL || timestamp_ms == NULL || clock->bound != 1U)
        return LXP_FATAL_INVARIANT;
    *timestamp_ms = clock->sealed_timestamp_ms;
    return LXP_OK;
}
