#include "layerx/lxp_merkle.h"
#include "layerx/lxp_crypto.h"

#include <string.h>

lxp_result lxp_merkle_leaf_hash(const void *data, size_t length, uint8_t out[32])
{
    return lxp_hash_domain(LXP_DOMAIN_MERKLE_LEAF, data, length, out);
}

lxp_result lxp_merkle_node_hash(const uint8_t left[32], const uint8_t right[32],
                                uint8_t out[32])
{
    uint8_t pair[64];
    lxp_result result;
    if (left == NULL || right == NULL || out == NULL) return LXP_ERR_NON_CANONICAL;
    (void)memcpy(pair, left, 32U);
    (void)memcpy(pair + 32U, right, 32U);
    result = lxp_hash_domain(LXP_DOMAIN_MERKLE_INTERNAL, pair, sizeof(pair), out);
    (void)memset(pair, 0, sizeof(pair));
    return result;
}

lxp_result lxp_merkle_build(const uint8_t (*leaf_hashes)[32], size_t count,
                            lxp_arena *arena, uint8_t root[32])
{
    uint8_t (*level)[32];
    void *memory = NULL;
    size_t mark;
    size_t level_count;
    size_t i;
    lxp_result result;
    if (arena == NULL || root == NULL || (leaf_hashes == NULL && count != 0U) ||
        count > SIZE_MAX / 32U) return LXP_ERR_LENGTH_LIMIT;
    if (count == 0U) return lxp_merkle_leaf_hash(NULL, 0U, root);
    mark = lxp_arena_mark(arena);
    result = lxp_arena_alloc(arena, count * 32U, _Alignof(uint64_t), &memory);
    if (result != LXP_OK) return result;
    level = (uint8_t (*)[32])memory;
    (void)memcpy(level, leaf_hashes, count * 32U);
    level_count = count;
    while (level_count > 1U) {
        size_t next_count = (level_count + 1U) / 2U;
        for (i = 0U; i < next_count; ++i) {
            size_t right = i * 2U + 1U;
            if (right >= level_count) right = i * 2U;
            result = lxp_merkle_node_hash(level[i * 2U], level[right], level[i]);
            if (result != LXP_OK) {
                (void)lxp_arena_reset(arena, mark);
                return result;
            }
        }
        level_count = next_count;
    }
    (void)memcpy(root, level[0], 32U);
    return lxp_arena_reset(arena, mark);
}

lxp_result lxp_merkle_root(const lxp_merkle_leaf *leaves, size_t count,
                           lxp_arena *arena, uint8_t root[32])
{
    lxp_byte_span *keys;
    uint8_t (*hashes)[32];
    void *key_memory = NULL;
    void *hash_memory = NULL;
    size_t mark;
    size_t i;
    lxp_result result;
    if (arena == NULL || root == NULL || (leaves == NULL && count != 0U) ||
        count > SIZE_MAX / sizeof(lxp_byte_span) || count > SIZE_MAX / 32U)
        return LXP_ERR_LENGTH_LIMIT;
    if (count == 0U) return lxp_merkle_build(NULL, 0U, arena, root);
    mark = lxp_arena_mark(arena);
    result = lxp_arena_alloc(arena, count * sizeof(lxp_byte_span),
                             _Alignof(lxp_byte_span), &key_memory);
    if (result != LXP_OK) return result;
    result = lxp_arena_alloc(arena, count * 32U, _Alignof(uint64_t), &hash_memory);
    if (result != LXP_OK) { (void)lxp_arena_reset(arena, mark); return result; }
    keys = (lxp_byte_span *)key_memory;
    hashes = (uint8_t (*)[32])hash_memory;
    for (i = 0U; i < count; ++i) keys[i] = leaves[i].key;
    result = lxp_codec_seq_check_sorted(keys, count);
    for (i = 0U; result == LXP_OK && i < count; ++i)
        result = lxp_merkle_leaf_hash(leaves[i].value.bytes,
                                      leaves[i].value.length, hashes[i]);
    if (result == LXP_OK)
        result = lxp_merkle_build((const uint8_t (*)[32])hashes,
                                  count, arena, root);
    (void)lxp_arena_reset(arena, mark);
    return result;
}

static uint8_t proof_depth(uint32_t count)
{
    uint8_t depth = 0U;
    while (count > 1U) { count = (count + 1U) / 2U; ++depth; }
    return depth;
}

