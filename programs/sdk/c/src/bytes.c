#include "layerx/program.h"

#include "internal.h"

void lxp_program_write_u16_be(uint8_t *out, uint16_t value)
{
    if (out == NULL) return;
    out[0] = (uint8_t)((value >> 8) & 0xFFU);
    out[1] = (uint8_t)(value & 0xFFU);
}

void lxp_program_write_u32_be(uint8_t *out, uint32_t value)
{
    if (out == NULL) return;
    out[0] = (uint8_t)((value >> 24) & 0xFFU);
    out[1] = (uint8_t)((value >> 16) & 0xFFU);
    out[2] = (uint8_t)((value >> 8) & 0xFFU);
    out[3] = (uint8_t)(value & 0xFFU);
}

void lxp_program_write_u64_be(uint8_t *out, uint64_t value)
{
    size_t index;
    if (out == NULL) return;
    for (index = 0U; index < 8U; ++index)
        out[index] = (uint8_t)((value >> (56U - (index * 8U))) & 0xFFU);
}

uint16_t lxp_program_read_u16_be(const uint8_t *bytes)
{
    if (bytes == NULL) return 0U;
    return (uint16_t)(((uint16_t)bytes[0] << 8) | (uint16_t)bytes[1]);
}

uint32_t lxp_program_read_u32_be(const uint8_t *bytes)
{
    if (bytes == NULL) return 0U;
    return ((uint32_t)bytes[0] << 24) | ((uint32_t)bytes[1] << 16) |
           ((uint32_t)bytes[2] << 8) | (uint32_t)bytes[3];
}

uint64_t lxp_program_read_u64_be(const uint8_t *bytes)
{
    uint64_t value = 0U;
    size_t index;
    if (bytes == NULL) return 0U;
    for (index = 0U; index < 8U; ++index)
        value = (value << 8) | (uint64_t)bytes[index];
    return value;
}

int32_t lxp_program_read_i32_be(const uint8_t *bytes)
{
    uint32_t unsigned_value = lxp_program_read_u32_be(bytes);
    if (unsigned_value <= (uint32_t)INT32_MAX) return (int32_t)unsigned_value;
    return (int32_t)(unsigned_value - (uint32_t)INT32_MAX - 1U) + INT32_MIN;
}

void lxp_program_copy(uint8_t *destination, const uint8_t *source,
                      size_t length)
{
    size_t index;
    if (destination == NULL || source == NULL) return;
    for (index = 0U; index < length; ++index) destination[index] = source[index];
}

bool lxp_program_bytes_equal(const uint8_t *left, const uint8_t *right,
                             size_t length)
{
    return lxp_program_bytes_compare(left, right, length) == 0;
}

int lxp_program_bytes_compare(const uint8_t *left, const uint8_t *right,
                              size_t length)
{
    size_t index;
    if (left == NULL || right == NULL) return left == right ? 0 : (left == NULL ? -1 : 1);
    for (index = 0U; index < length; ++index) {
        if (left[index] != right[index])
            return left[index] < right[index] ? -1 : 1;
    }
    return 0;
}

bool lxp_program_bytes32_is_zero(const uint8_t bytes[32])
{
    size_t index;
    if (bytes == NULL) return true;
    for (index = 0U; index < 32U; ++index)
        if (bytes[index] != 0U) return false;
    return true;
}

void lxp_program_bytes32_from_words(uint64_t word0, uint64_t word1,
                                    uint64_t word2, uint64_t word3,
                                    uint8_t out[32])
{
    if (out == NULL) return;
    lxp_program_write_u64_be(out, word0);
    lxp_program_write_u64_be(out + 8, word1);
    lxp_program_write_u64_be(out + 16, word2);
    lxp_program_write_u64_be(out + 24, word3);
}

lxp_program_id lxp_program_id_from_words(uint64_t word0, uint64_t word1,
                                         uint64_t word2, uint64_t word3)
{
    lxp_program_id value;
    lxp_program_bytes32_from_words(word0, word1, word2, word3, value.bytes);
    return value;
}

lxp_program_asset lxp_program_asset_from_words(uint64_t word0, uint64_t word1,
                                               uint64_t word2, uint64_t word3)
{
    lxp_program_asset value;
    lxp_program_bytes32_from_words(word0, word1, word2, word3, value.bytes);
    return value;
}

lxp_program_account lxp_program_account_from_words(uint64_t word0,
                                                   uint64_t word1,
                                                   uint64_t word2,
                                                   uint64_t word3)
{
    lxp_program_account value;
    lxp_program_bytes32_from_words(word0, word1, word2, word3, value.bytes);
    return value;
}

lxp_program_digest lxp_program_digest_from_words(uint64_t word0, uint64_t word1,
                                                 uint64_t word2, uint64_t word3)
{
    lxp_program_digest value;
    lxp_program_bytes32_from_words(word0, word1, word2, word3, value.bytes);
    return value;
}

lxp_program_status lxp_program_check_key(const uint8_t *key, size_t key_length)
{
    if (key == NULL) return LXP_PROGRAM_ERR_NULL_ARGUMENT;
    if (key_length == 0U) return LXP_PROGRAM_ERR_EMPTY_KEY;
    if (key_length > (size_t)LXP_PROGRAM_MAX_STORAGE_KEY_BYTES)
        return LXP_PROGRAM_ERR_KEY_TOO_LARGE;
    return LXP_PROGRAM_OK;
}
