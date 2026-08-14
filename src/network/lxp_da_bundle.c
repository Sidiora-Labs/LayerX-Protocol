#include "layerx/lxp_da.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_merkle.h"

#include <string.h>

static void store_u32(uint8_t out[4], uint32_t value)
{
    out[0] = (uint8_t)(value >> 24U);
    out[1] = (uint8_t)(value >> 16U);
    out[2] = (uint8_t)(value >> 8U);
    out[3] = (uint8_t)value;
}

static void store_u64(uint8_t out[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i) out[7U - i] = (uint8_t)(value >> (i * 8U));
}

lxp_result lxp_da_chunk_hash(lxp_da_chunk *chunk)
{
    uint8_t metadata[25];
    lxp_hash_context hash;
    size_t tag_length;
    const uint8_t *tag = lxp_domain_tag(LXP_DOMAIN_DA_CHUNK, &tag_length);
    lxp_result status;
    if (tag == NULL) return LXP_ERR_INVALID_TAG;
    store_u64(metadata, chunk->batch_number);
    store_u32(metadata + 8U, chunk->chunk_index);
    metadata[12] = (uint8_t)chunk->availability_class;
    store_u64(metadata + 13U, chunk->class_offset);
    store_u32(metadata + 21U, chunk->length);
    lxp_hash_init(&hash);
    status = lxp_hash_update(&hash, tag, tag_length);
    if (status == LXP_OK)
        status = lxp_hash_update(&hash, metadata, sizeof(metadata));
    if (status == LXP_OK)
        status = lxp_hash_update(&hash, chunk->bytes.bytes,
                                 chunk->bytes.length);
    return status == LXP_OK ? lxp_hash_final(&hash, chunk->chunk_hash) : status;
}

lxp_result lxp_da_recovery_metadata_encode(
    const lxp_da_recovery_input *input, lxp_arena *arena,
    lxp_byte_span *encoded)
{
    lxp_codec_writer writer;
    size_t capacity;
    size_t i;
    lxp_result status;
    if (input == NULL || arena == NULL || encoded == NULL ||
        input->module_root_count > LXP_DA_MAX_MODULE_ROOTS ||
        (input->module_roots == NULL && input->module_root_count != 0U) ||
        input->account_tree_frontier.length >
            LXP_DA_MAX_ACCOUNT_FRONTIER_BYTES ||
        (input->account_tree_frontier.bytes == NULL &&
         input->account_tree_frontier.length != 0U))
        return LXP_ERR_NON_CANONICAL;
    for (i = 1U; i < input->module_root_count; ++i)
        if (input->module_roots[i - 1U].module_id >=
            input->module_roots[i].module_id)
            return LXP_ERR_UNSORTED_SEQUENCE;
    capacity = 4U + input->module_root_count * 38U + 4U +
               input->account_tree_frontier.length + 24U;
    status = lxp_codec_writer_init(&writer, arena, capacity);
    if (status == LXP_OK)
        status = lxp_codec_write_seq(&writer,
            (uint32_t)input->module_root_count, LXP_DA_MAX_MODULE_ROOTS);
    for (i = 0U; status == LXP_OK && i < input->module_root_count; ++i) {
        status = lxp_codec_write_u16(&writer,
                                     input->module_roots[i].module_id);
        if (status == LXP_OK)
            status = lxp_codec_write_bytes(&writer,
                input->module_roots[i].state_root, 32U, 32U);
    }
    if (status == LXP_OK)
        status = lxp_codec_write_bytes(&writer,
            input->account_tree_frontier.bytes,
            input->account_tree_frontier.length,
            LXP_DA_MAX_ACCOUNT_FRONTIER_BYTES);
    if (status == LXP_OK)
        status = lxp_codec_write_u64(&writer, input->next_global_sequence);
    if (status == LXP_OK)
        status = lxp_codec_write_u64(&writer, input->receipt_watermark);
    if (status == LXP_OK)
        status = lxp_codec_write_u64(&writer, input->projection_watermark);
    if (status != LXP_OK) return status;
    encoded->bytes = writer.bytes;
    encoded->length = writer.length;
    return LXP_OK;
}

static size_t class_chunk_count(size_t length, size_t chunk_size)
{
    return length == 0U ? 1U : 1U + (length - 1U) / chunk_size;
}

