#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_projection.h"
#include "layerx/lxp_sequencer.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

typedef struct recovery_context {
    lxp_projection *projection;
    size_t activity_count;
    size_t receipt_count;
    bool force_divergence;
} recovery_context;

static lxp_result replay_record(void *opaque,
                                const lxp_log_record_header *header,
                                const uint8_t *body,
                                uint8_t recomputed[32],
                                uint8_t committed[32], bool *compare)
{
    recovery_context *context = (recovery_context *)opaque;
    (void)memset(recomputed, 0, 32U);
    (void)memset(committed, 0, 32U);
    *compare = false;
    if (header->record_kind == (uint8_t)LXP_LOG_ACTIVITY)
        context->activity_count += 1U;
    if (header->record_kind == (uint8_t)LXP_LOG_RECEIPT)
        context->receipt_count += 1U;
    if (header->record_kind == (uint8_t)LXP_LOG_STATE_DIFF) {
        if (header->body_length != 1U) return LXP_FATAL_REPLAY_DIVERGENCE;
        recomputed[0] = body[0];
        committed[0] = context->force_divergence ?
                       (uint8_t)(body[0] ^ 1U) : body[0];
        *compare = true;
    }
    return LXP_OK;
}

static lxp_result rebuild(void *opaque, lxp_log *log, uint64_t durable_head)
{
    recovery_context *context = (recovery_context *)opaque;
    (void)durable_head;
    return lxp_projection_rebuild(context->projection, log,
                                  "migrations/0001_projection.sql");
}

static int write_child(const char *directory, uint32_t boundary)
{
    uint8_t activity = 3U;
    uint8_t state_diff = 9U;
    uint8_t encoded[256];
    uint8_t receipt_data[2] = { 4U, 5U };
    size_t encoded_length;
    lxp_projection_record record;
    lxp_log log;
    (void)memset(&record, 0, sizeof(record));
    record.activity_id[0] = 1U;
    record.idempotency_key[0] = 2U;
    record.account_id[0] = 3U;
    record.asset_id[0] = 4U;
    record.amount[15] = 5U;
    record.receipt = receipt_data;
    record.receipt_length = sizeof(receipt_data);
    if (lxp_projection_record_encode(&record, encoded, sizeof(encoded),
                                     &encoded_length) != LXP_OK ||
        lxp_log_segment_create(&log, directory, 0U, 8192U) != LXP_OK ||
        lxp_log_append(&log, LXP_LOG_ACTIVITY, 0U, &activity, 1U, NULL) !=
            LXP_OK || lxp_log_sync(&log) != LXP_OK) return 1;
    if (boundary == 1U) _exit(71);
    if (lxp_log_append(&log, LXP_LOG_RECEIPT, 0U, encoded,
                       (uint32_t)encoded_length, NULL) != LXP_OK ||
        lxp_log_append(&log, LXP_LOG_STATE_DIFF, 0U, &state_diff, 1U, NULL) !=
            LXP_OK || lxp_log_sync(&log) != LXP_OK) return 1;
    if (boundary == 2U) _exit(72);
    if (lxp_log_append(&log, LXP_LOG_BATCH_HEADER, 0U, &state_diff, 1U,
                       NULL) != LXP_OK || lxp_log_sync(&log) != LXP_OK)
        return 1;
    _exit(73);
}

