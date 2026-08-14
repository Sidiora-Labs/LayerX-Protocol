#include "layerx/lxp_batch.h"
#include "layerx/lxp_merkle.h"

static lxp_result root_for(const lxp_byte_span *items, size_t count,
                           lxp_arena *arena, uint8_t root[32])
{
    uint8_t (*hashes)[32];
    void *memory = NULL;
    size_t mark;
    size_t i;
    lxp_result status;
    if ((items == NULL && count != 0U) || count > LXP_MAX_BATCH_ACTIVITIES ||
        count > SIZE_MAX / 32U) return LXP_ERR_LENGTH_LIMIT;
    if (count == 0U) return lxp_merkle_build(NULL, 0U, arena, root);
    mark = lxp_arena_mark(arena);
    status = lxp_arena_alloc(arena, count * 32U, _Alignof(uint64_t), &memory);
    if (status != LXP_OK) return status;
    hashes = (uint8_t (*)[32])memory;
    for (i = 0U; i < count && status == LXP_OK; ++i) {
        if (items[i].bytes == NULL && items[i].length != 0U)
            status = LXP_ERR_NON_CANONICAL;
        else
            status = lxp_merkle_leaf_hash(items[i].bytes, items[i].length,
                                          hashes[i]);
    }
    if (status == LXP_OK)
        status = lxp_merkle_build((const uint8_t (*)[32])hashes, count,
                                  arena, root);
    (void)lxp_arena_reset(arena, mark);
    return status;
}

lxp_result lxp_batch_roots_compute(const lxp_batch_root_inputs *inputs,
                                   lxp_arena *arena,
                                   lxp_batch_roots *roots)
{
    lxp_result status;
    if (inputs == NULL || arena == NULL || roots == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = root_for(inputs->activities, inputs->activity_count, arena,
                      roots->activity_merkle_root);
    if (status == LXP_OK)
        status = root_for(inputs->receipts, inputs->receipt_count, arena,
                          roots->receipt_merkle_root);
    if (status == LXP_OK)
        status = root_for(inputs->events, inputs->event_count, arena,
                          roots->event_merkle_root);
    if (status == LXP_OK)
        status = root_for(inputs->oracle_inputs, inputs->oracle_input_count,
                          arena, roots->oracle_root);
    if (status == LXP_OK)
        status = root_for(inputs->availability_chunks,
                          inputs->availability_chunk_count, arena,
                          roots->data_availability_root);
    return status;
}
