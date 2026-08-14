#include "layerx/lxp_u256.h"

#include <stddef.h>
#include <stdint.h>

static int u128_equal(lxp_u128 left, lxp_u128 right)
{
    return left.hi == right.hi && left.lo == right.lo;
}

static int u256_equal(lxp_u256 left, lxp_u256 right)
{
    size_t i;
    for (i = 0U; i < 4U; ++i)
        if (left.words[i] != right.words[i]) return 0;
    return 1;
}

static int check_division(lxp_u128 value, lxp_u128 divisor,
                          lxp_u128 expected_q, lxp_u128 expected_r)
{
    lxp_u256 product;
    lxp_u128 q;
    lxp_u128 r;
    if (lxp_u128_mul(value, (lxp_u128){ 0U, 1U }, &product) != LXP_OK)
        return 0;
    if (lxp_u256_div_floor(product, divisor, &q, &r) != LXP_OK) return 0;
    return u128_equal(q, expected_q) && u128_equal(r, expected_r);
}

int main(void)
{
    const lxp_u128 maximum = { UINT64_MAX, UINT64_MAX };
    lxp_u256 product;
    lxp_u256 out;
    lxp_u128 quotient;
    lxp_u128 remainder;
    lxp_u256 all_ones = {{ UINT64_MAX, UINT64_MAX, UINT64_MAX, UINT64_MAX }};

    if (lxp_u128_mul(maximum, maximum, &product) != LXP_OK ||
        !u256_equal(product, (lxp_u256){{ 1U, 0U, UINT64_MAX - 1U,
                                         UINT64_MAX }})) return 1;
    if (lxp_u128_mul((lxp_u128){ 1U, 0U }, (lxp_u128){ 1U, 0U },
                     &product) != LXP_OK ||
        !u256_equal(product, (lxp_u256){{ 0U, 0U, 1U, 0U }})) return 1;
    if (lxp_u256_add((lxp_u256){{ UINT64_MAX, UINT64_MAX, UINT64_MAX - 1U, 0U }},
                     (lxp_u256){{ 1U, 0U, 1U, 0U }}, &out) != LXP_OK ||
        !u256_equal(out, (lxp_u256){{ 0U, 0U, 0U, 1U }})) return 1;
    out = (lxp_u256){{ 7U, 8U, 9U, 10U }};
    if (lxp_u256_add(all_ones, (lxp_u256){{ 1U, 0U, 0U, 0U }}, &out) !=
        LXP_ERR_OVERFLOW || out.words[0] != 7U) return 1;
    if (!check_division(maximum, (lxp_u128){ 0U, 1U }, maximum,
                        (lxp_u128){ 0U, 0U })) return 1;
    if (!check_division(maximum, (lxp_u128){ 1U, 0U },
                        (lxp_u128){ 0U, UINT64_MAX },
                        (lxp_u128){ 0U, UINT64_MAX })) return 1;
    if (!check_division(maximum, maximum, (lxp_u128){ 0U, 1U },
                        (lxp_u128){ 0U, 0U })) return 1;
    product = (lxp_u256){{ UINT64_MAX, UINT64_MAX, 0U, 0U }};
    if (lxp_u256_div_floor(product, (lxp_u128){ 0U, 2U }, &quotient,
                               &remainder) != LXP_OK ||
        !u128_equal(quotient, (lxp_u128){ UINT64_C(0x7fffffffffffffff),
                                          UINT64_MAX }) ||
        !u128_equal(remainder, (lxp_u128){ 0U, 1U })) return 1;
    if (lxp_u256_div_floor(product, (lxp_u128){ 0U, 0U }, &quotient,
                               &remainder) != LXP_ERR_DIV_ZERO) return 1;
    product = (lxp_u256){{ 0U, 0U, 1U, 0U }};
    if (lxp_u256_div_floor(product, (lxp_u128){ 0U, 1U }, &quotient,
                               &remainder) != LXP_ERR_OVERFLOW) return 1;
    return 0;
}
