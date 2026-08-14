#include "layerx/lxp_fee.h"

#include <stddef.h>
#include <string.h>

static lxp_result storage_price(lxp_u128 rate, uint64_t bytes,
                                lxp_u128 *price)
{
    lxp_u128 remainder;
    return lxp_u128_mul_div_floor(rate, (lxp_u128){0U, bytes},
                                  (lxp_u128){0U, 1U}, price, &remainder);
}

lxp_result lxp_meter_init(
    lxp_meter_ctx *meter, uint64_t execution_ceiling,
    uint64_t storage_ceiling, lxp_u128 storage_rate, lxp_u128 fee_limit,
    uint32_t parameter_version, bool single_writer_bound)
{
    if (meter == NULL || parameter_version == 0U ||
        !single_writer_bound)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(meter, 0, sizeof(*meter));
    meter->execution_ceiling = execution_ceiling;
    meter->storage_ceiling = storage_ceiling;
    meter->storage_rate = storage_rate;
    meter->fee_limit = fee_limit;
    meter->parameter_version = parameter_version;
    meter->single_writer_bound = true;
    return LXP_OK;
}

lxp_result lxp_meter_charge_exec(lxp_meter_ctx *meter, uint64_t units)
{
    uint64_t updated;
    if (meter == NULL || !meter->single_writer_bound)
        return LXP_ERR_NON_CANONICAL;
    if (meter->exhausted) return LXP_ERR_GAS_EXHAUSTED;
    if (UINT64_MAX - meter->execution_units < units)
        return LXP_ERR_OVERFLOW;
    updated = meter->execution_units + units;
    meter->execution_units = updated;
    if (updated > meter->execution_ceiling) {
        meter->exhausted = true;
        return LXP_ERR_GAS_EXHAUSTED;
    }
    return LXP_OK;
}

lxp_result lxp_meter_charge_storage(lxp_meter_ctx *meter,
                                    int64_t net_byte_delta)
{
    uint64_t updated;
    lxp_u128 updated_fee;
    if (meter == NULL || !meter->single_writer_bound)
        return LXP_ERR_NON_CANONICAL;
    if (meter->exhausted) return LXP_ERR_GAS_EXHAUSTED;
    if (net_byte_delta >= 0) {
        uint64_t growth = (uint64_t)net_byte_delta;
        if (UINT64_MAX - meter->net_storage_bytes < growth)
            return LXP_ERR_OVERFLOW;
        updated = meter->net_storage_bytes + growth;
    } else {
        uint64_t shrink = (uint64_t)(-(net_byte_delta + 1)) + 1U;
        if (shrink > meter->net_storage_bytes)
            return LXP_ERR_OVERFLOW;
        updated = meter->net_storage_bytes - shrink;
    }
    if (storage_price(meter->storage_rate, updated, &updated_fee) != LXP_OK)
        return LXP_ERR_OVERFLOW;
    meter->net_storage_bytes = updated;
    meter->storage_fee = updated_fee;
    if (updated > meter->storage_ceiling) {
        meter->exhausted = true;
        return LXP_ERR_GAS_EXHAUSTED;
    }
    return LXP_OK;
}

lxp_result lxp_meter_exhausted(const lxp_meter_ctx *meter)
{
    if (meter == NULL || !meter->single_writer_bound)
        return LXP_ERR_NON_CANONICAL;
    return meter->exhausted ||
           meter->execution_units > meter->execution_ceiling ||
           meter->net_storage_bytes > meter->storage_ceiling ?
           LXP_ERR_GAS_EXHAUSTED : LXP_OK;
}

lxp_result lxp_meter_fee_usage(const lxp_meter_ctx *meter,
                               uint64_t canonical_encoded_bytes,
                               lxp_fee_meter *usage)
{
    if (meter == NULL || usage == NULL || !meter->single_writer_bound)
        return LXP_ERR_NON_CANONICAL;
    usage->canonical_encoded_bytes = canonical_encoded_bytes;
    usage->execution_units = meter->execution_units;
    usage->storage_units = meter->net_storage_bytes;
    return lxp_meter_exhausted(meter);
}

lxp_result lxp_meter_admission_check(bool fee_limit_present,
                                     bool canonical_nonnegative_integer,
                                     lxp_u128 fee_limit,
                                     lxp_u128 actor_spendable_fee_balance)
{
    if (!fee_limit_present || !canonical_nonnegative_integer)
        return LXP_ERR_MALFORMED_ENVELOPE;
    return lxp_fee_limit_check((lxp_u128){0U, 0U}, fee_limit,
                               actor_spendable_fee_balance);
}
