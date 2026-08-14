#ifndef LAYERX_LXP_REPLAY_FIXTURE_H
#define LAYERX_LXP_REPLAY_FIXTURE_H

#include "layerx/lxp_arena.h"
#include "layerx/lxp_codec.h"

#include <stddef.h>
#include <stdint.h>

enum {
    LXP_REPLAY_FIXTURE_MAX_RECORDS = 256,
    LXP_REPLAY_FIXTURE_RECEIPT_BYTES = 106,
    LXP_REPLAY_FIXTURE_EVENT_BYTES = 36
};

typedef struct lxp_replay_fixture_record {
    uint64_t global_sequence;
    uint8_t batch_boundary;
    lxp_byte_span canonical_activity;
    uint8_t expected_state_root[32];
    lxp_byte_span expected_receipt;
    lxp_byte_span expected_event;
    uint8_t expected_batch_root[32];
} lxp_replay_fixture_record;

typedef struct lxp_replay_fixture {
    lxp_replay_fixture_record *records;
    size_t record_count;
    uint8_t expected_terminal_root[32];
    uint8_t expected_digest[32];
} lxp_replay_fixture;

lxp_result lxp_replay_fixture_load(const char *path, lxp_arena *arena,
                                   lxp_replay_fixture *fixture);
lxp_result lxp_replay_digest(const lxp_replay_fixture *fixture,
                             uint8_t digest[32], uint8_t terminal_root[32],
                             uint64_t *first_divergent_sequence);
lxp_result lxp_replay_crossarch_case(const char *path, lxp_arena *arena,
                                     uint8_t digest[32],
                                     uint64_t *first_divergent_sequence);

#endif
