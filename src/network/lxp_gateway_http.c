#include "layerx/lxp_gateway.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_hash.h"

#include <limits.h>
#include <string.h>

static void store_u32(uint8_t bytes[4], uint32_t value)
{
    bytes[0] = (uint8_t)(value >> 24U);
    bytes[1] = (uint8_t)(value >> 16U);
    bytes[2] = (uint8_t)(value >> 8U);
    bytes[3] = (uint8_t)value;
}

static void store_u64(uint8_t bytes[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i)
        bytes[7U - i] = (uint8_t)(value >> (i * 8U));
}

lxp_result lxp_payment_requirement_encode(
    const lxp_payment_requirement *requirement,
    bool include_signature,
    uint8_t *bytes,
    size_t capacity,
    size_t *length)
{
    size_t required = include_signature ?
        LXP_PAYMENT_REQUIREMENT_ENCODED_SIZE :
        LXP_PAYMENT_REQUIREMENT_PREIMAGE_SIZE;
    size_t cursor = 0U;
    if (requirement == NULL || bytes == NULL || length == NULL ||
        capacity < required) return LXP_ERR_LENGTH_LIMIT;
    store_u32(bytes + cursor, requirement->network_id);
    cursor += 4U;
    (void)memcpy(bytes + cursor, requirement->recipient, 32U);
    cursor += 32U;
    (void)memcpy(bytes + cursor, requirement->asset, 32U);
    cursor += 32U;
    if (lxp_u128_to_be(requirement->amount, bytes + cursor) != LXP_OK)
        return LXP_ERR_NON_CANONICAL;
    cursor += 16U;
    (void)memcpy(bytes + cursor, requirement->invoice_id, 32U);
    cursor += 32U;
    (void)memcpy(bytes + cursor, requirement->purpose_hash, 32U);
    cursor += 32U;
    store_u64(bytes + cursor, requirement->expiry);
    cursor += 8U;
    store_u32(bytes + cursor, requirement->acceptable_conditions);
    cursor += 4U;
    if (include_signature) {
        (void)memcpy(bytes + cursor, requirement->service_signature, 64U);
        cursor += 64U;
    }
    *length = cursor;
    return LXP_OK;
}

lxp_result lxp_payment_requirement_verify(
    const lxp_payment_requirement *requirement,
    uint32_t executing_network_id,
    const uint8_t service_public_key[32])
{
    uint8_t preimage[LXP_PAYMENT_REQUIREMENT_PREIMAGE_SIZE];
    size_t length = 0U;
    lxp_result status;
    if (requirement == NULL || service_public_key == NULL ||
        requirement->network_id == 0U ||
        requirement->network_id != executing_network_id ||
        lxp_ct_is_zero(requirement->recipient, 32U) ||
        lxp_ct_is_zero(requirement->asset, 32U) ||
        lxp_ct_is_zero(requirement->invoice_id, 32U) ||
        lxp_ct_is_zero(requirement->purpose_hash, 32U) ||
        requirement->expiry == 0U ||
        requirement->acceptable_conditions == 0U ||
        lxp_ct_is_zero(requirement->service_signature, 64U))
        return LXP_ERR_NON_CANONICAL;
    status = lxp_payment_requirement_encode(
        requirement, false, preimage, sizeof(preimage), &length);
    if (status != LXP_OK || length != sizeof(preimage)) return status;
    return lxp_ed25519_verify_raw(
        service_public_key, requirement->service_signature,
        preimage, sizeof(preimage));
}

static bool take_literal(
    const uint8_t *json, size_t length, size_t *cursor, const char *literal)
{
    size_t literal_length = strlen(literal);
    if (*cursor > length || literal_length > length - *cursor ||
        memcmp(json + *cursor, literal, literal_length) != 0) return false;
    *cursor += literal_length;
    return true;
}

static int hex_nibble(uint8_t byte)
{
    if (byte >= (uint8_t)'0' && byte <= (uint8_t)'9')
        return (int)(byte - (uint8_t)'0');
    if (byte >= (uint8_t)'a' && byte <= (uint8_t)'f')
        return 10 + (int)(byte - (uint8_t)'a');
    return -1;
}

static bool take_hex(
    const uint8_t *json, size_t length, size_t *cursor,
    uint8_t *output, size_t output_length)
{
    size_t i;
    if (*cursor > length || output_length > (length - *cursor) / 2U)
        return false;
    for (i = 0U; i < output_length; ++i) {
        int high = hex_nibble(json[*cursor + i * 2U]);
        int low = hex_nibble(json[*cursor + i * 2U + 1U]);
        if (high < 0 || low < 0) return false;
        output[i] = (uint8_t)((unsigned)high << 4U) | (uint8_t)low;
    }
    *cursor += output_length * 2U;
    return true;
}

