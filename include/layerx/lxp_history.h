#ifndef LAYERX_LXP_HISTORY_H
#define LAYERX_LXP_HISTORY_H

#include "layerx/lxp_arena.h"
#include "layerx/lxp_codec.h"
#include "layerx/lxp_storage.h"

#include <stddef.h>
#include <stdint.h>

enum {
    LXP_HISTORY_MAX_RESULTS = 4096
};

typedef enum lxp_history_query_kind {
    LXP_HISTORY_BY_CHECKPOINT_ID = 1,
    LXP_HISTORY_BY_BATCH_NUMBER = 2,
    LXP_HISTORY_BY_SEQUENCE_RANGE = 3,
    LXP_HISTORY_BY_ACTIVITY_ID = 4
} lxp_history_query_kind;

typedef enum lxp_receipt_query_kind {
    LXP_RECEIPT_BY_TRANSACTION_ID = 1,
    LXP_RECEIPT_BY_IDEMPOTENCY_KEY = 2,
    LXP_RECEIPT_BY_GLOBAL_SEQUENCE = 3
} lxp_receipt_query_kind;

typedef struct lxp_history_query_spec {
    lxp_history_query_kind kind;
    uint8_t identifier[32];
    uint64_t batch_number;
    uint64_t first_sequence;
    uint64_t last_sequence;
    size_t maximum_response_bytes;
} lxp_history_query_spec;

typedef struct lxp_history_item {
    uint8_t record_kind;
    uint64_t global_sequence;
    lxp_byte_span canonical_bytes;
} lxp_history_item;

typedef struct lxp_history_result {
    lxp_history_item *items;
    size_t count;
    size_t total_bytes;
} lxp_history_result;

typedef struct lxp_receipt_query {
    lxp_receipt_query_kind kind;
    uint8_t identifier[32];
    uint64_t global_sequence;
    size_t maximum_response_bytes;
} lxp_receipt_query;

typedef struct lxp_history {
    const lxp_log *log;
    void *database;
} lxp_history;

typedef lxp_result (*lxp_history_send_fn)(
    void *context, uint8_t record_kind, uint64_t global_sequence,
    const uint8_t *canonical_bytes, size_t length);

lxp_result lxp_history_open(lxp_history *history, const lxp_log *log,
                            const char *database_path,
                            const char *migration_path);
lxp_result lxp_history_close(lxp_history *history);
lxp_result lxp_history_index_rebuild(lxp_history *history);
lxp_result lxp_history_query(const lxp_history *history,
                             const lxp_history_query_spec *query,
                             lxp_arena *arena, lxp_history_result *result);
lxp_result lxp_history_serve_range(const lxp_history *history,
                                   uint64_t first_sequence,
                                   uint64_t last_sequence,
                                   size_t maximum_response_bytes,
                                   lxp_history_send_fn send,
                                   void *send_context, lxp_arena *arena);
lxp_result lxp_receipt_lookup(const lxp_history *history,
                              const lxp_receipt_query *query,
                              lxp_arena *arena, lxp_byte_span *receipt);
#endif
