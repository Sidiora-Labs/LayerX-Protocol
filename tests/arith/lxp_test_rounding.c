#include "layerx/lxp_u128.h"

#include <stdint.h>

static int same_u128(lxp_u128 left, lxp_u128 right)
{
    return left.hi == right.hi && left.lo == right.lo;
}

static int same_i128(lxp_i128 left, lxp_i128 right)
{
    return left.negative == right.negative &&
           same_u128(left.magnitude, right.magnitude);
}

int main(void)
{
    const lxp_i128 positive_five = { false, { 0U, 5U } };
    const lxp_i128 negative_three = { true, { 0U, 3U } };
    const lxp_i128 negative_zero = { true, { 0U, 0U } };
    lxp_i128 signed_out;
    lxp_u128 quotient;
    lxp_u128 remainder;
    lxp_u128 payout_debit;
    lxp_u128 payout_credit;
    lxp_u128 fee_debit;
    lxp_u128 fee_credit;

    if (lxp_i128_add(positive_five, negative_three, &signed_out) != LXP_OK ||
        !same_i128(signed_out, (lxp_i128){ false, { 0U, 2U } })) return 1;
    if (lxp_i128_sub(negative_three, positive_five, &signed_out) != LXP_OK ||
        !same_i128(signed_out, (lxp_i128){ true, { 0U, 8U } })) return 1;
    if (lxp_i128_add(negative_zero, negative_zero, &signed_out) != LXP_OK ||
        !same_i128(signed_out, (lxp_i128){ false, { 0U, 0U } })) return 1;
    signed_out = (lxp_i128){ false, { 7U, 9U } };
    if (lxp_i128_add((lxp_i128){ false, { UINT64_MAX, UINT64_MAX } },
                     (lxp_i128){ false, { 0U, 1U } }, &signed_out) !=
        LXP_ERR_OVERFLOW ||
        !same_i128(signed_out, (lxp_i128){ false, { 7U, 9U } })) return 1;

    if (lxp_u128_mul_div_floor((lxp_u128){ 0U, 10U },
                               (lxp_u128){ 0U, 3U },
                               (lxp_u128){ 0U, 4U }, &quotient,
                               &remainder) != LXP_OK ||
        !same_u128(quotient, (lxp_u128){ 0U, 7U }) ||
        !same_u128(remainder, (lxp_u128){ 0U, 2U })) return 1;
    if (lxp_u128_mul_div_floor((lxp_u128){ 0U, 1U },
                               (lxp_u128){ 0U, 1U },
                               (lxp_u128){ 0U, 0U }, &quotient,
                               &remainder) != LXP_ERR_DIV_ZERO) return 1;

    if (lxp_u128_mul_bps_floor((lxp_u128){ 0U, 101U }, 100U,
                               &payout_debit) != LXP_OK ||
        lxp_u128_mul_bps_floor((lxp_u128){ 0U, 101U }, 100U,
                               &payout_credit) != LXP_OK ||
        !same_u128(payout_debit, (lxp_u128){ 0U, 1U }) ||
        !same_u128(payout_debit, payout_credit)) return 1;
    if (lxp_u128_mul_bps_ceil((lxp_u128){ 0U, 101U }, 100U,
                              &fee_debit) != LXP_OK ||
        lxp_u128_mul_bps_ceil((lxp_u128){ 0U, 101U }, 100U,
                              &fee_credit) != LXP_OK ||
        !same_u128(fee_debit, (lxp_u128){ 0U, 2U }) ||
        !same_u128(fee_debit, fee_credit)) return 1;
    if (lxp_u128_mul_bps_floor((lxp_u128){ UINT64_MAX, UINT64_MAX },
                               LXP_BASIS_POINTS_ONE, &quotient) != LXP_OK ||
        !same_u128(quotient, (lxp_u128){ UINT64_MAX, UINT64_MAX })) return 1;
    return 0;
}
