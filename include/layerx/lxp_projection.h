#ifndef LAYERX_LXP_PROJECTION_H
#define LAYERX_LXP_PROJECTION_H

#include "layerx/lxp_result.h"
#include "layerx/lxp_storage.h"

#include <pthread.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct lxp_projection {
    void *database;
    pthread_t owner;
    bool stale;
} lxp_projection;

typedef struct lxp_projection_record {
    uint8_t activity_id[32];
    uint8_t idempotency_key[32];
    uint8_t account_id[32];
    uint8_t asset_id[32];
    uint8_t amount[16];
    const uint8_t *receipt;
    size_t receipt_length;
    int32_t result_code;
} lxp_projection_record;

lxp_result lxp_projection_open(lxp_projection *projection,
                               const char *database_path,
                               const char *migration_path);
lxp_result lxp_projection_apply(lxp_projection *projection,
                                uint64_t global_sequence,
                                const lxp_projection_record *record);
lxp_result lxp_projection_watermark(lxp_projection *projection,
                                    uint64_t *watermark, bool *has_watermark);
void lxp_projection_mark_stale(lxp_projection *projection);
lxp_result lxp_projection_close(lxp_projection *projection);
lxp_result lxp_projection_record_encode(const lxp_projection_record *record,
                                        uint8_t *output, size_t capacity,
                                        size_t *encoded_length);
lxp_result lxp_projection_record_decode(const uint8_t *input, size_t length,
                                        lxp_projection_record *record);
lxp_result lxp_projection_drop_all(lxp_projection *projection,
                                   const char *migration_path);
lxp_result lxp_projection_rebuild(lxp_projection *projection, lxp_log *log,
                                  const char *migration_path);
lxp_result lxp_log_replay_range(const lxp_log *log, uint64_t start_offset,
                                uint64_t end_offset, lxp_log_replay_fn replay,
                                void *context);

#endif
