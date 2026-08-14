#include "layerx/lxp_guarantor.h"

#include "layerx/lxp_crypto.h"

#include <string.h>

enum { LXP_DISSENT_MESSAGE_BYTES = 121 };

static void store_u32(uint8_t out[4], uint32_t value)
{
    out[0] = (uint8_t)(value >> 24U);
    out[1] = (uint8_t)(value >> 16U);
    out[2] = (uint8_t)(value >> 8U);
    out[3] = (uint8_t)value;
}

static void store_u64(uint8_t out[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i) out[7U - i] = (uint8_t)(value >> (i * 8U));
}

static int span_equal(lxp_byte_span left, lxp_byte_span right)
{
    return left.length == right.length &&
        lxp_ct_memcmp(left.bytes, right.bytes, left.length) == 0;
}

static lxp_result hash_span(lxp_byte_span span, uint8_t hash[32])
{
    if (span.bytes == NULL && span.length != 0U)
        return LXP_ERR_NON_CANONICAL;
    return lxp_hash_sha256(span.bytes, span.length, hash);
}

static lxp_result set_divergence(
    lxp_guarantor_divergence *divergence, uint64_t batch_number,
    uint64_t sequence, lxp_guarantor_divergence_component component,
    lxp_byte_span expected, lxp_byte_span produced)
{
    divergence->batch_number = batch_number;
    divergence->global_sequence = sequence;
    divergence->component = component;
    if (hash_span(expected, divergence->expected_hash) != LXP_OK ||
        hash_span(produced, divergence->produced_hash) != LXP_OK)
        return LXP_ERR_NON_CANONICAL;
    return LXP_FATAL_REPLAY_DIVERGENCE;
}

lxp_result lxp_guarantor_first_divergence(
    uint64_t batch_number, uint64_t first_sequence,
    const lxp_replay_batch_result *published,
    const lxp_replay_batch_result *recomputed,
    lxp_guarantor_divergence *divergence)
{
    size_t i;
    if (published == NULL || recomputed == NULL || divergence == NULL ||
        published->outputs == NULL || recomputed->outputs == NULL)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(divergence, 0, sizeof(*divergence));
    if (published->activity_count != recomputed->activity_count)
        return set_divergence(divergence, batch_number, first_sequence,
            LXP_GUARANTOR_DIVERGENCE_RECEIPT,
            published->canonical_receipt_section,
            recomputed->canonical_receipt_section);
    for (i = 0U; i < published->activity_count; ++i) {
        const lxp_replay_activity_output *expected = &published->outputs[i];
        const lxp_replay_activity_output *produced = &recomputed->outputs[i];
        uint8_t expected_scalar[16];
        uint8_t produced_scalar[16];
        if (expected->result_code != produced->result_code) {
            store_u32(expected_scalar, (uint32_t)expected->result_code);
            store_u32(produced_scalar, (uint32_t)produced->result_code);
            return set_divergence(divergence, batch_number,
                first_sequence + i, LXP_GUARANTOR_DIVERGENCE_RESULT_CODE,
                (lxp_byte_span){expected_scalar, 4U},
                (lxp_byte_span){produced_scalar, 4U});
        }
        if (lxp_u128_cmp(expected->fee_charged, produced->fee_charged) != 0) {
            (void)lxp_u128_to_be(expected->fee_charged, expected_scalar);
            (void)lxp_u128_to_be(produced->fee_charged, produced_scalar);
            return set_divergence(divergence, batch_number,
                first_sequence + i, LXP_GUARANTOR_DIVERGENCE_FEE,
                (lxp_byte_span){expected_scalar, 16U},
                (lxp_byte_span){produced_scalar, 16U});
        }
#define CHECK_SPAN(field, kind) do { \
    if (!span_equal(expected->field, produced->field)) \
        return set_divergence(divergence, batch_number, first_sequence + i, \
            (kind), expected->field, produced->field); \
} while (0)
        CHECK_SPAN(effects, LXP_GUARANTOR_DIVERGENCE_EFFECTS);
        CHECK_SPAN(resulting_balance, LXP_GUARANTOR_DIVERGENCE_BALANCE);
        CHECK_SPAN(canonical_receipt, LXP_GUARANTOR_DIVERGENCE_RECEIPT);
        CHECK_SPAN(canonical_events, LXP_GUARANTOR_DIVERGENCE_EVENTS);
#undef CHECK_SPAN
        if (lxp_ct_memcmp(expected->resulting_state_root,
                          produced->resulting_state_root, 32U) != 0)
            return set_divergence(divergence, batch_number,
                first_sequence + i, LXP_GUARANTOR_DIVERGENCE_STATE_ROOT,
                (lxp_byte_span){expected->resulting_state_root, 32U},
                (lxp_byte_span){produced->resulting_state_root, 32U});
    }
    return LXP_OK;
}

