#include "layerx/lxp_u128.h"

#include <stdint.h>
#include <string.h>

static int equal(lxp_u128 left, lxp_u128 right)
{
    return left.hi == right.hi && left.lo == right.lo;
}

int main(void)
{
    const lxp_u128 zero = { 0U, 0U };
    const lxp_u128 one = { 0U, 1U };
    const lxp_u128 max_minus_one = { UINT64_MAX, UINT64_MAX - 1U };
    const lxp_u128 maximum = { UINT64_MAX, UINT64_MAX };
    const lxp_u128 low_max = { 0U, UINT64_MAX };
    const lxp_u128 high_one = { 1U, 0U };
    lxp_u128 out = { 7U, 9U };
    uint8_t encoded[16];
    lxp_u128 decoded;

    if (!lxp_u128_is_zero(zero) || lxp_u128_is_zero(one)) return 1;
    if (lxp_u128_cmp(zero, one) >= 0 || lxp_u128_cmp(maximum, one) <= 0 ||
        lxp_u128_cmp(maximum, maximum) != 0) return 1;
    if (lxp_u128_add(zero, one, &out) != LXP_OK || !equal(out, one)) return 1;
    if (lxp_u128_add(low_max, one, &out) != LXP_OK ||
        !equal(out, high_one)) return 1;
    if (lxp_u128_add(max_minus_one, one, &out) != LXP_OK ||
        !equal(out, maximum)) return 1;
    out = (lxp_u128){ 7U, 9U };
    if (lxp_u128_add(maximum, one, &out) != LXP_ERR_OVERFLOW ||
        !equal(out, (lxp_u128){ 7U, 9U })) return 1;
    if (lxp_u128_sub(high_one, one, &out) != LXP_OK ||
        !equal(out, low_max)) return 1;
    if (lxp_u128_sub(maximum, max_minus_one, &out) != LXP_OK ||
        !equal(out, one)) return 1;
    out = (lxp_u128){ 7U, 9U };
    if (lxp_u128_sub(zero, one, &out) != LXP_ERR_UNDERFLOW ||
        !equal(out, (lxp_u128){ 7U, 9U })) return 1;
    if (lxp_u128_to_be(max_minus_one, encoded) != LXP_OK ||
        lxp_u128_from_be(encoded, &decoded) != LXP_OK ||
        !equal(decoded, max_minus_one)) return 1;
    if (encoded[0] != 0xffU || encoded[15] != 0xfeU) return 1;
    if (lxp_u128_add(one, one, NULL) != LXP_ERR_NON_CANONICAL ||
        lxp_u128_from_be(NULL, &out) != LXP_ERR_NON_CANONICAL ||
        lxp_u128_to_be(one, NULL) != LXP_ERR_NON_CANONICAL) return 1;
    return 0;
}
