#include "layerx/lxp_u128.h"

#include <stddef.h>

static void canonicalize(lxp_i128 *value)
{
    if (lxp_u128_is_zero(value->magnitude)) value->negative = false;
}

lxp_result lxp_i128_add(lxp_i128 left, lxp_i128 right, lxp_i128 *out)
{
    lxp_i128 result;
    lxp_result status;
    int order;
    if (out == NULL) return LXP_ERR_NON_CANONICAL;
    canonicalize(&left);
    canonicalize(&right);
    if (left.negative == right.negative) {
        status = lxp_u128_add(left.magnitude, right.magnitude,
                              &result.magnitude);
        if (status != LXP_OK) return status;
        result.negative = left.negative;
    } else {
        order = lxp_u128_cmp(left.magnitude, right.magnitude);
        if (order >= 0) {
            status = lxp_u128_sub(left.magnitude, right.magnitude,
                                  &result.magnitude);
            result.negative = left.negative;
        } else {
            status = lxp_u128_sub(right.magnitude, left.magnitude,
                                  &result.magnitude);
            result.negative = right.negative;
        }
        if (status != LXP_OK) return status;
    }
    canonicalize(&result);
    *out = result;
    return LXP_OK;
}

lxp_result lxp_i128_sub(lxp_i128 left, lxp_i128 right, lxp_i128 *out)
{
    canonicalize(&right);
    if (!lxp_u128_is_zero(right.magnitude)) right.negative = !right.negative;
    return lxp_i128_add(left, right, out);
}

lxp_result lxp_u128_mul_div_floor(lxp_u128 value, lxp_u128 multiplier,
                                  lxp_u128 divisor, lxp_u128 *quotient,
                                  lxp_u128 *remainder)
{
    lxp_u256 product;
    lxp_result status;
    if (quotient == NULL || remainder == NULL) return LXP_ERR_NON_CANONICAL;
    status = lxp_u128_mul(value, multiplier, &product);
    if (status != LXP_OK) return status;
    return lxp_u256_div_floor(product, divisor, quotient, remainder);
}

lxp_result lxp_u128_mul_bps_floor(lxp_u128 value, uint32_t basis_points,
                                  lxp_u128 *out)
{
    lxp_u128 remainder;
    return lxp_u128_mul_div_floor(value,
                                  (lxp_u128){ 0U, (uint64_t)basis_points },
                                  (lxp_u128){ 0U, LXP_BASIS_POINTS_ONE },
                                  out, &remainder);
}

lxp_result lxp_u128_mul_bps_ceil(lxp_u128 value, uint32_t basis_points,
                                 lxp_u128 *out)
{
    lxp_u128 quotient;
    lxp_u128 remainder;
    lxp_result status;
    if (out == NULL) return LXP_ERR_NON_CANONICAL;
    status = lxp_u128_mul_div_floor(
        value, (lxp_u128){ 0U, (uint64_t)basis_points },
        (lxp_u128){ 0U, LXP_BASIS_POINTS_ONE }, &quotient, &remainder);
    if (status != LXP_OK) return status;
    if (!lxp_u128_is_zero(remainder)) {
        status = lxp_u128_add(quotient, (lxp_u128){ 0U, 1U }, &quotient);
        if (status != LXP_OK) return status;
    }
    *out = quotient;
    return LXP_OK;
}
