#include "layerx/lx_stream.h"

#include <string.h>

lxp_result lx_stream_elapsed_ms(const lx_stream_record *record,
                                uint64_t batch_timestamp,
                                uint64_t *elapsed_ms)
{
    uint64_t effective;
    if (record == NULL || elapsed_ms == NULL)
        return LXP_ERR_NON_CANONICAL;
    if (batch_timestamp < record->last_accrual_timestamp)
        return LXP_ERR_NON_MONOTONIC_TIME;
    effective = batch_timestamp;
    if (record->end_timestamp != 0U && effective > record->end_timestamp)
        effective = record->end_timestamp;
    if (effective < record->last_accrual_timestamp)
        *elapsed_ms = 0U;
    else
        *elapsed_ms = effective - record->last_accrual_timestamp;
    return LXP_OK;
}

lxp_result lx_stream_carry_apply(lx_stream_record *record,
                                 lxp_u128 remainder)
{
    if (record == NULL ||
        lxp_u128_cmp(remainder, (lxp_u128){ 0U, record->rate_unit }) >= 0)
        return LXP_FATAL_INVARIANT;
    record->remainder_carry = remainder;
    return LXP_OK;
}

lxp_result lx_stream_accrue(lx_stream_record *record,
                            uint64_t batch_timestamp,
                            lxp_u128 *newly_accrued)
{
    uint64_t elapsed;
    uint64_t effective;
    lxp_u256 product;
    lxp_u256 carry;
    lxp_u256 numerator;
    lxp_u128 quotient;
    lxp_u128 remainder;
    lxp_u128 cap_remaining;
    lxp_u128 updated_total;
    lxp_result status;
    if (record == NULL || newly_accrued == NULL || record->rate_unit == 0U ||
        record->mode != LX_STREAM_MODE_TIME)
        return LXP_ERR_NON_CANONICAL;
    *newly_accrued = (lxp_u128){ 0U, 0U };
    status = lx_stream_elapsed_ms(record, batch_timestamp, &elapsed);
    if (status != LXP_OK) return status;
    if (record->closed || record->paused || record->underfunded || elapsed == 0U ||
        lxp_u128_cmp(record->accrued_total, record->total_cap) >= 0)
        return LXP_OK;
    status = lxp_u128_mul(record->rate, (lxp_u128){ 0U, elapsed }, &product);
    if (status != LXP_OK) return LXP_ERR_ACCRUAL_OVERFLOW;
    (void)memset(&carry, 0, sizeof(carry));
    carry.words[0] = record->remainder_carry.lo;
    carry.words[1] = record->remainder_carry.hi;
    status = lxp_u256_add(product, carry, &numerator);
    if (status != LXP_OK) return LXP_ERR_ACCRUAL_OVERFLOW;
    status = lxp_u256_div_floor(numerator,
                                (lxp_u128){ 0U, record->rate_unit },
                                &quotient, &remainder);
    if (status != LXP_OK) return LXP_ERR_ACCRUAL_OVERFLOW;
    status = lxp_u128_sub(record->total_cap, record->accrued_total,
                          &cap_remaining);
    if (status != LXP_OK) return LXP_FATAL_INVARIANT;
    if (lxp_u128_cmp(quotient, cap_remaining) > 0) {
        quotient = cap_remaining;
        remainder = (lxp_u128){ 0U, 0U };
    }
    status = lxp_u128_add(record->accrued_total, quotient, &updated_total);
    if (status != LXP_OK) return LXP_ERR_ACCRUAL_OVERFLOW;
    status = lx_stream_carry_apply(record, remainder);
    if (status != LXP_OK) return status;
    effective = batch_timestamp;
    if (record->end_timestamp != 0U && effective > record->end_timestamp)
        effective = record->end_timestamp;
    record->last_accrual_timestamp = effective;
    record->accrued_total = updated_total;
    *newly_accrued = quotient;
    return LXP_OK;
}
