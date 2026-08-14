#include "layerx/lxp_fee.h"

#include <string.h>

static lxp_result resolve(const lxp_param_table *parameters, const char *name,
                          uint64_t epoch, const uint8_t cohort_id[32],
                          uint64_t *value, uint32_t *version)
{
    return lxp_gov_param_enact(
        parameters,
        (lxp_byte_span){(const uint8_t *)name, strlen(name)},
        epoch, cohort_id, value, version);
}

lxp_result lxp_fee_schedule(
    const lxp_param_table *parameters, uint64_t batch_epoch,
    const uint8_t cohort_id[32], lxp_fee_params *schedule,
    uint32_t *parameter_version)
{
    static const char *const keys[6] = {
        "fee.base", "fee.activity", "fee.byte", "fee.exec",
        "fee.storage", "fee.multiplier_bps"
    };
    uint64_t values[6];
    uint32_t version = 0U;
    size_t i;
    lxp_result status;
    if (parameters == NULL || schedule == NULL || parameter_version == NULL)
        return LXP_ERR_NON_CANONICAL;
    for (i = 0U; i < 6U; ++i) {
        uint32_t resolved_version;
        status = resolve(parameters, keys[i], batch_epoch, cohort_id,
                         &values[i], &resolved_version);
        if (status != LXP_OK) return status;
        if (i == 0U) version = resolved_version;
        else if (resolved_version != version) return LXP_FATAL_INVARIANT;
    }
    if (values[5] > UINT32_MAX) return LXP_ERR_PARAMETER_BOUNDS;
    (void)memset(schedule, 0, sizeof(*schedule));
    schedule->version = 1U;
    schedule->base_fee = (lxp_u128){0U, values[0]};
    schedule->per_activity_type_unit = (lxp_u128){0U, values[1]};
    schedule->per_encoded_byte = (lxp_u128){0U, values[2]};
    schedule->per_execution_unit = (lxp_u128){0U, values[3]};
    schedule->per_storage_unit = (lxp_u128){0U, values[4]};
    schedule->multiplier_basis_points = (uint32_t)values[5];
    *parameter_version = version;
    return LXP_OK;
}
