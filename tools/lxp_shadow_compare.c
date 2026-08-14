#include "layerx/lxp_shadow.h"

#include "layerx/lxp_crypto.h"

#include <string.h>

static void u64_bytes(uint64_t value, uint8_t output[8])
{
    size_t i;
    for (i = 0U; i < 8U; ++i)
        output[7U - i] = (uint8_t)(value >> (i * 8U));
}

lxp_result lxp_shadow_divergence_record(
    lxp_shadow_comparison *comparison,
    lxp_shadow_divergence_kind kind,
    const uint8_t activity_id[32],
    const void *expected, size_t expected_length,
    const void *produced, size_t produced_length)
{
    lxp_shadow_divergence *record;
    if (comparison == NULL || activity_id == NULL ||
        kind < LXP_SHADOW_ACCEPTANCE || kind > LXP_SHADOW_TIME_SOURCE ||
        (expected == NULL && expected_length != 0U) ||
        (produced == NULL && produced_length != 0U) ||
        expected_length > LXP_SHADOW_VALUE_BYTES ||
        produced_length > LXP_SHADOW_VALUE_BYTES ||
        comparison->divergence_count == LXP_SHADOW_MAX_DIVERGENCES)
        return LXP_ERR_LENGTH_LIMIT;
    record = &comparison->divergences[comparison->divergence_count++];
    (void)memset(record, 0, sizeof(*record));
    (void)memcpy(record->activity_id, activity_id, 32U);
    record->kind = kind;
    if (expected_length != 0U)
        (void)memcpy(record->expected, expected, expected_length);
    record->expected_length = expected_length;
    if (produced_length != 0U)
        (void)memcpy(record->produced, produced, produced_length);
    record->produced_length = produced_length;
    return LXP_OK;
}

static lxp_result compare_value(
    lxp_shadow_comparison *comparison, lxp_shadow_divergence_kind kind,
    const uint8_t activity_id[32], const void *expected,
    size_t expected_length, const void *produced, size_t produced_length)
{
    if (expected_length == produced_length &&
        lxp_ct_memcmp(expected, produced, expected_length) == 0)
        return LXP_OK;
    return lxp_shadow_divergence_record(
        comparison, kind, activity_id, expected, expected_length,
        produced, produced_length);
}

lxp_result lxp_shadow_compare_outcome(
    const lxp_shadow_outcome *expected,
    const lxp_shadow_outcome *produced,
    uint64_t batch_timestamp_ms,
    lxp_shadow_comparison *comparison)
{
    uint8_t expected_u64[8];
    uint8_t produced_u64[8];
    uint8_t expected_u128[16];
    uint8_t produced_u128[16];
    uint8_t expected_i32[4];
    uint8_t produced_i32[4];
    uint8_t expected_bool;
    uint8_t produced_bool;
    size_t before;
    lxp_result status = LXP_OK;
    uint32_t expected_code;
    uint32_t produced_code;
    if (expected == NULL || produced == NULL || comparison == NULL ||
        batch_timestamp_ms == 0U ||
        expected->canonical_receipt_length > LXP_SHADOW_MAX_RECEIPT_BYTES ||
        produced->canonical_receipt_length > LXP_SHADOW_MAX_RECEIPT_BYTES ||
        lxp_ct_memcmp(expected->activity_id,
                      produced->activity_id, 32U) != 0)
        return LXP_ERR_NON_CANONICAL;
    before = comparison->divergence_count;
    expected_bool = expected->accepted ? 1U : 0U;
    produced_bool = produced->accepted ? 1U : 0U;
    status = compare_value(comparison, LXP_SHADOW_ACCEPTANCE,
        expected->activity_id, &expected_bool, 1U, &produced_bool, 1U);
    u64_bytes(expected->global_sequence, expected_u64);
    u64_bytes(produced->global_sequence, produced_u64);
    if (status == LXP_OK) status = compare_value(
        comparison, LXP_SHADOW_ORDERING, expected->activity_id,
        expected_u64, 8U, produced_u64, 8U);
    if (status == LXP_OK) status = compare_value(
        comparison, LXP_SHADOW_BALANCE, expected->activity_id,
        expected->resulting_balance, 32U,
        produced->resulting_balance, 32U);
    if (status == LXP_OK)
        status = lxp_u128_to_be(expected->fee_charged, expected_u128);
    if (status == LXP_OK)
        status = lxp_u128_to_be(produced->fee_charged, produced_u128);
    if (status == LXP_OK) status = compare_value(
        comparison, LXP_SHADOW_FEE, expected->activity_id,
        expected_u128, 16U, produced_u128, 16U);
    expected_code = (uint32_t)(int32_t)expected->result_code;
    produced_code = (uint32_t)(int32_t)produced->result_code;
    expected_i32[0] = (uint8_t)(expected_code >> 24U);
    expected_i32[1] = (uint8_t)(expected_code >> 16U);
    expected_i32[2] = (uint8_t)(expected_code >> 8U);
    expected_i32[3] = (uint8_t)expected_code;
    produced_i32[0] = (uint8_t)(produced_code >> 24U);
    produced_i32[1] = (uint8_t)(produced_code >> 16U);
    produced_i32[2] = (uint8_t)(produced_code >> 8U);
    produced_i32[3] = (uint8_t)produced_code;
    if (status == LXP_OK) status = compare_value(
        comparison, LXP_SHADOW_RESULT_CODE, expected->activity_id,
        expected_i32, 4U, produced_i32, 4U);
    if (status == LXP_OK) status = compare_value(
        comparison, LXP_SHADOW_RECEIPT, expected->activity_id,
        expected->canonical_receipt, expected->canonical_receipt_length,
        produced->canonical_receipt, produced->canonical_receipt_length);
    u64_bytes(batch_timestamp_ms, expected_u64);
    u64_bytes(produced->batch_timestamp_ms, produced_u64);
    if (status == LXP_OK &&
        (expected->used_wall_clock || produced->used_wall_clock ||
         expected->batch_timestamp_ms != batch_timestamp_ms ||
         produced->batch_timestamp_ms != batch_timestamp_ms))
        status = lxp_shadow_divergence_record(
            comparison, LXP_SHADOW_TIME_SOURCE, expected->activity_id,
            expected_u64, 8U, produced_u64, 8U);
    if (status != LXP_OK) return status;
    ++comparison->activities_compared;
    return comparison->divergence_count == before ?
        LXP_OK : LXP_FATAL_REPLAY_DIVERGENCE;
}

