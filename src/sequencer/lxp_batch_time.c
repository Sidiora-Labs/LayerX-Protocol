#include "layerx/lxp_batch.h"

lxp_result lxp_batch_timestamp_select(lxp_batch_header *header,
                                      uint64_t timestamp_ms)
{
    if (header == NULL || timestamp_ms == 0U) return LXP_ERR_NON_CANONICAL;
    if (header->timestamp_ms == 0U) {
        header->timestamp_ms = timestamp_ms;
        return LXP_OK;
    }
    return header->timestamp_ms == timestamp_ms ? LXP_OK :
           LXP_ERR_NON_CANONICAL;
}

lxp_result lxp_batch_timestamp_validate(const lxp_batch_header *previous,
                                        const lxp_batch_header *candidate,
                                        uint64_t maximum_forward_drift_ms)
{
    if (previous == NULL || candidate == NULL ||
        maximum_forward_drift_ms == 0U) return LXP_ERR_NON_CANONICAL;
    if (candidate->timestamp_ms <= previous->timestamp_ms)
        return LXP_ERR_TIMESTAMP_REGRESSION;
    if (candidate->timestamp_ms - previous->timestamp_ms >
        maximum_forward_drift_ms) return LXP_ERR_TIMESTAMP_REGRESSION;
    return LXP_OK;
}
