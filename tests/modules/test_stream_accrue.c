#include "layerx/lx_stream.h"

#include <string.h>

static int replay(lx_stream_record *record)
{
    static const uint64_t timestamps[] = { 250U, 500U, 1000U, 1400U };
    size_t i;
    for (i = 0U; i < sizeof(timestamps) / sizeof(timestamps[0]); ++i) {
        lxp_u128 accrued;
        if (lx_stream_accrue(record, timestamps[i], &accrued) != LXP_OK)
            return 1;
    }
    return 0;
}

int main(void)
{
    lx_stream_record first;
    lx_stream_record second;
    lx_stream_record capped;
    lx_stream_record overflow;
    lxp_u128 accrued;
    uint64_t elapsed;

    (void)memset(&first, 0, sizeof(first));
    first.mode = LX_STREAM_MODE_TIME;
    first.rate = (lxp_u128){ 0U, 3U };
    first.rate_unit = 1000U;
    first.start_timestamp = 100U;
    first.last_accrual_timestamp = 100U;
    first.end_timestamp = 1100U;
    first.total_cap = (lxp_u128){ 0U, 100U };
    second = first;
    if (replay(&first) != 0 || replay(&second) != 0 ||
        memcmp(&first, &second, sizeof(first)) != 0 ||
        first.accrued_total.lo != 3U || first.remainder_carry.lo != 0U ||
        first.last_accrual_timestamp != 1100U ||
        lx_stream_elapsed_ms(&first, 1099U, &elapsed) !=
            LXP_ERR_NON_MONOTONIC_TIME)
        return 1;

    capped = first;
    capped.last_accrual_timestamp = 100U;
    capped.end_timestamp = 0U;
    capped.accrued_total = (lxp_u128){ 0U, 0U };
    capped.total_cap = (lxp_u128){ 0U, 2U };
    capped.remainder_carry = (lxp_u128){ 0U, 0U };
    if (lx_stream_accrue(&capped, 1100U, &accrued) != LXP_OK ||
        accrued.lo != 2U || capped.accrued_total.lo != 2U ||
        !lxp_u128_is_zero(capped.remainder_carry))
        return 1;

    (void)memset(&overflow, 0, sizeof(overflow));
    overflow.mode = LX_STREAM_MODE_TIME;
    overflow.rate = (lxp_u128){ UINT64_MAX, UINT64_MAX };
    overflow.rate_unit = 1U;
    overflow.last_accrual_timestamp = 1U;
    overflow.total_cap = (lxp_u128){ UINT64_MAX, UINT64_MAX };
    if (lx_stream_accrue(&overflow, UINT64_MAX, &accrued) !=
        LXP_ERR_ACCRUAL_OVERFLOW)
        return 1;
    return 0;
}