lxp_result lxp_merkle_proof_generate(const uint8_t (*leaf_hashes)[32],
                                     size_t count, size_t leaf_index,
                                     lxp_arena *arena,
                                     lxp_merkle_proof *proof,
                                     uint8_t root[32])
{
    uint8_t (*level)[32];
    void *memory = NULL;
    size_t mark;
    size_t level_count = count;
    size_t index = leaf_index;
    size_t i;
    lxp_result result;
    if (leaf_hashes == NULL || arena == NULL || proof == NULL || root == NULL ||
        count == 0U || count > UINT32_MAX || leaf_index >= count ||
        proof_depth((uint32_t)count) > LXP_MERKLE_MAX_DEPTH ||
        count > SIZE_MAX / 32U) return LXP_ERR_LENGTH_LIMIT;
    mark = lxp_arena_mark(arena);
    result = lxp_arena_alloc(arena, count * 32U, _Alignof(uint64_t), &memory);
    if (result != LXP_OK) return result;
    level = (uint8_t (*)[32])memory;
    (void)memcpy(level, leaf_hashes, count * 32U);
    (void)memset(proof, 0, sizeof(*proof));
    proof->leaf_index = (uint32_t)leaf_index;
    proof->leaf_count = (uint32_t)count;
    proof->depth = proof_depth((uint32_t)count);
    for (i = 0U; level_count > 1U; ++i) {
        size_t sibling = index ^ 1U;
        size_t next_count = (level_count + 1U) / 2U;
        size_t node;
        if (sibling >= level_count) sibling = index;
        (void)memcpy(proof->siblings[i], level[sibling], 32U);
        for (node = 0U; node < next_count; ++node) {
            size_t right = node * 2U + 1U;
            if (right >= level_count) right = node * 2U;
            result = lxp_merkle_node_hash(level[node * 2U], level[right], level[node]);
            if (result != LXP_OK) { (void)lxp_arena_reset(arena, mark); return result; }
        }
        index /= 2U;
        level_count = next_count;
    }
    (void)memcpy(root, level[0], 32U);
    return lxp_arena_reset(arena, mark);
}

lxp_result lxp_merkle_proof_verify(const uint8_t leaf_hash[32],
                                   const lxp_merkle_proof *proof,
                                   const uint8_t expected_root[32])
{
    uint8_t current[32];
    uint8_t next[32];
    uint8_t difference = 0U;
    uint32_t index;
    uint32_t level_count;
    size_t i;
    lxp_result result;
    if (leaf_hash == NULL || proof == NULL || expected_root == NULL ||
        proof->leaf_count == 0U || proof->leaf_index >= proof->leaf_count ||
        proof->depth > LXP_MERKLE_MAX_DEPTH ||
        proof->depth != proof_depth(proof->leaf_count)) return LXP_ERR_NON_CANONICAL;
    (void)memcpy(current, leaf_hash, 32U);
    index = proof->leaf_index;
    level_count = proof->leaf_count;
    for (i = 0U; i < proof->depth; ++i) {
        uint32_t sibling = index ^ 1U;
        const uint8_t *left;
        const uint8_t *right;
        if (sibling >= level_count &&
            lxp_ct_memcmp(proof->siblings[i], current, 32U) != 0)
            return LXP_ERR_NON_CANONICAL;
        left = (index & 1U) == 0U ? current : proof->siblings[i];
        right = (index & 1U) == 0U ? proof->siblings[i] : current;
        result = lxp_merkle_node_hash(left, right, next);
        if (result != LXP_OK) return result;
        (void)memcpy(current, next, 32U);
        index /= 2U;
        level_count = (level_count + 1U) / 2U;
    }
    for (i = 0U; i < 32U; ++i) difference |= current[i] ^ expected_root[i];
    lxp_secure_zero(current, sizeof(current));
    lxp_secure_zero(next, sizeof(next));
    return difference == 0U ? LXP_OK : LXP_ERR_ROOT_MISMATCH;
}

lxp_result lxp_merkle_proof_encode(lxp_codec_writer *writer,
                                   const lxp_merkle_proof *proof)
{
    lxp_result result;
    if (proof == NULL || proof->depth > LXP_MERKLE_MAX_DEPTH ||
        proof->depth != proof_depth(proof->leaf_count)) return LXP_ERR_NON_CANONICAL;
    result = lxp_codec_write_struct_header(writer, 0x4d50U);
    if (result == LXP_OK) result = lxp_codec_write_u32(writer, proof->leaf_index);
    if (result == LXP_OK) result = lxp_codec_write_u32(writer, proof->leaf_count);
    if (result == LXP_OK) result = lxp_codec_write_u8(writer, proof->depth);
    if (result == LXP_OK)
        result = lxp_codec_write_bytes(writer, proof->siblings,
                    (size_t)proof->depth * 32U, LXP_MERKLE_MAX_DEPTH * 32U);
    return result;
}