lxp_result lxp_da_bundle_build(const lxp_batch_body *body, size_t chunk_size,
                               lxp_arena *arena, lxp_da_bundle *bundle)
{
    const lxp_byte_span classes[LXP_DA_CLASS_COUNT] = {
        body == NULL ? (lxp_byte_span){NULL, 0U} : body->activities,
        body == NULL ? (lxp_byte_span){NULL, 0U} : body->receipts,
        body == NULL ? (lxp_byte_span){NULL, 0U} : body->oracle_inputs,
        body == NULL ? (lxp_byte_span){NULL, 0U} : body->state_diff,
        body == NULL ? (lxp_byte_span){NULL, 0U} : body->recovery_metadata
    };
    size_t count = 0U;
    size_t total = 0U;
    size_t index = 0U;
    size_t class_index;
    void *memory;
    lxp_result status = LXP_OK;
    if (body == NULL || arena == NULL || bundle == NULL || chunk_size == 0U ||
        chunk_size > LXP_DA_MAX_CHUNK_BYTES)
        return LXP_ERR_NON_CANONICAL;
    for (class_index = 0U; class_index < LXP_DA_CLASS_COUNT; ++class_index) {
        size_t class_count;
        if (classes[class_index].bytes == NULL &&
            classes[class_index].length != 0U)
            return LXP_ERR_NON_CANONICAL;
        class_count = class_chunk_count(classes[class_index].length,
                                        chunk_size);
        if (class_count > LXP_DA_MAX_CHUNKS - count ||
            classes[class_index].length > SIZE_MAX - total)
            return LXP_ERR_LENGTH_LIMIT;
        count += class_count;
        total += classes[class_index].length;
    }
    status = lxp_arena_alloc(arena, count * sizeof(lxp_da_chunk),
                             _Alignof(lxp_da_chunk), &memory);
    if (status != LXP_OK) return status;
    bundle->chunks = (lxp_da_chunk *)memory;
    bundle->chunk_count = count;
    bundle->batch_number = body->header.batch_number;
    bundle->total_bytes = total;
    for (class_index = 0U; status == LXP_OK &&
         class_index < LXP_DA_CLASS_COUNT; ++class_index) {
        uint64_t offset = 0U;
        do {
            lxp_da_chunk *chunk = &bundle->chunks[index];
            size_t remaining = classes[class_index].length - (size_t)offset;
            size_t length = remaining < chunk_size ? remaining : chunk_size;
            chunk->batch_number = body->header.batch_number;
            chunk->chunk_index = (uint32_t)index;
            chunk->availability_class = (lxp_da_class)(class_index + 1U);
            chunk->class_offset = offset;
            chunk->length = (uint32_t)length;
            chunk->bytes.bytes = length == 0U ? NULL :
                classes[class_index].bytes + (size_t)offset;
            chunk->bytes.length = length;
            status = lxp_da_chunk_hash(chunk);
            offset += length;
            ++index;
        } while (status == LXP_OK &&
                 offset < classes[class_index].length);
    }
    return status;
}

lxp_result lxp_da_bundle_root(const lxp_da_bundle *bundle, lxp_arena *arena,
                              uint8_t root[32])
{
    uint8_t (*hashes)[32];
    void *memory;
    size_t mark;
    size_t i;
    lxp_result status;
    if (bundle == NULL || arena == NULL || root == NULL ||
        bundle->chunks == NULL || bundle->chunk_count < LXP_DA_CLASS_COUNT ||
        bundle->chunk_count > LXP_DA_MAX_CHUNKS)
        return LXP_ERR_NON_CANONICAL;
    mark = lxp_arena_mark(arena);
    status = lxp_arena_alloc(arena, bundle->chunk_count * 32U,
                             _Alignof(uint64_t), &memory);
    if (status != LXP_OK) return status;
    hashes = (uint8_t (*)[32])memory;
    for (i = 0U; i < bundle->chunk_count; ++i) {
        lxp_da_chunk copy = bundle->chunks[i];
        if (copy.chunk_index != i || copy.batch_number != bundle->batch_number) {
            status = LXP_ERR_UNSORTED_SEQUENCE;
            break;
        }
        status = lxp_da_chunk_hash(&copy);
        if (status != LXP_OK ||
            lxp_ct_memcmp(copy.chunk_hash,
                          bundle->chunks[i].chunk_hash, 32U) != 0) {
            status = status == LXP_OK ? LXP_ERR_ROOT_MISMATCH : status;
            break;
        }
        (void)memcpy(hashes[i], copy.chunk_hash, 32U);
    }
    if (status == LXP_OK)
        status = lxp_merkle_build((const uint8_t (*)[32])hashes,
                                  bundle->chunk_count, arena, root);
    (void)lxp_arena_reset(arena, mark);
    return status;
}
