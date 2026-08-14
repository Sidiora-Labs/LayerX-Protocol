#include "layerx/lxp_sequencer.h"
#include "layerx/lxp_crypto.h"

#include <stdlib.h>
#include <string.h>

static lxp_result replay_durable(
    lxp_log *log, uint64_t snapshot_sequence,
    const lxp_sequencer_recovery_ops *operations, void *context,
    lxp_sequencer_recovery_result *result)
{
    uint64_t offset = 0U;
    while (offset < log->write_offset) {
        lxp_log_record_header header;
        uint8_t *body = NULL;
        lxp_result status = lxp_log_read(log, offset, &header, NULL, 0U);
        if (status != LXP_OK && status != LXP_ERR_LENGTH_LIMIT) return status;
        if (header.body_length != 0U) {
            body = malloc(header.body_length);
            if (body == NULL) return LXP_ERR_IO;
            status = lxp_log_read(log, offset, &header, body,
                                  header.body_length);
        }
        if (status == LXP_OK && (snapshot_sequence == UINT64_MAX ||
            header.global_sequence > snapshot_sequence)) {
            uint8_t recomputed[32];
            uint8_t committed[32];
            bool compare = false;
            status = operations->replay_record(context, &header, body,
                                                recomputed, committed,
                                                &compare);
            if (status == LXP_OK && compare &&
                lxp_ct_memcmp(recomputed, committed, 32U) != 0)
                status = LXP_FATAL_REPLAY_DIVERGENCE;
            if (status == LXP_OK && compare)
                (void)memcpy(result->resulting_state_root, recomputed, 32U);
        }
        free(body);
        if (status != LXP_OK) return status;
        offset += LXP_LOG_HEADER_BYTES + header.body_length;
    }
    return offset == log->write_offset ? LXP_OK : LXP_ERR_LOG_TRUNCATED;
}

lxp_result lxp_sequencer_recover(
    lxp_log *log, const lxp_sequencer_recovery_ops *operations,
    void *context, lxp_sequencer_recovery_result *result)
{
    uint64_t snapshot_sequence = UINT64_MAX;
    uint64_t durable_head;
    lxp_result status;
    if (log == NULL || operations == NULL ||
        operations->replay_record == NULL ||
        operations->rebuild_projections == NULL || result == NULL)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(result, 0, sizeof(*result));
    status = lxp_log_recover(log, NULL, NULL);
    if (status != LXP_OK) return status;
    durable_head = log->next_sequence == 0U ? UINT64_MAX :
                   log->next_sequence - 1U;
    if (operations->load_snapshot != NULL) {
        status = operations->load_snapshot(context, durable_head,
                                           &snapshot_sequence,
                                           result->resulting_state_root);
        if (status == LXP_ERR_UNKNOWN_FIELD) {
            snapshot_sequence = UINT64_MAX;
            (void)memset(result->resulting_state_root, 0, 32U);
            status = LXP_OK;
        }
        if (status != LXP_OK || (durable_head != UINT64_MAX &&
            snapshot_sequence != UINT64_MAX &&
            snapshot_sequence > durable_head)) return status != LXP_OK ?
                status : LXP_ERR_SNAPSHOT_MISMATCH;
    }
    status = replay_durable(log, snapshot_sequence, operations, context,
                            result);
    if (status != LXP_OK) {
        result->halted = status == LXP_FATAL_REPLAY_DIVERGENCE;
        return status;
    }
    status = operations->rebuild_projections(context, log, durable_head);
    if (status != LXP_OK) return status;
    result->durable_head = durable_head;
    result->next_sequence = log->next_sequence;
    result->snapshot_sequence = snapshot_sequence;
    return LXP_OK;
}

lxp_result lxp_sequencer_header_registry_init(
    lxp_sequencer_header_registry *registry,
    lxp_equivocation_publish_fn publish_evidence, void *publish_context)
{
    if (registry == NULL || publish_evidence == NULL)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(registry, 0, sizeof(*registry));
    registry->publish_evidence = publish_evidence;
    registry->publish_context = publish_context;
    return LXP_OK;
}

lxp_result lxp_sequencer_equivocation_detect(
    lxp_sequencer_header_registry *registry,
    const lxp_batch_header *header, const uint8_t signature[64],
    lxp_arena *arena, lxp_sequencer_equivocation_evidence *evidence)
{
    uint8_t hash[32];
    size_t mark;
    size_t i;
    lxp_result status;
    if (registry == NULL || header == NULL || signature == NULL ||
        arena == NULL || evidence == NULL ||
        registry->publish_evidence == NULL) return LXP_ERR_NON_CANONICAL;
    mark = lxp_arena_mark(arena);
    status = lxp_batch_header_hash(header, arena, hash);
    (void)lxp_arena_reset(arena, mark);
    if (status != LXP_OK) return status;
    for (i = 0U; i < registry->count; ++i) {
        lxp_sealed_header_record *prior = &registry->records[i];
        if (prior->header.batch_number != header->batch_number) continue;
        if (lxp_ct_memcmp(prior->header_hash, hash, 32U) == 0) return LXP_OK;
        evidence->first = *prior;
        evidence->second.header = *header;
        (void)memcpy(evidence->second.header_hash, hash, 32U);
        (void)memcpy(evidence->second.signature, signature, 64U);
        registry->checkpoint_halted = true;
        status = registry->publish_evidence(registry->publish_context,
                                            evidence);
        return status == LXP_OK ? LXP_ERR_EQUIVOCATION : status;
    }
    if (registry->count == LXP_MAX_SEALED_BATCH_HEADERS)
        return LXP_ERR_LENGTH_LIMIT;
    registry->records[registry->count].header = *header;
    (void)memcpy(registry->records[registry->count].header_hash, hash, 32U);
    (void)memcpy(registry->records[registry->count].signature, signature, 64U);
    registry->count += 1U;
    return LXP_OK;
}

lxp_result lxp_sequencer_loss(lxp_sequencer_liveness *liveness)
{
    if (liveness == NULL) return LXP_ERR_NON_CANONICAL;
    liveness->accepting_activities = false;
    liveness->handover_required = true;
    (void)memset(liveness->authorised_sequencer_id, 0, 32U);
    liveness->first_authorised_batch = 0U;
    return LXP_OK;
}

lxp_result lxp_sequencer_handover_authorize(
    lxp_sequencer_liveness *liveness, const uint8_t sequencer_id[32],
    uint64_t first_batch_number)
{
    if (liveness == NULL || sequencer_id == NULL ||
        lxp_ct_is_zero(sequencer_id, 32U) || !liveness->handover_required)
        return LXP_ERR_AUTH_SCOPE;
    (void)memcpy(liveness->authorised_sequencer_id, sequencer_id, 32U);
    liveness->first_authorised_batch = first_batch_number;
    liveness->handover_required = false;
    liveness->accepting_activities = true;
    return LXP_OK;
}

lxp_result lxp_sequencer_can_seal(const lxp_sequencer_liveness *liveness,
                                  const lxp_batch_header *header)
{
    if (liveness == NULL || header == NULL) return LXP_ERR_NON_CANONICAL;
    if (liveness->handover_required) return LXP_ERR_MODULE_DISABLED;
    if (liveness->first_authorised_batch != 0U &&
        (header->batch_number < liveness->first_authorised_batch ||
         lxp_ct_memcmp(header->sequencer_id,
                       liveness->authorised_sequencer_id, 32U) != 0))
        return LXP_ERR_AUTH_SCOPE;
    return liveness->accepting_activities ? LXP_OK : LXP_ERR_MODULE_DISABLED;
}
