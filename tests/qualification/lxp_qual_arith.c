#include "layerx/lxp_qualification.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

static const uint64_t boundary_words[] = {
    UINT64_C(0),
    UINT64_C(1),
    UINT64_C(2),
    UINT64_C(3),
    UINT64_C(0x7fffffff),
    UINT64_C(0x80000000),
    UINT64_C(0xffffffff),
    UINT64_C(0x100000000),
    UINT64_C(0x7fffffffffffffff),
    UINT64_C(0x8000000000000000),
    UINT64_MAX - UINT64_C(1),
    UINT64_MAX
};

static int u128_equal(lxp_u128 left, lxp_u128 right)
{
    return left.hi == right.hi && left.lo == right.lo;
}

static int u256_equal(lxp_u256 left, lxp_u256 right)
{
    size_t word;
    for (word = 0U; word < 4U; ++word)
        if (left.words[word] != right.words[word]) return 0;
    return 1;
}

static __uint128_t native_u128(lxp_u128 value)
{
    return ((__uint128_t)value.hi << 64U) | (__uint128_t)value.lo;
}

static lxp_u128 split_u128(__uint128_t value)
{
    return (lxp_u128){ (uint64_t)(value >> 64U), (uint64_t)value };
}

static lxp_u256 reference_mul(lxp_u128 left, lxp_u128 right)
{
    __uint128_t p00 = (__uint128_t)left.lo * (__uint128_t)right.lo;
    __uint128_t p01 = (__uint128_t)left.lo * (__uint128_t)right.hi;
    __uint128_t p10 = (__uint128_t)left.hi * (__uint128_t)right.lo;
    __uint128_t p11 = (__uint128_t)left.hi * (__uint128_t)right.hi;
    __uint128_t middle = (p00 >> 64U) + (uint64_t)p01 + (uint64_t)p10;
    __uint128_t high = (p01 >> 64U) + (p10 >> 64U) +
                       (uint64_t)p11 + (middle >> 64U);
    lxp_u256 result = {{
        (uint64_t)p00,
        (uint64_t)middle,
        (uint64_t)high,
        (uint64_t)((p11 >> 64U) + (high >> 64U))
    }};
    return result;
}

static lxp_result check_u128_pair(lxp_u128 left, lxp_u128 right)
{
    const lxp_u128 sentinel = { UINT64_C(0x55aa55aa55aa55aa),
                                UINT64_C(0xaa55aa55aa55aa55) };
    __uint128_t native_left = native_u128(left);
    __uint128_t native_right = native_u128(right);
    __uint128_t maximum = ~(__uint128_t)0U;
    lxp_u128 out = sentinel;
    lxp_u256 product;
    lxp_result status;

    status = lxp_u128_add(left, right, &out);
    if (native_right > maximum - native_left) {
        if (status != LXP_ERR_OVERFLOW || !u128_equal(out, sentinel))
            return LXP_FATAL_INVARIANT;
    } else if (status != LXP_OK ||
               !u128_equal(out, split_u128(native_left + native_right))) {
        return LXP_FATAL_INVARIANT;
    }

    out = sentinel;
    status = lxp_u128_sub(left, right, &out);
    if (native_left < native_right) {
        if (status != LXP_ERR_UNDERFLOW || !u128_equal(out, sentinel))
            return LXP_FATAL_INVARIANT;
    } else if (status != LXP_OK ||
               !u128_equal(out, split_u128(native_left - native_right))) {
        return LXP_FATAL_INVARIANT;
    }

    if (lxp_u128_mul(left, right, &product) != LXP_OK ||
        !u256_equal(product, reference_mul(left, right)))
        return LXP_FATAL_INVARIANT;
    return LXP_OK;
}

