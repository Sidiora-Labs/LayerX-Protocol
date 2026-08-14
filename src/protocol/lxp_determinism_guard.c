#include "layerx/lxp_kernel.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_hash.h"

#include <stdatomic.h>
#include <string.h>

static atomic_bool guard_violated;

lxp_result lxp_determinism_guard_check(void)
{
    return atomic_load_explicit(&guard_violated, memory_order_acquire) ?
           LXP_FATAL_INVARIANT : LXP_OK;
}

lxp_result lxp_determinism_guard_trip(const char *symbol)
{
    if (symbol == NULL || symbol[0] == '\0') return LXP_ERR_NON_CANONICAL;
    atomic_store_explicit(&guard_violated, true, memory_order_release);
    return LXP_FATAL_INVARIANT;
}

void lxp_determinism_guard_reset(void)
{
    atomic_store_explicit(&guard_violated, false, memory_order_release);
}

lxp_result lxp_replay_compare_roots(const uint8_t expected[32],
                                    const uint8_t produced[32])
{
    if (expected == NULL || produced == NULL) return LXP_ERR_NON_CANONICAL;
    return lxp_ct_memcmp(expected, produced, 32U) == 0 ? LXP_OK :
           LXP_FATAL_REPLAY_DIVERGENCE;
}

lxp_result lxp_kernel_replay(lxp_kernel *kernel,
                             const lxp_replay_record *records,
                             const uint8_t (*expected_roots)[32],
                             size_t record_count, size_t worker_threads,
                             uint8_t terminal_root[32])
{
    size_t i;
    lxp_result status;
    (void)worker_threads;
    if (kernel == NULL || terminal_root == NULL ||
        (records == NULL && record_count != 0U)) return LXP_ERR_NON_CANONICAL;
    status = lxp_determinism_guard_check();
    if (status != LXP_OK) return status;
    for (i = 0U; i < record_count; ++i) {
        uint8_t arena_bytes[64];
        lxp_arena arena;
        lxp_module_ctx ctx;
        status = lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes));
        if (status == LXP_OK)
            status = lxp_module_ctx_init(&ctx, kernel, records[i].module_id,
                                         (uint64_t)i + 1U, kernel->epoch,
                                         kernel->state->next_sequence, 1U,
                                         &arena, true);
        if (status == LXP_OK)
            status = lxp_ctx_kv_put(&ctx, records[i].key,
                                    records[i].key_length, records[i].value,
                                    records[i].value_length);
        if (status == LXP_OK) status = lxp_module_ctx_commit(&ctx);
        if (status == LXP_OK)
            status = lxp_state_journal_open(kernel->state,
                                            kernel->state->next_sequence,
                                            kernel->journal);
        if (status == LXP_OK) status = lxp_state_journal_commit(kernel->journal);
        if (status == LXP_OK) status = lxp_state_root(kernel, terminal_root);
        if (status == LXP_OK && expected_roots != NULL)
            status = lxp_replay_compare_roots(expected_roots[i], terminal_root);
        if (status != LXP_OK) return status;
    }
    if (record_count == 0U) return lxp_state_root(kernel, terminal_root);
    return LXP_OK;
}

lxp_result lxp_replay_golden_run(const lxp_replay_record *records,
                                 size_t record_count,
                                 const uint8_t (*roots)[32],
                                 size_t worker_threads,
                                 uint8_t digest[32])
{
    lxp_hash_context context;
    size_t i;
    (void)records;
    (void)worker_threads;
    if (digest == NULL || (roots == NULL && record_count != 0U))
        return LXP_ERR_NON_CANONICAL;
    lxp_hash_init(&context);
    for (i = 0U; i < record_count; ++i) {
        lxp_result status = lxp_hash_update(&context, roots[i], 32U);
        if (status != LXP_OK) return status;
    }
    return lxp_hash_final(&context, digest);
}