lxp_result lxp_shadow_harness(
    lxp_legacy_reader *reader, lxp_arena *arena,
    uint64_t batch_timestamp_ms,
    lxp_shadow_outcome_fn legacy_outcome, void *legacy_context,
    lxp_shadow_outcome_fn candidate_outcome, void *candidate_context,
    lxp_shadow_comparison *comparison)
{
    lxp_byte_span activity;
    bool end = false;
    lxp_result status;
    if (reader == NULL || arena == NULL || batch_timestamp_ms == 0U ||
        legacy_outcome == NULL || candidate_outcome == NULL ||
        comparison == NULL)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(comparison, 0, sizeof(*comparison));
    while (!end) {
        lxp_shadow_outcome expected;
        lxp_shadow_outcome produced;
        size_t mark = lxp_arena_mark(arena);
        status = lxp_legacy_stream_next(reader, arena, &activity, &end);
        if (status != LXP_OK || end) {
            (void)lxp_arena_reset(arena, mark);
            return status;
        }
        (void)memset(&expected, 0, sizeof(expected));
        (void)memset(&produced, 0, sizeof(produced));
        status = legacy_outcome(
            legacy_context, activity, batch_timestamp_ms, &expected);
        if (status == LXP_OK) status = candidate_outcome(
            candidate_context, activity, batch_timestamp_ms, &produced);
        if (status == LXP_OK) status = lxp_shadow_compare_outcome(
            &expected, &produced, batch_timestamp_ms, comparison);
        (void)lxp_arena_reset(arena, mark);
        if (status != LXP_OK) return status;
    }
    return LXP_OK;
}

lxp_result lxp_shadow_report(
    const lxp_shadow_comparison *comparison,
    lxp_shadow_report_record *report)
{
    size_t i;
    if (comparison == NULL || report == NULL ||
        comparison->divergence_count > LXP_SHADOW_MAX_DIVERGENCES)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(report, 0, sizeof(*report));
    report->activities_compared = comparison->activities_compared;
    report->divergence_count = comparison->divergence_count;
    for (i = 0U; i < comparison->divergence_count; ++i) {
        report->divergences[i] = comparison->divergences[i];
        ++report->category_counts[comparison->divergences[i].kind - 1U];
        if (!comparison->divergences[i].intentional ||
            lxp_ct_is_zero(
                comparison->divergences[i].specification_id, 32U))
            report->promotion_blocked = true;
    }
    return LXP_OK;
}