lxp_result lxp_u128_proof_harness(uint64_t *case_count)
{
    size_t a;
    size_t b;
    size_t c;
    size_t d;
    uint64_t count = 0U;
    if (case_count == NULL) return LXP_ERR_NON_CANONICAL;
    for (a = 0U; a < sizeof(boundary_words) / sizeof(boundary_words[0]); ++a) {
        for (b = 0U; b < sizeof(boundary_words) / sizeof(boundary_words[0]);
             ++b) {
            lxp_u128 left = { boundary_words[a], boundary_words[b] };
            for (c = 0U;
                 c < sizeof(boundary_words) / sizeof(boundary_words[0]); ++c) {
                for (d = 0U;
                     d < sizeof(boundary_words) / sizeof(boundary_words[0]);
                     ++d) {
                    lxp_u128 right = { boundary_words[c], boundary_words[d] };
                    lxp_result status = check_u128_pair(left, right);
                    lxp_u256 dividend = {{ left.lo, left.hi, 0U, 0U }};
                    lxp_u128 quotient = { 0U, 0U };
                    lxp_u128 remainder = { 0U, 0U };
                    if (status != LXP_OK) return status;
                    status = lxp_u256_div_floor(dividend, right, &quotient,
                                                &remainder);
                    if (lxp_u128_is_zero(right)) {
                        if (status != LXP_ERR_DIV_ZERO)
                            return LXP_FATAL_INVARIANT;
                    } else if (status != LXP_OK ||
                               !u128_equal(quotient, split_u128(
                                   native_u128(left) / native_u128(right))) ||
                               !u128_equal(remainder, split_u128(
                                   native_u128(left) % native_u128(right)))) {
                        return LXP_FATAL_INVARIANT;
                    }
                    ++count;
                }
            }
        }
    }
    *case_count = count;
    return LXP_OK;
}

static lxp_u256 boundary_pattern(size_t offset)
{
    size_t count = sizeof(boundary_words) / sizeof(boundary_words[0]);
    lxp_u256 value;
    size_t word;
    for (word = 0U; word < 4U; ++word)
        value.words[word] = boundary_words[(offset + word * 3U) % count];
    return value;
}

static lxp_result division_invariant(lxp_u256 dividend, lxp_u128 divisor)
{
    const lxp_u128 sentinel_q = { UINT64_C(0x1122334455667788),
                                  UINT64_C(0x99aabbccddeeff00) };
    const lxp_u128 sentinel_r = { UINT64_C(0x0f1e2d3c4b5a6978),
                                  UINT64_C(0x8796a5b4c3d2e1f0) };
    lxp_u128 quotient = sentinel_q;
    lxp_u128 remainder = sentinel_r;
    lxp_u128 top = { dividend.words[3], dividend.words[2] };
    lxp_result status = lxp_u256_div_floor(dividend, divisor, &quotient,
                                           &remainder);
    if (lxp_u128_is_zero(divisor)) {
        return status == LXP_ERR_DIV_ZERO && u128_equal(quotient, sentinel_q) &&
                               u128_equal(remainder, sentinel_r) ? LXP_OK : LXP_FATAL_INVARIANT;
    }
    if (lxp_u128_cmp(top, divisor) >= 0) {
        return status == LXP_ERR_OVERFLOW && u128_equal(quotient, sentinel_q) &&
               u128_equal(remainder, sentinel_r) ? LXP_OK : LXP_FATAL_INVARIANT;
    }
    if (status != LXP_OK || lxp_u128_cmp(remainder, divisor) >= 0)
        return LXP_FATAL_INVARIANT;
    {
        lxp_u256 reconstructed = reference_mul(quotient, divisor);
        lxp_u256 residue = {{ remainder.lo, remainder.hi, 0U, 0U }};
        if (lxp_u256_add(reconstructed, residue, &reconstructed) != LXP_OK ||
            !u256_equal(reconstructed, dividend)) return LXP_FATAL_INVARIANT;
    }
    return LXP_OK;
}

