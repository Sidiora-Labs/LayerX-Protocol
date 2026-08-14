#include "lxp_arith_reference.h"
#include "lxp_test_arith_property.h"

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define DECIMAL_CAPACITY 82U
#define U128_MAX_DECIMAL "340282366920938463463374607431768211455"

int lxp_fuzz_arith(const uint8_t *data, size_t size);

static void decimal_double(char value[DECIMAL_CAPACITY])
{
    size_t length = strlen(value);
    size_t i;
    unsigned int carry = 0U;
    for (i = length; i-- > 0U;) {
        unsigned int digit = (unsigned int)(value[i] - '0') * 2U + carry;
        value[i] = (char)('0' + digit % 10U);
        carry = digit / 10U;
    }
    if (carry != 0U) {
        (void)memmove(value + 1U, value, length + 1U);
        value[0] = (char)('0' + carry);
    }
}

static void decimal_increment(char value[DECIMAL_CAPACITY])
{
    size_t i = strlen(value);
    while (i > 0U) {
        --i;
        if (value[i] != '9') {
            value[i] = (char)(value[i] + 1);
            return;
        }
        value[i] = '0';
    }
    {
        size_t length = strlen(value);
        (void)memmove(value + 1U, value, length + 1U);
        value[0] = '1';
    }
}

static void bits_to_decimal(const uint64_t *words, size_t word_count,
                            char output[DECIMAL_CAPACITY])
{
    size_t bit = word_count * 64U;
    (void)strcpy(output, "0");
    while (bit-- > 0U) {
        decimal_double(output);
        if (((words[bit / 64U] >> (bit % 64U)) & UINT64_C(1)) != 0U)
            decimal_increment(output);
    }
}

static void u128_to_decimal(lxp_u128 value, char output[DECIMAL_CAPACITY])
{
    uint64_t words[2] = { value.lo, value.hi };
    bits_to_decimal(words, 2U, output);
}

static int exceeds_u128(const char *value)
{
    size_t length = strlen(value);
    size_t max_length = sizeof(U128_MAX_DECIMAL) - 1U;
    return length > max_length ||
           (length == max_length && strcmp(value, U128_MAX_DECIMAL) > 0);
}

int lxp_test_arith_property(lxp_u128 left, lxp_u128 right,
                            uint32_t basis_points)
{
    char left_decimal[DECIMAL_CAPACITY];
    char right_decimal[DECIMAL_CAPACITY];
    char actual[DECIMAL_CAPACITY];
    char expected[DECIMAL_CAPACITY];
    char residue[DECIMAL_CAPACITY];
    char basis_decimal[DECIMAL_CAPACITY];
    lxp_u128 value;
    lxp_u128 remainder;
    lxp_u256 wide;
    lxp_result status;
    lxp_result expected_status;
    uint64_t bps_word = basis_points;

    u128_to_decimal(left, left_decimal);
    u128_to_decimal(right, right_decimal);
    bits_to_decimal(&bps_word, 1U, basis_decimal);

    expected_status = lxp_arith_reference_apply(
        LXP_REF_ADD, left_decimal, right_decimal, expected, sizeof(expected),
        NULL, 0U);
    if (expected_status != LXP_OK) return 0;
    if (exceeds_u128(expected)) expected_status = LXP_ERR_OVERFLOW;
    status = lxp_u128_add(left, right, &value);
    if (status != expected_status) return 0;
    if (status == LXP_OK) {
        u128_to_decimal(value, actual);
        if (strcmp(actual, expected) != 0) return 0;
    }

    expected_status = lxp_arith_reference_apply(
        LXP_REF_SUB, left_decimal, right_decimal, expected, sizeof(expected),
        NULL, 0U);
    status = lxp_u128_sub(left, right, &value);
    if (status != expected_status) return 0;
    if (status == LXP_OK) {
        u128_to_decimal(value, actual);
        if (strcmp(actual, expected) != 0) return 0;
    }

    if (lxp_arith_reference_apply(LXP_REF_MUL, left_decimal, right_decimal,
                                  expected, sizeof(expected), NULL, 0U) !=
        LXP_OK || lxp_u128_mul(left, right, &wide) != LXP_OK) return 0;
    bits_to_decimal(wide.words, 4U, actual);
    if (strcmp(actual, expected) != 0) return 0;

    expected_status = lxp_arith_reference_apply(
        LXP_REF_DIV_FLOOR, left_decimal, right_decimal, expected,
        sizeof(expected), residue, sizeof(residue));
    wide = (lxp_u256){{ left.lo, left.hi, 0U, 0U }};
    status = lxp_u256_div_floor(wide, right, &value, &remainder);
    if (status != expected_status) return 0;
    if (status == LXP_OK) {
        u128_to_decimal(value, actual);
        if (strcmp(actual, expected) != 0) return 0;
        u128_to_decimal(remainder, actual);
        if (strcmp(actual, residue) != 0) return 0;
    }

    if (lxp_arith_reference_apply(LXP_REF_MUL, left_decimal, basis_decimal,
                                  actual, sizeof(actual), NULL, 0U) != LXP_OK ||
        lxp_arith_reference_apply(LXP_REF_DIV_FLOOR, actual, "10000",
                                  expected, sizeof(expected), residue,
                                  sizeof(residue)) != LXP_OK) return 0;
    status = lxp_u128_mul_bps_floor(left, basis_points, &value);
    if (exceeds_u128(expected)) {
        if (status != LXP_ERR_OVERFLOW) return 0;
    } else {
        if (status != LXP_OK) return 0;
        u128_to_decimal(value, actual);
        if (strcmp(actual, expected) != 0) return 0;
    }
    return 1;
}

static uint64_t next_random(uint64_t *state)
{
    uint64_t value = *state;
    value ^= value << 13U;
    value ^= value >> 7U;
    value ^= value << 17U;
    *state = value;
    return value;
}

int main(void)
{
    static const uint64_t boundary[] = {
        0U, 1U, UINT32_MAX - 1U, UINT32_MAX, UINT64_C(0x100000000),
        UINT64_C(0x100000001), UINT64_MAX - 1U, UINT64_MAX
    };
    uint64_t seed = UINT64_C(0x4c58502d61726974);
    size_t i;
    size_t j;
    for (i = 0U; i < sizeof(boundary) / sizeof(boundary[0]); ++i) {
        for (j = 0U; j < sizeof(boundary) / sizeof(boundary[0]); ++j) {
            lxp_u128 left = { boundary[i], boundary[j] };
            lxp_u128 right = { boundary[j], boundary[i] };
            if (!lxp_test_arith_property(left, right,
                                         (uint32_t)boundary[i])) {
                (void)fprintf(stderr, "boundary property failed at %zu,%zu\n", i, j);
                return 1;
            }
        }
    }
    for (i = 0U; i < 20000U; ++i) {
        lxp_u128 left = { next_random(&seed), next_random(&seed) };
        lxp_u128 right = { next_random(&seed), next_random(&seed) };
        if (!lxp_test_arith_property(left, right,
                                     (uint32_t)next_random(&seed))) {
            (void)fprintf(stderr, "random property failed at %zu\n", i);
            return 1;
        }
    }
    {
        uint8_t bytes[36] = { 0U };
        for (i = 0U; i < sizeof(bytes); ++i) bytes[i] = (uint8_t)i;
        if (lxp_fuzz_arith(bytes, sizeof(bytes)) != 0) return 1;
    }
    return 0;
}