static bool take_u64(
    const uint8_t *json, size_t length, size_t *cursor, uint64_t *value)
{
    uint64_t result = 0U;
    size_t start = *cursor;
    if (start >= length || json[start] < (uint8_t)'0' ||
        json[start] > (uint8_t)'9') return false;
    if (json[start] == (uint8_t)'0' && start + 1U < length &&
        json[start + 1U] >= (uint8_t)'0' &&
        json[start + 1U] <= (uint8_t)'9') return false;
    while (*cursor < length && json[*cursor] >= (uint8_t)'0' &&
           json[*cursor] <= (uint8_t)'9') {
        uint64_t digit = (uint64_t)(json[*cursor] - (uint8_t)'0');
        if (result > (UINT64_MAX - digit) / 10U) return false;
        result = result * 10U + digit;
        ++*cursor;
    }
    *value = result;
    return *cursor != start;
}

static bool take_u128(
    const uint8_t *json, size_t length, size_t *cursor, lxp_u128 *value)
{
    lxp_u128 result = {0U, 0U};
    size_t start = *cursor;
    if (start >= length || json[start] < (uint8_t)'0' ||
        json[start] > (uint8_t)'9') return false;
    if (json[start] == (uint8_t)'0' && start + 1U < length &&
        json[start + 1U] >= (uint8_t)'0' &&
        json[start + 1U] <= (uint8_t)'9') return false;
    while (*cursor < length && json[*cursor] >= (uint8_t)'0' &&
           json[*cursor] <= (uint8_t)'9') {
        lxp_u256 product;
        lxp_u128 next;
        lxp_u128 digit = {0U,
            (uint64_t)(json[*cursor] - (uint8_t)'0')};
        if (lxp_u128_mul(result, (lxp_u128){0U, 10U}, &product) != LXP_OK ||
            product.words[2] != 0U || product.words[3] != 0U)
            return false;
        next = (lxp_u128){product.words[1], product.words[0]};
        if (lxp_u128_add(next, digit, &result) != LXP_OK) return false;
        ++*cursor;
    }
    *value = result;
    return *cursor != start;
}

lxp_result lxp_gateway_translate(
    const uint8_t *json,
    size_t json_length,
    lxp_payment_requirement *requirement,
    uint8_t canonical[LXP_PAYMENT_REQUIREMENT_ENCODED_SIZE],
    size_t *canonical_length)
{
    uint64_t number;
    size_t cursor = 0U;
    if (json == NULL || requirement == NULL || canonical == NULL ||
        canonical_length == NULL || json_length == 0U)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(requirement, 0, sizeof(*requirement));
#define TAKE(text) do { \
    if (!take_literal(json, json_length, &cursor, text)) \
        return LXP_ERR_NON_CANONICAL; \
} while (0)
    TAKE("{\"network_id\":");
    if (!take_u64(json, json_length, &cursor, &number) ||
        number == 0U || number > UINT32_MAX)
        return LXP_ERR_NON_CANONICAL;
    requirement->network_id = (uint32_t)number;
    TAKE(",\"recipient\":\"");
    if (!take_hex(json, json_length, &cursor,
                  requirement->recipient, 32U)) return LXP_ERR_NON_CANONICAL;
    TAKE("\",\"asset\":\"");
    if (!take_hex(json, json_length, &cursor,
                  requirement->asset, 32U)) return LXP_ERR_NON_CANONICAL;
    TAKE("\",\"amount\":\"");
    if (!take_u128(json, json_length, &cursor, &requirement->amount))
        return LXP_ERR_NON_CANONICAL;
    TAKE("\",\"invoice_id\":\"");
    if (!take_hex(json, json_length, &cursor,
                  requirement->invoice_id, 32U)) return LXP_ERR_NON_CANONICAL;
    TAKE("\",\"purpose_hash\":\"");
    if (!take_hex(json, json_length, &cursor,
                  requirement->purpose_hash, 32U)) return LXP_ERR_NON_CANONICAL;
    TAKE("\",\"expiry\":");
    if (!take_u64(json, json_length, &cursor, &requirement->expiry) ||
        requirement->expiry == 0U) return LXP_ERR_NON_CANONICAL;
    TAKE(",\"acceptable_conditions\":");
    if (!take_u64(json, json_length, &cursor, &number) ||
        number == 0U || number > UINT32_MAX)
        return LXP_ERR_NON_CANONICAL;
    requirement->acceptable_conditions = (uint32_t)number;
    TAKE(",\"service_signature\":\"");
    if (!take_hex(json, json_length, &cursor,
                  requirement->service_signature, 64U))
        return LXP_ERR_NON_CANONICAL;
    TAKE("\"}");
#undef TAKE
    if (cursor != json_length) return LXP_ERR_NON_CANONICAL;
    return lxp_payment_requirement_encode(
        requirement, true, canonical,
        LXP_PAYMENT_REQUIREMENT_ENCODED_SIZE, canonical_length);
}
