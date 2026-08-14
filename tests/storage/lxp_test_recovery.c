#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_storage.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef struct replay_state {
    uint8_t balance;
    uint8_t expected;
    int have_expected;
    int force_divergence;
} replay_state;

static lxp_result replay(void *opaque, const lxp_log_record_header *header,
                         const uint8_t *body)
{
    replay_state *state = (replay_state *)opaque;
    if (header->body_length != 1U) return LXP_FATAL_REPLAY_DIVERGENCE;
    if (header->record_kind == (uint8_t)LXP_LOG_CHECKPOINT)
        state->balance = body[0];
    else if (header->record_kind == (uint8_t)LXP_LOG_RECEIPT) {
        state->expected = body[0];
        state->have_expected = 1;
    } else if (header->record_kind == (uint8_t)LXP_LOG_STATE_DIFF) {
        state->balance = (uint8_t)(state->balance + body[0]);
        if (state->force_divergence != 0 || state->have_expected == 0 ||
            state->balance != state->expected)
            return LXP_FATAL_REPLAY_DIVERGENCE;
        state->have_expected = 0;
    }
    return LXP_OK;
}

int main(void)
{
    char directory[] = "/tmp/lxp-recovery-XXXXXX";
    char path[128];
    uint8_t one = 1U;
    uint8_t expected = 1U;
    uint8_t partial[7] = { 'L', 'X', 'P', 'L', 1U, 0U, 0U };
    lxp_log log;
    replay_state state = { 0U, 0U, 0, 0 };
    replay_state divergent = { 0U, 0U, 0, 1 };
    uint64_t incomplete_offset;
    if (mkdtemp(directory) == NULL ||
        lxp_log_segment_create(&log, directory, 0U, 4096U) != LXP_OK) {
        (void)fprintf(stderr, "recovery setup failed\n");
        return 1;
    }
    if (lxp_log_append(&log, LXP_LOG_ACTIVITY, 0U, &one, 1U, NULL) != LXP_OK ||
        lxp_log_sync(&log) != LXP_OK ||
        lxp_log_append(&log, LXP_LOG_RECEIPT, 0U, &expected, 1U, NULL) != LXP_OK ||
        lxp_log_append(&log, LXP_LOG_STATE_DIFF, 0U, &one, 1U, NULL) != LXP_OK ||
        lxp_log_sync(&log) != LXP_OK ||
        lxp_log_append(&log, LXP_LOG_ACTIVITY, 1U, &one, 1U,
                       &incomplete_offset) != LXP_OK) {
        (void)fprintf(stderr, "recovery append failed\n");
        return 1;
    }
    if (pwrite(log.descriptor, partial, sizeof(partial),
               (off_t)log.write_offset) != (ssize_t)sizeof(partial) ||
        lxp_log_close(&log) != LXP_OK) {
        (void)fprintf(stderr, "partial tail write failed\n");
        return 1;
    }
    if (snprintf(path, sizeof(path), "%s/%020u.lxp", directory, 0U) < 0 ||
        lxp_log_open(&log, path) != LXP_OK) {
        (void)fprintf(stderr, "recovery open failed\n");
        return 1;
    }
    {
        lxp_result recovery_status = lxp_log_recover(&log, replay, &state);
        if (recovery_status != LXP_OK) {
            (void)fprintf(stderr, "recovery operation failed: %d\n",
                          (int)recovery_status);
            return 1;
        }
    }
    if (state.balance != 1U || state.have_expected != 0 ||
        lxp_log_resume_sequence(&log) != 1U ||
        log.write_offset != incomplete_offset) {
        (void)fprintf(stderr, "recovery state failed: %u %d %llu %llu %llu\n",
                      state.balance, state.have_expected,
                      (unsigned long long)lxp_log_resume_sequence(&log),
                      (unsigned long long)log.write_offset,
                      (unsigned long long)incomplete_offset);
        return 1;
    }
    if (lxp_log_recover(&log, replay, &divergent) !=
        LXP_FATAL_REPLAY_DIVERGENCE) {
        (void)fprintf(stderr, "divergence was not fatal\n");
        return 1;
    }
    if (lxp_log_close(&log) != LXP_OK || unlink(path) != 0 ||
        rmdir(directory) != 0) return 1;
    return 0;
}
