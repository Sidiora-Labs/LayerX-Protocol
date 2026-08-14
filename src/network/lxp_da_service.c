#include "layerx/lxp_da.h"

#include <string.h>

enum { LXP_DA_RESPONSE_HEADER_BYTES = 98 };

static void put_u32(uint8_t out[4], uint32_t value)
{
    out[0] = (uint8_t)(value >> 24U);
    out[1] = (uint8_t)(value >> 16U);
    out[2] = (uint8_t)(value >> 8U);
    out[3] = (uint8_t)value;
}

static void put_u64(uint8_t out[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i) out[7U - i] = (uint8_t)(value >> (i * 8U));
}

lxp_result lxp_da_serve_chunk(const lxp_da_store *store,
                              uint64_t batch_number, uint32_t chunk_index,
                              lxp_arena *arena, lxp_byte_span *response)
{
    lxp_da_bundle bundle;
    lxp_da_chunk *chunk;
    uint8_t root[32];
    uint8_t *encoded;
    void *memory;
    size_t mark;
    size_t length;
    lxp_result status;
    if (store == NULL || arena == NULL || response == NULL)
        return LXP_ERR_NON_CANONICAL;
    mark = lxp_arena_mark(arena);
    status = lxp_da_store_read_bundle(store, batch_number, arena, &bundle,
                                      root);
    if (status != LXP_OK) {
        (void)lxp_arena_reset(arena, mark);
        return status;
    }
    if (chunk_index >= bundle.chunk_count) {
        (void)lxp_arena_reset(arena, mark);
        return LXP_ERR_DA_MISSING;
    }
    chunk = &bundle.chunks[chunk_index];
    length = LXP_DA_RESPONSE_HEADER_BYTES + chunk->length;
    status = lxp_arena_alloc(arena, length, 1U, &memory);
    if (status != LXP_OK) {
        (void)lxp_arena_reset(arena, mark);
        return status;
    }
    encoded = (uint8_t *)memory;
    (void)memcpy(encoded, "LXDR", 4U);
    encoded[4] = 1U;
    put_u64(encoded + 5U, batch_number);
    put_u32(encoded + 13U, chunk_index);
    put_u32(encoded + 17U, (uint32_t)bundle.chunk_count);
    encoded[21] = (uint8_t)chunk->availability_class;
    put_u64(encoded + 22U, chunk->class_offset);
    put_u32(encoded + 30U, chunk->length);
    (void)memcpy(encoded + 34U, chunk->chunk_hash, 32U);
    (void)memcpy(encoded + 66U, root, 32U);
    if (chunk->length != 0U)
        (void)memcpy(encoded + LXP_DA_RESPONSE_HEADER_BYTES,
                     chunk->bytes.bytes, chunk->length);
    response->bytes = encoded;
    response->length = length;
    return LXP_OK;
}