static void dissent_message(const lxp_guarantor_dissent_record *dissent,
                            uint8_t message[LXP_DISSENT_MESSAGE_BYTES])
{
    (void)memcpy(message, dissent->guarantor_id, 32U);
    store_u64(message + 32U, dissent->epoch);
    store_u64(message + 40U, dissent->divergence.batch_number);
    store_u64(message + 48U, dissent->divergence.global_sequence);
    message[56] = (uint8_t)dissent->divergence.component;
    (void)memcpy(message + 57U, dissent->divergence.expected_hash, 32U);
    (void)memcpy(message + 89U, dissent->divergence.produced_hash, 32U);
}

lxp_result lxp_guarantor_dissent(
    const lxp_guarantor_ctx *ctx, uint64_t epoch,
    const lxp_guarantor_divergence *divergence,
    lxp_guarantor_dissent_record *dissent)
{
    uint8_t message[LXP_DISSENT_MESSAGE_BYTES];
    if (ctx == NULL || divergence == NULL || dissent == NULL || epoch == 0U ||
        divergence->component < LXP_GUARANTOR_DIVERGENCE_SIGNATURE ||
        divergence->component > LXP_GUARANTOR_DIVERGENCE_EVENTS)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(dissent, 0, sizeof(*dissent));
    (void)memcpy(dissent->guarantor_id, ctx->guarantor_id, 32U);
    dissent->epoch = epoch;
    dissent->divergence = *divergence;
    dissent_message(dissent, message);
    return lxp_secp256k1_sign(ctx->paxeer_private_key,
                              LXP_DOMAIN_CHECKPOINT_CERTIFICATE,
                              message, sizeof(message), dissent->signature);
}

lxp_result lxp_guarantor_dissent_verify(
    const lxp_guarantor_dissent_record *dissent,
    const uint8_t public_key[33])
{
    uint8_t message[LXP_DISSENT_MESSAGE_BYTES];
    if (dissent == NULL || public_key == NULL) return LXP_ERR_NON_CANONICAL;
    dissent_message(dissent, message);
    return lxp_secp256k1_verify(public_key, 33U, dissent->signature,
                                LXP_DOMAIN_CHECKPOINT_CERTIFICATE,
                                message, sizeof(message));
}

lxp_result lxp_guarantor_withhold(
    lxp_guarantor_ctx *ctx, uint64_t epoch,
    const lxp_guarantor_divergence *divergence,
    lxp_guarantor_dissent_record *dissent)
{
    lxp_result status;
    if (ctx == NULL || ctx->publish_dissent == NULL)
        return LXP_ERR_NON_CANONICAL;
    ctx->ready_to_sign = false;
    ctx->attestation_halted_epoch = epoch;
    status = lxp_guarantor_dissent(ctx, epoch, divergence, dissent);
    if (status == LXP_OK)
        status = ctx->publish_dissent(ctx->dissent_context, dissent);
    return status == LXP_OK ? LXP_FATAL_REPLAY_DIVERGENCE : status;
}
