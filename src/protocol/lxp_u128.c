#include "layerx/lxp_u128.h"

#include <stddef.h>

lxp_result lxp_u128_add(lxp_u128 left, lxp_u128 right, lxp_u128 *out)
{
    lxp_u128 result;
    if (out == NULL) return LXP_ERR_NON_CANONICAL;
    result.lo = left.lo + right.lo;
    result.hi = left.hi + right.hi;
    if (result.hi < left.hi) return LXP_ERR_OVERFLOW;
    if (result.lo < left.lo) {
        if (result.hi == UINT64_MAX) return LXP_ERR_OVERFLOW;
        ++result.hi;
    }
    *out = result;
    return LXP_OK;
}

lxp_result lxp_u128_sub(lxp_u128 left, lxp_u128 right, lxp_u128 *out)
{
    lxp_u128 result;
    uint64_t borrow;
    if (out == NULL) return LXP_ERR_NON_CANONICAL;
    if (lxp_u128_cmp(left, right) < 0) return LXP_ERR_UNDERFLOW;
    borrow = left.lo < right.lo ? 1U : 0U;
    result.lo = left.lo - right.lo;
    result.hi = left.hi - right.hi - borrow;
    *out = result;
    return LXP_OK;
}

int lxp_u128_cmp(lxp_u128 left, lxp_u128 right)
{
    if (left.hi < right.hi) return -1;
    if (left.hi > right.hi) return 1;
    if (left.lo < right.lo) return -1;
    return left.lo > right.lo ? 1 : 0;
}

bool lxp_u128_is_zero(lxp_u128 value)
{
    return value.hi == 0U && value.lo == 0U;
}

lxp_result lxp_u128_from_be(const uint8_t bytes[16], lxp_u128 *out)
{
    size_t i;
    lxp_u128 result = { 0U, 0U };
    if (bytes == NULL || out == NULL) return LXP_ERR_NON_CANONICAL;
    for (i = 0U; i < 8U; ++i) {
        result.hi = (result.hi << 8U) | bytes[i];
        result.lo = (result.lo << 8U) | bytes[i + 8U];
    }
    *out = result;
    return LXP_OK;
}

lxp_result lxp_u128_to_be(lxp_u128 value, uint8_t bytes[16])
{
    size_t i;
    if (bytes == NULL) return LXP_ERR_NON_CANONICAL;
    for (i = 0U; i < 8U; ++i) {
        bytes[7U - i] = (uint8_t)(value.hi >> (i * 8U));
        bytes[15U - i] = (uint8_t)(value.lo >> (i * 8U));
    }
    return LXP_OK;
}
