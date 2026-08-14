#ifndef LAYERX_LXP_SHADOW_H
#define LAYERX_LXP_SHADOW_H

#include "layerx/lxp_legacy.h"
#include "layerx/lxp_u128.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum {
    LXP_SHADOW_MAX_RECEIPT_BYTES = 4096,
    LXP_SHADOW_MAX_DIVERGENCES = 128,
    LXP_SHADOW_VALUE_BYTES = 4096,
    LXP_SHADOW_CATEGORY_COUNT = 7
};

typedef enum lxp_shadow_divergence_kind {
    LXP_SHADOW_ACCEPTANCE = 1,
    LXP_SHADOW_ORDERING = 2,
    LXP_SHADOW_BALANCE = 3,
    LXP_SHADOW_FEE = 4,
    LXP_SHADOW_RESULT_CODE = 5,
    LXP_SHADOW_RECEIPT = 6,
    LXP_SHADOW_TIME_SOURCE = 7
} lxp_shadow_divergence_kind;

typedef struct lxp_shadow_outcome {
    uint8_t activity_id[32];
    bool accepted;
    uint64_t global_sequence;
    uint8_t resulting_balance[32];
    lxp_u128 fee_charged;
    lxp_result result_code;
    uint8_t canonical_receipt[LXP_SHADOW_MAX_RECEIPT_BYTES];
    size_t canonical_receipt_length;
    uint64_t batch_timestamp_ms;
    bool used_wall_clock;
} lxp_shadow_outcome;

typedef struct lxp_shadow_divergence {
    uint8_t activity_id[32];
    lxp_shadow_divergence_kind kind;
    uint8_t expected[LXP_SHADOW_VALUE_BYTES];
    size_t expected_length;
    uint8_t produced[LXP_SHADOW_VALUE_BYTES];
    size_t produced_length;
    bool intentional;
    uint8_t specification_id[32];
} lxp_shadow_divergence;

typedef struct lxp_shadow_comparison {
    size_t activities_compared;
    lxp_shadow_divergence divergences[LXP_SHADOW_MAX_DIVERGENCES];
    size_t divergence_count;
} lxp_shadow_comparison;

typedef struct lxp_shadow_report_record {
    size_t activities_compared;
    size_t divergence_count;
    size_t category_counts[LXP_SHADOW_CATEGORY_COUNT];
    lxp_shadow_divergence divergences[LXP_SHADOW_MAX_DIVERGENCES];
    bool promotion_blocked;
} lxp_shadow_report_record;

typedef lxp_result (*lxp_shadow_outcome_fn)(
    void *context, lxp_byte_span canonical_activity,
    uint64_t batch_timestamp_ms, lxp_shadow_outcome *outcome);

lxp_result lxp_shadow_divergence_record(
    lxp_shadow_comparison *comparison,
    lxp_shadow_divergence_kind kind,
    const uint8_t activity_id[32],
    const void *expected, size_t expected_length,
    const void *produced, size_t produced_length);
lxp_result lxp_shadow_compare_outcome(
    const lxp_shadow_outcome *expected,
    const lxp_shadow_outcome *produced,
    uint64_t batch_timestamp_ms,
    lxp_shadow_comparison *comparison);
lxp_result lxp_shadow_harness(
    lxp_legacy_reader *reader, lxp_arena *arena,
    uint64_t batch_timestamp_ms,
    lxp_shadow_outcome_fn legacy_outcome, void *legacy_context,
    lxp_shadow_outcome_fn candidate_outcome, void *candidate_context,
    lxp_shadow_comparison *comparison);
lxp_result lxp_shadow_report(
    const lxp_shadow_comparison *comparison,
    lxp_shadow_report_record *report);

#endif