static int fault_boundaries(void)
{
    uint32_t boundary;
    for (boundary = 1U; boundary <= 3U; ++boundary) {
        char directory[] = "/tmp/lxp-seq-recovery-XXXXXX";
        char path[128];
        char db_path[] = "/tmp/lxp-seq-projection-XXXXXX";
        pid_t child;
        int status;
        int descriptor;
        lxp_log log;
        lxp_projection projection;
        lxp_sequencer_recovery_ops operations = {
            NULL, replay_record, rebuild
        };
        lxp_sequencer_recovery_result result;
        recovery_context context;
        if (mkdtemp(directory) == NULL) return 1;
        child = fork();
        if (child < 0) return 1;
        if (child == 0) _exit(write_child(directory, boundary));
        if (waitpid(child, &status, 0) != child || !WIFEXITED(status) ||
            WEXITSTATUS(status) == 0) return 1;
        descriptor = mkstemp(db_path);
        if (descriptor < 0 || close(descriptor) != 0 || unlink(db_path) != 0 ||
            snprintf(path, sizeof(path), "%s/%020u.lxp", directory, 0U) < 0 ||
            lxp_log_open(&log, path) != LXP_OK ||
            lxp_projection_open(&projection, db_path,
                "migrations/0001_projection.sql") != LXP_OK) return 1;
        (void)memset(&context, 0, sizeof(context));
        context.projection = &projection;
        if (lxp_sequencer_recover(&log, &operations, &context, &result) !=
            LXP_OK) return 1;
        if (boundary == 1U) {
            if (context.activity_count != 0U || context.receipt_count != 0U ||
                result.next_sequence != 0U || log.write_offset != 0U) return 1;
        } else if (context.activity_count != 1U ||
                   context.receipt_count != 1U ||
                   result.durable_head != 0U || result.next_sequence != 1U ||
                   result.resulting_state_root[0] != 9U) return 1;
        if (boundary > 1U) {
            context.force_divergence = true;
            context.activity_count = 0U;
            context.receipt_count = 0U;
            if (lxp_sequencer_recover(&log, &operations, &context, &result) !=
                    LXP_FATAL_REPLAY_DIVERGENCE || !result.halted)
                return 1;
        }
        if (lxp_projection_close(&projection) != LXP_OK ||
            lxp_log_close(&log) != LXP_OK || unlink(path) != 0 ||
            rmdir(directory) != 0 || unlink(db_path) != 0) return 1;
    }
    return 0;
}

static lxp_result publish_evidence(
    void *opaque, const lxp_sequencer_equivocation_evidence *evidence)
{
    size_t *published = (size_t *)opaque;
    if (evidence->first.header.batch_number !=
        evidence->second.header.batch_number) return LXP_ERR_NON_CANONICAL;
    *published += 1U;
    return LXP_OK;
}

static int equivocation_and_handover(void)
{
    uint8_t arena_storage[2048];
    uint8_t signature[64] = { 1U };
    uint8_t next_id[32] = { 9U };
    lxp_arena arena;
    lxp_batch_header first;
    lxp_batch_header second;
    lxp_sequencer_header_registry registry;
    lxp_sequencer_equivocation_evidence evidence;
    lxp_sequencer_liveness liveness = { true, false, {0U}, 0U };
    size_t published = 0U;
    (void)memset(&first, 0, sizeof(first));
    first.protocol_version = 1U;
    first.network_id = 3U;
    first.batch_number = 4U;
    first.timestamp_ms = 8U;
    second = first;
    second.resulting_state_root[0] = 1U;
    if (lxp_arena_init(&arena, arena_storage, sizeof(arena_storage)) != LXP_OK ||
        lxp_sequencer_header_registry_init(&registry, publish_evidence,
                                           &published) != LXP_OK ||
        lxp_sequencer_equivocation_detect(&registry, &first, signature,
                                           &arena, &evidence) != LXP_OK ||
        lxp_sequencer_equivocation_detect(&registry, &first, signature,
                                           &arena, &evidence) != LXP_OK ||
        lxp_sequencer_equivocation_detect(&registry, &second, signature,
                                           &arena, &evidence) !=
            LXP_ERR_EQUIVOCATION || !registry.checkpoint_halted ||
        published != 1U) return 1;
    first.batch_number = 5U;
    (void)memcpy(first.sequencer_id, next_id, 32U);
    if (lxp_sequencer_loss(&liveness) != LXP_OK ||
        liveness.accepting_activities || !liveness.handover_required ||
        lxp_sequencer_can_seal(&liveness, &first) != LXP_ERR_MODULE_DISABLED ||
        lxp_sequencer_handover_authorize(&liveness, next_id, 5U) != LXP_OK ||
        lxp_sequencer_can_seal(&liveness, &first) != LXP_OK)
        return 1;
    return lxp_sequencer_can_seal(&liveness, &first) == LXP_OK ? 0 : 1;
}

int main(void)
{
    return fault_boundaries() != 0 || equivocation_and_handover() != 0;
}
