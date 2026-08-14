#include "layerx/lxp_sequencer.h"

#include <string.h>

lxp_result lxp_seq_allocator_init(lxp_seq_allocator *allocator,
                                  uint64_t next_sequence,
                                  lxp_seq_persist_fn persist,
                                  void *persist_context)
{
    if (allocator == NULL || persist == NULL) return LXP_ERR_NON_CANONICAL;
    allocator->next_sequence = next_sequence;
    allocator->persist = persist;
    allocator->persist_context = persist_context;
    return LXP_OK;
}

lxp_result lxp_seq_assign(lxp_seq_allocator *allocator,
                          lxp_admission_result admission,
                          uint64_t presented_account_sequence,
                          uint64_t *next_account_sequence,
                          uint64_t *global_sequence)
{
    uint64_t assigned;
    lxp_result status;
    if (allocator == NULL || next_account_sequence == NULL ||
        global_sequence == NULL || allocator->persist == NULL)
        return LXP_ERR_NON_CANONICAL;
    if (admission.result_code != LXP_OK) return admission.result_code;
    if (!admission.assign_global_sequence ||
        !admission.consume_account_sequence) return LXP_FATAL_INVARIANT;
    if (presented_account_sequence != *next_account_sequence)
        return presented_account_sequence < *next_account_sequence ?
               LXP_ERR_SEQUENCE_REUSED : LXP_ERR_SEQUENCE_GAP;
    if (allocator->next_sequence == UINT64_MAX ||
        *next_account_sequence == UINT64_MAX) return LXP_ERR_OVERFLOW;
    assigned = allocator->next_sequence;
    status = allocator->persist(allocator->persist_context, assigned);
    if (status != LXP_OK) return status;
    allocator->next_sequence = assigned + 1U;
    *next_account_sequence += 1U;
    *global_sequence = assigned;
    return LXP_OK;
}

lxp_result lxp_batch_range_check(const lxp_batch_header *previous,
                                 const lxp_batch_header *candidate)
{
    if (previous == NULL || candidate == NULL) return LXP_ERR_NON_CANONICAL;
    if (previous->batch_number == UINT64_MAX ||
        candidate->batch_number != previous->batch_number + 1U ||
        previous->last_sequence == UINT64_MAX ||
        candidate->first_sequence != previous->last_sequence + 1U ||
        candidate->last_sequence < candidate->first_sequence)
        return LXP_ERR_BATCH_GAP;
    return memcmp(candidate->previous_state_root,
                  previous->resulting_state_root, 32U) == 0 ?
           LXP_OK : LXP_ERR_ROOT_MISMATCH;
}