lxp_result lxp_u256_boundary_case(uint64_t *case_count)
{
    size_t count_words = sizeof(boundary_words) / sizeof(boundary_words[0]);
    uint64_t count = 0U;
    size_t a;
    size_t b;
    size_t c;
    if (case_count == NULL) return LXP_ERR_NON_CANONICAL;
    for (a = 0U; a < count_words; ++a) {
        lxp_u256 left = boundary_pattern(a);
        for (b = 0U; b < count_words; ++b) {
            lxp_u256 right = boundary_pattern(b);
            lxp_u256 out = {{ UINT64_C(7), UINT64_C(8), UINT64_C(9),
                              UINT64_C(10) }};
            lxp_u256 expected;
            uint64_t carry = 0U;
            size_t word;
            lxp_result expected_status = LXP_OK;
            for (word = 0U; word < 4U; ++word) {
                __uint128_t sum = (__uint128_t)left.words[word] +
                                  (__uint128_t)right.words[word] + carry;
                expected.words[word] = (uint64_t)sum;
                carry = (uint64_t)(sum >> 64U);
            }
            if (carry != 0U) expected_status = LXP_ERR_OVERFLOW;
            if (lxp_u256_add(left, right, &out) != expected_status)
                return LXP_FATAL_INVARIANT;
            if (expected_status == LXP_OK && !u256_equal(out, expected))
                return LXP_FATAL_INVARIANT;
            if (expected_status != LXP_OK &&
                !u256_equal(out, (lxp_u256){{ 7U, 8U, 9U, 10U }}))
                return LXP_FATAL_INVARIANT;
            ++count;
        }
        for (b = 0U; b < count_words; ++b) {
            for (c = 0U; c < count_words; ++c) {
                lxp_u128 divisor = { boundary_words[b], boundary_words[c] };
                lxp_result status = division_invariant(left, divisor);
                if (status != LXP_OK) return status;
                ++count;
            }
        }
    }
    *case_count = count;
    return LXP_OK;
}

static lxp_result check_rounding(lxp_u128 value, uint32_t basis_points)
{
    const lxp_u128 sentinel = { UINT64_C(0x13579bdf2468ace0),
                                UINT64_C(0xfdb97531eca86420) };
    lxp_u128 multiplier = { 0U, (uint64_t)basis_points };
    lxp_u128 divisor = { 0U, LXP_BASIS_POINTS_ONE };
    lxp_u128 quotient = sentinel;
    lxp_u128 remainder = sentinel;
    lxp_u128 floor_value = sentinel;
    lxp_u128 ceil_value = sentinel;
    lxp_u128 expected_ceil;
    lxp_u256 product;
    lxp_u256 reconstructed;
    lxp_u256 residue;
    lxp_result status = lxp_u128_mul_div_floor(value, multiplier, divisor,
                                               &quotient, &remainder);
    if (status == LXP_ERR_OVERFLOW) {
        lxp_result floor_status = lxp_u128_mul_bps_floor(
            value, basis_points, &floor_value);
        lxp_result ceil_status = lxp_u128_mul_bps_ceil(
            value, basis_points, &ceil_value);
        return floor_status == LXP_ERR_OVERFLOW &&
               ceil_status == LXP_ERR_OVERFLOW &&
               u128_equal(floor_value, sentinel) &&
               u128_equal(ceil_value, sentinel) ? LXP_OK :
               LXP_FATAL_INVARIANT;
    }
    if (status != LXP_OK) return status;
    if (lxp_u128_mul_bps_floor(value, basis_points, &floor_value) != LXP_OK ||
        !u128_equal(floor_value, quotient)) return LXP_FATAL_INVARIANT;
    expected_ceil = quotient;
    if (!lxp_u128_is_zero(remainder) &&
        lxp_u128_add(expected_ceil, (lxp_u128){ 0U, 1U }, &expected_ceil) !=
            LXP_OK) return LXP_ERR_OVERFLOW;
    if (lxp_u128_mul_bps_ceil(value, basis_points, &ceil_value) != LXP_OK ||
        !u128_equal(ceil_value, expected_ceil) ||
        lxp_u128_cmp(remainder, divisor) >= 0)
        return LXP_FATAL_INVARIANT;
    if (lxp_u128_mul(value, multiplier, &product) != LXP_OK ||
        lxp_u128_mul(quotient, divisor, &reconstructed) != LXP_OK)
        return LXP_FATAL_INVARIANT;
    residue = (lxp_u256){{ remainder.lo, remainder.hi, 0U, 0U }};
    if (lxp_u256_add(reconstructed, residue, &reconstructed) != LXP_OK ||
        !u256_equal(product, reconstructed)) return LXP_FATAL_INVARIANT;
    return LXP_OK;
}

