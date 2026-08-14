#ifndef LAYERX_LXP_LEGACY_H
#define LAYERX_LXP_LEGACY_H

#include "layerx/lxp_codec.h"

#include <stdbool.h>
#include <stddef.h>

typedef enum lxp_legacy_write_kind {
    LXP_LEGACY_WRITE_STATE = 1,
    LXP_LEGACY_WRITE_CUSTODY = 2,
    LXP_LEGACY_WRITE_SETTLEMENT = 3,
    LXP_LEGACY_WRITE_EVENT = 4
} lxp_legacy_write_kind;

typedef struct lxp_legacy_boundaries {
    bool postgres_defines_protocol;
    bool http_is_canonical_wire;
    bool memory_challenge_is_authority;
    bool execution_reads_crossverse;
    bool development_settlement_fallback;
    bool process_local_events_authoritative;
} lxp_legacy_boundaries;

typedef struct lxp_legacy_reader {
    int descriptor;
    bool read_only;
    bool halted;
    size_t records_read;
} lxp_legacy_reader;
#define lxp_legacy_reader lxp_legacy_reader

lxp_result lxp_legacy_stream_open(
    const char *path, lxp_legacy_reader *reader);
lxp_result lxp_legacy_stream_next(
    lxp_legacy_reader *reader, lxp_arena *arena,
    lxp_byte_span *canonical_activity, bool *end_of_stream);
lxp_result lxp_legacy_readonly_guard(
    lxp_legacy_reader *reader, lxp_legacy_write_kind attempted_write);
lxp_result lxp_legacy_boundaries_check(
    const lxp_legacy_boundaries *boundaries);
lxp_result lxp_legacy_stream_close(lxp_legacy_reader *reader);

#endif
