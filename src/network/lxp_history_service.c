#include "layerx/lxp_history.h"

lxp_result lxp_history_serve_range(const lxp_history *history,
                                   uint64_t first_sequence,
                                   uint64_t last_sequence,
                                   size_t maximum_response_bytes,
                                   lxp_history_send_fn send,
                                   void *send_context, lxp_arena *arena)
{
    lxp_history_query_spec query = {0};
    lxp_history_result result;
    size_t mark;
    size_t i;
    lxp_result status;
    if (send == NULL || arena == NULL || maximum_response_bytes == 0U ||
        first_sequence > last_sequence) return LXP_ERR_NON_CANONICAL;
    query.kind = LXP_HISTORY_BY_SEQUENCE_RANGE;
    query.first_sequence = first_sequence;
    query.last_sequence = last_sequence;
    query.maximum_response_bytes = maximum_response_bytes;
    mark = lxp_arena_mark(arena);
    status = lxp_history_query(history, &query, arena, &result);
    for (i = 0U; status == LXP_OK && i < result.count; ++i)
        status = send(send_context, result.items[i].record_kind,
                      result.items[i].global_sequence,
                      result.items[i].canonical_bytes.bytes,
                      result.items[i].canonical_bytes.length);
    (void)lxp_arena_reset(arena, mark);
    return status;
}
