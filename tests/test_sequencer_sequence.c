#include "layerx/lxp_sequencer.h"

#include <stdint.h>
#include <string.h>

typedef struct durable_state {
    uint64_t watermark;
    uint64_t writes;
    uint64_t fail_at;
} durable_state;

static lxp_result persist(void *context, uint64_t watermark)
{
    durable_state *state = (durable_state *)context;
    if (state->writes == state->fail_at) return LXP_ERR_IO;
    state->watermark = watermark;
    state->writes += 1U;
    return LXP_OK;
}

static int million_contiguous(void)
{
    lxp_seq_allocator allocator;
    durable_state durable = { 0U, 0U, UINT64_MAX };
    lxp_admission_result admitted = { LXP_OK, true, true, true };
    uint64_t account_sequence = 19U;
    uint64_t i;
    if (lxp_seq_allocator_init(&allocator, 1U, persist, &durable) != LXP_OK)
        return 1;
    for (i = 0U; i < UINT64_C(1000000); ++i) {
        uint64_t assigned = 0U;
        if (lxp_seq_assign(&allocator, admitted, account_sequence,
                           &account_sequence, &assigned) != LXP_OK ||
            assigned != i + 1U || durable.watermark != assigned)
            return 1;
    }
    return allocator.next_sequence != UINT64_C(1000001) ||
           account_sequence != UINT64_C(1000019) ||
           durable.writes != UINT64_C(1000000);
}

static int rejection_is_atomic(void)
{
    lxp_seq_allocator allocator;
    durable_state durable = { 76U, 0U, UINT64_MAX };
    lxp_admission_result rejected = lxp_admit_activity(NULL, NULL);
    lxp_admission_result admitted = { LXP_OK, true, true, true };
    uint64_t account_sequence = 9U;
    uint64_t assigned = 55U;
    if (lxp_seq_allocator_init(&allocator, 77U, persist, &durable) != LXP_OK ||
        lxp_seq_assign(&allocator, rejected, 9U, &account_sequence,
                       &assigned) != LXP_ERR_MALFORMED_ENVELOPE ||
        allocator.next_sequence != 77U || account_sequence != 9U ||
        assigned != 55U || durable.writes != 0U) return 1;
    durable.fail_at = 0U;
    if (lxp_seq_assign(&allocator, admitted, 9U, &account_sequence,
                       &assigned) != LXP_ERR_IO ||
        allocator.next_sequence != 77U || account_sequence != 9U ||
        assigned != 55U) return 1;
    return 0;
}

static int queue_is_admission_order(void)
{
    lxp_admission_ticket storage[3];
    lxp_admission_queue queue;
    lxp_activity activities[3];
    lxp_admission_ticket ticket;
    lxp_admission_result results[3] = {
        { LXP_OK, true, true, true },
        { LXP_ERR_BAD_SIGNATURE, false, false, false },
        { LXP_OK, true, true, true }
    };
    uint64_t order;
    size_t i;
    (void)memset(activities, 0, sizeof(activities));
    if (lxp_admission_queue_init(&queue, storage, 3U, 41U) != LXP_OK)
        return 1;
    for (i = 0U; i < 3U; ++i) {
        if (lxp_admission_queue_push(&queue, &activities[i], results[2U - i],
                                     &order) != LXP_OK ||
            order != 41U + i) return 1;
    }
    for (i = 0U; i < 3U; ++i) {
        if (lxp_admission_queue_pop(&queue, &ticket) != LXP_OK ||
            ticket.admission_order != 41U + i ||
            ticket.activity != &activities[i]) return 1;
    }
    return lxp_admission_queue_pop(&queue, &ticket) == LXP_ERR_TRUNCATED ?
           0 : 1;
}

static int range_validation(void)
{
    lxp_batch_header previous;
    lxp_batch_header candidate;
    (void)memset(&previous, 0, sizeof(previous));
    (void)memset(&candidate, 0, sizeof(candidate));
    previous.batch_number = 6U;
    previous.last_sequence = 20U;
    previous.resulting_state_root[0] = 9U;
    candidate.batch_number = 7U;
    candidate.first_sequence = 21U;
    candidate.last_sequence = 25U;
    candidate.previous_state_root[0] = 9U;
    if (lxp_batch_range_check(&previous, &candidate) != LXP_OK) return 1;
    candidate.first_sequence = 22U;
    if (lxp_batch_range_check(&previous, &candidate) != LXP_ERR_BATCH_GAP)
        return 1;
    candidate.first_sequence = 21U;
    candidate.last_sequence = 20U;
    if (lxp_batch_range_check(&previous, &candidate) != LXP_ERR_BATCH_GAP)
        return 1;
    candidate.last_sequence = 25U;
    candidate.previous_state_root[0] ^= 1U;
    return lxp_batch_range_check(&previous, &candidate) ==
           LXP_ERR_ROOT_MISMATCH ? 0 : 1;
}

int main(void)
{
    return million_contiguous() != 0 || rejection_is_atomic() != 0 ||
           queue_is_admission_order() != 0 || range_validation() != 0;
}
