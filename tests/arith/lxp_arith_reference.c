#include "lxp_arith_reference.h"

#include <stdint.h>
#include <string.h>

#define REF_DIGITS 160U

typedef struct decimal_value {
    uint8_t digit[REF_DIGITS];
    size_t length;
} decimal_value;

static void normalize(decimal_value *value)
{
    while (value->length > 1U && value->digit[value->length - 1U] == 0U)
        --value->length;
}

static lxp_result parse(const char *text, decimal_value *value)
{
    size_t length;
    size_t i;
    if (text == NULL || value == NULL) return LXP_ERR_NON_CANONICAL;
    length = strlen(text);
    if (length == 0U || length > REF_DIGITS) return LXP_ERR_LENGTH_LIMIT;
    value->length = length;
    for (i = 0U; i < length; ++i) {
        char character = text[length - 1U - i];
        if (character < '0' || character > '9') return LXP_ERR_NON_CANONICAL;
        value->digit[i] = (uint8_t)(character - '0');
    }
    normalize(value);
    return LXP_OK;
}

static int compare(const decimal_value *left, const decimal_value *right)
{
    size_t i;
    if (left->length < right->length) return -1;
    if (left->length > right->length) return 1;
    for (i = left->length; i-- > 0U;) {
        if (left->digit[i] < right->digit[i]) return -1;
        if (left->digit[i] > right->digit[i]) return 1;
    }
    return 0;
}

static lxp_result add(const decimal_value *left, const decimal_value *right,
                      decimal_value *out)
{
    size_t length = left->length > right->length ? left->length : right->length;
    size_t i;
    uint16_t carry = 0U;
    if (length + 1U > REF_DIGITS) return LXP_ERR_OVERFLOW;
    for (i = 0U; i < length; ++i) {
        uint16_t sum = carry;
        if (i < left->length) sum = (uint16_t)(sum + left->digit[i]);
        if (i < right->length) sum = (uint16_t)(sum + right->digit[i]);
        out->digit[i] = (uint8_t)(sum % 10U);
        carry = (uint16_t)(sum / 10U);
    }
    out->length = length;
    if (carry != 0U) out->digit[out->length++] = (uint8_t)carry;
    normalize(out);
    return LXP_OK;
}

static lxp_result subtract(const decimal_value *left,
                           const decimal_value *right, decimal_value *out)
{
    size_t i;
    int16_t borrow = 0;
    if (compare(left, right) < 0) return LXP_ERR_UNDERFLOW;
    out->length = left->length;
    for (i = 0U; i < left->length; ++i) {
        int16_t difference = (int16_t)left->digit[i] - borrow;
        if (i < right->length) difference -= (int16_t)right->digit[i];
        if (difference < 0) {
            difference += 10;
            borrow = 1;
        } else borrow = 0;
        out->digit[i] = (uint8_t)difference;
    }
    normalize(out);
    return LXP_OK;
}

static lxp_result multiply(const decimal_value *left,
                           const decimal_value *right, decimal_value *out)
{
    size_t i;
    (void)memset(out, 0, sizeof(*out));
    if (left->length + right->length > REF_DIGITS + 1U)
        return LXP_ERR_OVERFLOW;
    out->length = left->length + right->length;
    if (out->length > REF_DIGITS) out->length = REF_DIGITS;
    for (i = 0U; i < left->length; ++i) {
        size_t j;
        uint16_t carry = 0U;
        for (j = 0U; j < right->length; ++j) {
            size_t position = i + j;
            uint16_t product;
            if (position >= REF_DIGITS) return LXP_ERR_OVERFLOW;
            product = (uint16_t)((uint16_t)out->digit[position] +
                      (uint16_t)left->digit[i] * (uint16_t)right->digit[j] +
                      carry);
            out->digit[position] = (uint8_t)(product % 10U);
            carry = (uint16_t)(product / 10U);
        }
        for (j = i + right->length; carry != 0U; ++j) {
            uint16_t sum;
            if (j >= REF_DIGITS) return LXP_ERR_OVERFLOW;
            sum = (uint16_t)out->digit[j] + carry;
            out->digit[j] = (uint8_t)(sum % 10U);
            carry = (uint16_t)(sum / 10U);
        }
    }
    normalize(out);
    return LXP_OK;
}

static lxp_result append_digit(decimal_value *value, uint8_t digit)
{
    size_t i;
    if (value->length == 1U && value->digit[0] == 0U) {
        value->digit[0] = digit;
        return LXP_OK;
    }
    if (value->length == REF_DIGITS) return LXP_ERR_OVERFLOW;
    for (i = value->length; i > 0U; --i) value->digit[i] = value->digit[i - 1U];
    value->digit[0] = digit;
    ++value->length;
    normalize(value);
    return LXP_OK;
}

static lxp_result divide_floor(const decimal_value *dividend,
                               const decimal_value *divisor,
                               decimal_value *quotient,
                               decimal_value *remainder)
{
    size_t i;
    decimal_value next;
    if (divisor->length == 1U && divisor->digit[0] == 0U)
        return LXP_ERR_DIV_ZERO;
    (void)memset(quotient, 0, sizeof(*quotient));
    quotient->length = dividend->length;
    (void)memset(remainder, 0, sizeof(*remainder));
    remainder->length = 1U;
    for (i = dividend->length; i-- > 0U;) {
        uint8_t count = 0U;
        lxp_result status = append_digit(remainder, dividend->digit[i]);
        if (status != LXP_OK) return status;
        while (compare(remainder, divisor) >= 0) {
            status = subtract(remainder, divisor, &next);
            if (status != LXP_OK) return status;
            *remainder = next;
            ++count;
        }
        quotient->digit[i] = count;
    }
    normalize(quotient);
    normalize(remainder);
    return LXP_OK;
}

static lxp_result format(const decimal_value *value, char *text,
                         size_t capacity)
{
    size_t i;
    if (text == NULL || capacity <= value->length) return LXP_ERR_LENGTH_LIMIT;
    for (i = 0U; i < value->length; ++i)
        text[i] = (char)('0' + value->digit[value->length - 1U - i]);
    text[value->length] = '\0';
    return LXP_OK;
}

lxp_result lxp_arith_reference_apply(lxp_arith_reference_op operation,
                                     const char *left_text,
                                     const char *right_text, char *result,
                                     size_t result_capacity, char *remainder,
                                     size_t remainder_capacity)
{
    decimal_value left;
    decimal_value right;
    decimal_value output;
    decimal_value residue = {{ 0U }, 1U};
    lxp_result status = parse(left_text, &left);
    if (status != LXP_OK) return status;
    status = parse(right_text, &right);
    if (status != LXP_OK) return status;
    if (operation == LXP_REF_ADD) status = add(&left, &right, &output);
    else if (operation == LXP_REF_SUB) status = subtract(&left, &right, &output);
    else if (operation == LXP_REF_MUL) status = multiply(&left, &right, &output);
    else if (operation == LXP_REF_DIV_FLOOR)
        status = divide_floor(&left, &right, &output, &residue);
    else return LXP_ERR_INVALID_TAG;
    if (status != LXP_OK) return status;
    status = format(&output, result, result_capacity);
    if (status != LXP_OK) return status;
    if (remainder != NULL)
        return format(&residue, remainder, remainder_capacity);
    return LXP_OK;
}
