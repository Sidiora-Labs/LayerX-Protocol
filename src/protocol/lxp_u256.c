#include "layerx/lxp_u256.h"

#include <stddef.h>

lxp_result lxp_u128_mul(lxp_u128 left, lxp_u128 right, lxp_u256 *out)
{
    uint32_t a[4];
    uint32_t b[4];
    uint32_t product[8] = { 0U, 0U, 0U, 0U, 0U, 0U, 0U, 0U };
    size_t i;
    if (out == NULL) return LXP_ERR_NON_CANONICAL;
    a[0] = (uint32_t)left.lo;
    a[1] = (uint32_t)(left.lo >> 32U);
    a[2] = (uint32_t)left.hi;
    a[3] = (uint32_t)(left.hi >> 32U);
    b[0] = (uint32_t)right.lo;
    b[1] = (uint32_t)(right.lo >> 32U);
    b[2] = (uint32_t)right.hi;
    b[3] = (uint32_t)(right.hi >> 32U);
    for (i = 0U; i < 4U; ++i) {
        uint64_t carry = 0U;
        size_t j;
        for (j = 0U; j < 4U; ++j) {
            uint64_t sum = (uint64_t)a[i] * (uint64_t)b[j] +
                           (uint64_t)product[i + j] + carry;
            product[i + j] = (uint32_t)sum;
            carry = sum >> 32U;
        }
        product[i + 4U] = (uint32_t)carry;
    }
    for (i = 0U; i < 4U; ++i) {
        out->words[i] = (uint64_t)product[i * 2U] |
                        ((uint64_t)product[i * 2U + 1U] << 32U);
    }
    return LXP_OK;
}

lxp_result lxp_u256_add(lxp_u256 left, lxp_u256 right, lxp_u256 *out)
{
    lxp_u256 result;
    uint64_t carry = 0U;
    size_t i;
    if (out == NULL) return LXP_ERR_NON_CANONICAL;
    for (i = 0U; i < 4U; ++i) {
        uint64_t sum = left.words[i] + right.words[i];
        uint64_t carry_from_operands = sum < left.words[i] ? 1U : 0U;
        uint64_t with_carry = sum + carry;
        uint64_t carry_from_input = with_carry < sum ? 1U : 0U;
        result.words[i] = with_carry;
        carry = carry_from_operands | carry_from_input;
    }
    if (carry != 0U) return LXP_ERR_OVERFLOW;
    *out = result;
    return LXP_OK;
}

static uint64_t dividend_low_bit(lxp_u256 dividend, size_t bit)
{
    size_t word = bit / 64U;
    size_t offset = bit % 64U;
    return (dividend.words[word] >> offset) & UINT64_C(1);
}

lxp_result lxp_u256_div_floor(lxp_u256 dividend, lxp_u128 divisor,
                              lxp_u128 *quotient, lxp_u128 *remainder)
{
    lxp_u128 top = { dividend.words[3], dividend.words[2] };
    lxp_u128 q = { 0U, 0U };
    lxp_u128 r;
    size_t bit;
    if (quotient == NULL || remainder == NULL) return LXP_ERR_NON_CANONICAL;
    if (lxp_u128_is_zero(divisor)) return LXP_ERR_DIV_ZERO;
    if (lxp_u128_cmp(top, divisor) >= 0) return LXP_ERR_OVERFLOW;
    r = top;
    for (bit = 128U; bit-- > 0U;) {
        uint64_t high_carry = r.hi >> 63U;
        lxp_u128 shifted = {
            (r.hi << 1U) | (r.lo >> 63U),
            (r.lo << 1U) | dividend_low_bit(dividend, bit)
        };
        if (high_carry != 0U || lxp_u128_cmp(shifted, divisor) >= 0) {
            if (high_carry != 0U) {
                uint64_t borrow = shifted.lo < divisor.lo ? 1U : 0U;
                shifted.lo -= divisor.lo;
                shifted.hi = shifted.hi - divisor.hi - borrow;
            } else {
                lxp_result result = lxp_u128_sub(shifted, divisor, &shifted);
                if (result != LXP_OK) return result;
            }
            if (bit >= 64U) q.hi |= UINT64_C(1) << (bit - 64U);
            else q.lo |= UINT64_C(1) << bit;
        }
        r = shifted;
    }
    *quotient = q;
    *remainder = r;
    return LXP_OK;
}