lxp_result lxp_rounding_direction_check(uint64_t *case_count)
{
    static const uint32_t bps_boundaries[] = {
        0U, 1U, 2U, 9999U, 10000U, 10001U, UINT32_MAX - 1U, UINT32_MAX
    };
    size_t count_words = sizeof(boundary_words) / sizeof(boundary_words[0]);
    uint64_t count = 0U;
    size_t a;
    size_t b;
    size_t c;
    if (case_count == NULL) return LXP_ERR_NON_CANONICAL;
    for (a = 0U; a < count_words; ++a) {
        for (b = 0U; b < count_words; ++b) {
            lxp_u128 value = { boundary_words[a], boundary_words[b] };
            for (c = 0U;
                 c < sizeof(bps_boundaries) / sizeof(bps_boundaries[0]); ++c) {
                lxp_result status = check_rounding(value, bps_boundaries[c]);
                if (status != LXP_OK) return status;
                ++count;
            }
        }
    }
    for (a = 0U; a <= 16U; ++a) {
        uint32_t basis_points;
        for (basis_points = 0U; basis_points <= LXP_BASIS_POINTS_ONE;
             ++basis_points) {
            lxp_result status = check_rounding((lxp_u128){ 0U, a },
                                               basis_points);
            if (status != LXP_OK) return status;
            ++count;
        }
    }
    *case_count = count;
    return LXP_OK;
}

#ifdef __CPROVER
extern uint64_t nondet_uint64_t(void);
extern uint32_t nondet_uint32_t(void);

void lxp_cbmc_u128_add_sub(void)
{
    lxp_u128 left = { nondet_uint64_t(), nondet_uint64_t() };
    lxp_u128 right = { nondet_uint64_t(), nondet_uint64_t() };
    lxp_u128 sentinel = { nondet_uint64_t(), nondet_uint64_t() };
    lxp_u128 out = sentinel;
    __uint128_t a = native_u128(left);
    __uint128_t b = native_u128(right);
    __uint128_t maximum = ~(__uint128_t)0U;
    lxp_result status = lxp_u128_add(left, right, &out);
    __CPROVER_assert(status == (b > maximum - a ? LXP_ERR_OVERFLOW : LXP_OK),
                     "u128 add reports exact status");
    __CPROVER_assert(status == LXP_OK ? u128_equal(out, split_u128(a + b)) :
                     u128_equal(out, sentinel),
                     "u128 add is exact or leaves output unchanged");
    out = sentinel;
    status = lxp_u128_sub(left, right, &out);
    __CPROVER_assert(status == (a < b ? LXP_ERR_UNDERFLOW : LXP_OK),
                     "u128 sub reports exact status");
    __CPROVER_assert(status == LXP_OK ? u128_equal(out, split_u128(a - b)) :
                     u128_equal(out, sentinel),
                     "u128 sub is exact or leaves output unchanged");
}

void lxp_cbmc_u128_mul(void)
{
    lxp_u128 left = { nondet_uint64_t(), nondet_uint64_t() };
    lxp_u128 right = { nondet_uint64_t(), nondet_uint64_t() };
    lxp_u256 out;
    __CPROVER_assert(lxp_u128_mul(left, right, &out) == LXP_OK,
                     "u128 widening multiply cannot overflow");
    __CPROVER_assert(u256_equal(out, reference_mul(left, right)),
                     "u128 widening multiply is exact");
}

