#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_shadow.h"
#include "layerx/lxp_hash.h"

#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static lxp_result outcome(
    void *context, lxp_byte_span activity,
    uint64_t batch_timestamp_ms, lxp_shadow_outcome *result)
{
    bool candidate = context != NULL;
    if (activity.length != 1U || result == NULL) return LXP_ERR_NON_CANONICAL;
    if (lxp_hash_activity_id(
            activity.bytes, activity.length, result->activity_id) != LXP_OK)
        return LXP_ERR_BAD_SIGNATURE;
    result->accepted = true;
    result->global_sequence = activity.bytes[0];
    result->resulting_balance[0] = (uint8_t)(100U - activity.bytes[0]);
    result->fee_charged = (lxp_u128){0U, 1U};
    result->result_code = LXP_OK;
    result->canonical_receipt[0] = activity.bytes[0];
    result->canonical_receipt_length = 1U;
    result->batch_timestamp_ms = batch_timestamp_ms;
    if (candidate && activity.bytes[0] == 2U) {
        result->fee_charged.lo = 2U;
        result->canonical_receipt[0] = 3U;
    }
    return LXP_OK;
}

int main(void)
{
    static uint8_t arena_bytes[4096U];
    static lxp_shadow_comparison comparison;
    static lxp_shadow_report_record report;
    uint8_t stream[] = {
        0U, 0U, 0U, 1U, 1U,
        0U, 0U, 0U, 1U, 2U
    };
    char path[] = "/tmp/lxp-shadow-stream-XXXXXX";
    int descriptor = mkstemp(path);
    lxp_legacy_reader reader;
    lxp_arena arena;
    lxp_shadow_outcome expected;
    lxp_shadow_outcome produced;
    size_t i;

    if (descriptor < 0 ||
        write(descriptor, stream, sizeof(stream)) != (ssize_t)sizeof(stream) ||
        close(descriptor) != 0 ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_legacy_stream_open(path, &reader) != LXP_OK ||
        lxp_shadow_harness(
            &reader, &arena, 1000U, outcome, NULL,
            outcome, &reader, &comparison) != LXP_FATAL_REPLAY_DIVERGENCE ||
        comparison.activities_compared != 2U ||
        comparison.divergence_count != 2U ||
        comparison.divergences[0].kind != LXP_SHADOW_FEE ||
        comparison.divergences[1].kind != LXP_SHADOW_RECEIPT ||
        lxp_shadow_report(&comparison, &report) != LXP_OK ||
        !report.promotion_blocked ||
        report.category_counts[LXP_SHADOW_FEE - 1U] != 1U ||
        report.category_counts[LXP_SHADOW_RECEIPT - 1U] != 1U)
        return 1;
    for (i = 0U; i < comparison.divergence_count; ++i) {
        comparison.divergences[i].intentional = true;
        comparison.divergences[i].specification_id[0] = (uint8_t)(i + 1U);
    }
    if (lxp_shadow_report(&comparison, &report) != LXP_OK ||
        report.promotion_blocked)
        return 1;
    (void)memset(&expected, 0, sizeof(expected));
    (void)memset(&produced, 0, sizeof(produced));
    expected.activity_id[0] = 4U;
    produced.activity_id[0] = 4U;
    expected.batch_timestamp_ms = 1000U;
    produced.batch_timestamp_ms = 1000U;
    produced.used_wall_clock = true;
    (void)memset(&comparison, 0, sizeof(comparison));
    if (lxp_shadow_compare_outcome(
            &expected, &produced, 1000U,
            &comparison) != LXP_FATAL_REPLAY_DIVERGENCE ||
        comparison.divergence_count != 1U ||
        comparison.divergences[0].kind != LXP_SHADOW_TIME_SOURCE ||
        lxp_legacy_stream_close(&reader) != LXP_OK || unlink(path) != 0)
        return 1;
    return 0;
}
