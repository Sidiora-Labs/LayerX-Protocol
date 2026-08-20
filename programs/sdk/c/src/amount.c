#include "layerx/program.h"

/*
 * Protocol amounts are unsigned one hundred and twenty-eight bit integers held
 * as two sixty-four bit halves. Every operation is checked and a refusal leaves
 * the output unchanged. There is no floating point anywhere in this file and no
 * bare arithmetic operator is exposed on the type.
 */

lxp_program_amount lxp_program_amount_from_parts(uint64_t hi, uint64_t lo)
{
    lxp_program_amount value;
    value.hi = hi;
    value.lo = lo;
    return value;
}

lxp_program_amount lxp_program_amount_from_words(uint64_t hi, uint64_t lo)
{
    return lxp_program_amount_from_parts(hi, lo);
}

lxp_program_amount lxp_program_amount_from_be(const uint8_t bytes[16])
{
    if (bytes == NULL) return lxp_program_amount_from_parts(0U, 0U);
    return lxp_program_amount_from_parts(lxp_program_read_u64_be(bytes),
                                         lxp_program_read_u64_be(bytes + 8));
}

void lxp_program_amount_to_be(lxp_program_amount value, uint8_t bytes[16])
{
    if (bytes == NULL) return;
    lxp_program_write_u64_be(bytes, value.hi);
    lxp_program_write_u64_be(bytes + 8, value.lo);
}

bool lxp_program_amount_is_zero(lxp_program_amount value)
{
    return value.hi == 0U && value.lo == 0U;
}

int lxp_program_amount_cmp(lxp_program_amount left, lxp_program_amount right)
{
    if (left.hi != right.hi) return left.hi < right.hi ? -1 : 1;
    if (left.lo != right.lo) return left.lo < right.lo ? -1 : 1;
    return 0;
}

lxp_program_status lxp_program_amount_add(lxp_program_amount left,
                                          lxp_program_amount right,
                                          lxp_program_amount *out)
{
    uint64_t lo;
    uint64_t hi;
    uint64_t carry;
    if (out == NULL) return LXP_PROGRAM_ERR_NULL_ARGUMENT;
    lo = left.lo + right.lo;
    carry = lo < left.lo ? 1U : 0U;
    if (left.hi > UINT64_MAX - right.hi) return LXP_PROGRAM_ERR_OVERFLOW;
    hi = left.hi + right.hi;
    if (hi > UINT64_MAX - carry) return LXP_PROGRAM_ERR_OVERFLOW;
    hi = hi + carry;
    out->hi = hi;
    out->lo = lo;
    return LXP_PROGRAM_OK;
}

lxp_program_status lxp_program_amount_sub(lxp_program_amount left,
                                          lxp_program_amount right,
                                          lxp_program_amount *out)
{
    uint64_t lo;
    uint64_t hi;
    uint64_t borrow;
    if (out == NULL) return LXP_PROGRAM_ERR_NULL_ARGUMENT;
    if (lxp_program_amount_cmp(left, right) < 0)
        return LXP_PROGRAM_ERR_UNDERFLOW;
    lo = left.lo - right.lo;
    borrow = left.lo < right.lo ? 1U : 0U;
    hi = left.hi - right.hi - borrow;
    out->hi = hi;
    out->lo = lo;
    return LXP_PROGRAM_OK;
}