void lxp_cbmc_u256_add(void)
{
    lxp_u256 left;
    lxp_u256 right;
    lxp_u256 expected;
    lxp_u256 sentinel;
    lxp_u256 out;
    uint64_t carry = 0U;
    size_t word;
    for (word = 0U; word < 4U; ++word) {
        left.words[word] = nondet_uint64_t();
        right.words[word] = nondet_uint64_t();
        sentinel.words[word] = nondet_uint64_t();
    }
    out = sentinel;
    for (word = 0U; word < 4U; ++word) {
        __uint128_t sum = (__uint128_t)left.words[word] + right.words[word] +
                          carry;
        expected.words[word] = (uint64_t)sum;
        carry = (uint64_t)(sum >> 64U);
    }
    {
        lxp_result status = lxp_u256_add(left, right, &out);
        __CPROVER_assert(status == (carry != 0U ? LXP_ERR_OVERFLOW : LXP_OK),
                         "u256 add reports exact status");
        __CPROVER_assert(status == LXP_OK ? u256_equal(out, expected) :
                         u256_equal(out, sentinel),
                         "u256 add is exact or leaves output unchanged");
    }
}

void lxp_cbmc_u256_div_floor(void)
{
    lxp_u256 dividend;
    lxp_u128 divisor = { nondet_uint64_t(), nondet_uint64_t() };
    lxp_u128 quotient = { nondet_uint64_t(), nondet_uint64_t() };
    lxp_u128 remainder = { nondet_uint64_t(), nondet_uint64_t() };
    lxp_u128 original_q = quotient;
    lxp_u128 original_r = remainder;
    lxp_u128 top;
    lxp_result status;
    size_t word;
    for (word = 0U; word < 4U; ++word)
        dividend.words[word] = nondet_uint64_t();
    top = (lxp_u128){ dividend.words[3], dividend.words[2] };
    status = lxp_u256_div_floor(dividend, divisor, &quotient, &remainder);
    if (lxp_u128_is_zero(divisor)) {
        __CPROVER_assert(status == LXP_ERR_DIV_ZERO,
                         "u256 division rejects zero divisor");
        __CPROVER_assert(u128_equal(quotient, original_q) &&
                         u128_equal(remainder, original_r),
                         "zero-divisor failure leaves outputs unchanged");
    } else if (lxp_u128_cmp(top, divisor) >= 0) {
        __CPROVER_assert(status == LXP_ERR_OVERFLOW,
                         "u256 division reports quotient overflow");
        __CPROVER_assert(u128_equal(quotient, original_q) &&
                         u128_equal(remainder, original_r),
                         "overflow failure leaves outputs unchanged");
    } else {
        lxp_u256 reconstructed;
        lxp_u256 residue;
        __CPROVER_assert(status == LXP_OK,
                         "representable u256 division succeeds");
        __CPROVER_assert(lxp_u128_cmp(remainder, divisor) < 0,
                         "division remainder is less than divisor");
        (void)lxp_u128_mul(quotient, divisor, &reconstructed);
        residue = (lxp_u256){{ remainder.lo, remainder.hi, 0U, 0U }};
        __CPROVER_assert(lxp_u256_add(reconstructed, residue,
                                      &reconstructed) == LXP_OK &&
                         u256_equal(reconstructed, dividend),
                         "quotient times divisor plus remainder is dividend");
    }
}

void lxp_cbmc_rounding(void)
{
    lxp_u128 value = { nondet_uint64_t(), nondet_uint64_t() };
    uint32_t basis_points = nondet_uint32_t();
    __CPROVER_assert(check_rounding(value, basis_points) == LXP_OK,
                     "floor and ceiling directions conserve exact product");
}
#endif
