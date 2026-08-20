#include "layerx/program.h"

/*
 * The paid-counter quickstart. It configures a fee, settles a verified 402LXP
 * receipt against that fee, advances a counter, pays the operator and emits the
 * evidence a consumer renders. Every value that crosses the guest boundary is
 * an integer, and every effect is an explicit capability the invoking activity
 * granted.
 */

enum {
    QUICKSTART_ERR_NOT_CONFIGURED = -64,
    QUICKSTART_ERR_STATE = -65,
    QUICKSTART_ERR_ASSET_MISMATCH = -66,
    QUICKSTART_ERR_UNDERPAID = -67,
    QUICKSTART_ERR_RECEIPT_FAILED = -68,
    QUICKSTART_ERR_COUNTER_OVERFLOW = -69
};

enum {
    CONFIGURED_EVENT_BYTES = 80,
    SETTLED_EVENT_BYTES = 56,
    NOTED_EVENT_BYTES = 16,
    FORWARD_CAPABILITY_BYTES = 4,
    FORWARD_INPUT_BYTES = 8
};

/*
 * The canonical entrypoint carries a single integer, so the activity picks
 * the operation with a selector. Every other selector is refused rather than
 * silently treated as the first one.
 */
enum { ENTRY_SELECTOR_COUNT = 0, ENTRY_SELECTOR_RESET = 1 };

static const char key_fee[] = "layerx.quickstart.fee";
static const char key_asset[] = "layerx.quickstart.asset";
static const char key_payee[] = "layerx.quickstart.payee";
static const char key_count[] = "layerx.quickstart.count";
static const char topic_configured[] = "layerx.quickstart.configured";
static const char topic_settled[] = "layerx.quickstart.settled";
static const char topic_noted[] = "layerx.quickstart.noted";

static lxp_program_status load_exact(const char *key, size_t key_length,
                                     uint8_t *out, size_t expected)
{
    size_t length = 0U;
    bool found = false;
    lxp_program_status status = lxp_program_storage_read(
        (const uint8_t *)key, key_length, out, expected, &length, &found);
    if (status != LXP_PROGRAM_OK) return status;
    if (!found) return QUICKSTART_ERR_NOT_CONFIGURED;
    if (length != expected) return QUICKSTART_ERR_STATE;
    return LXP_PROGRAM_OK;
}

static lxp_program_status load_counter(uint64_t *out)
{
    uint8_t encoded[8];
    size_t length = 0U;
    bool found = false;
    lxp_program_status status = lxp_program_storage_read(
        (const uint8_t *)key_count, sizeof(key_count) - 1U, encoded,
        sizeof(encoded), &length, &found);
    if (status != LXP_PROGRAM_OK) return status;
    if (!found) {
        *out = 0U;
        return LXP_PROGRAM_OK;
    }
    if (length != sizeof(encoded)) return QUICKSTART_ERR_STATE;
    *out = lxp_program_read_u64_be(encoded);
    return LXP_PROGRAM_OK;
}

LXP_PROGRAM_EXPORT("configure")
int32_t configure(int64_t fee_high, int64_t fee_low, int64_t asset_word0,
                  int64_t asset_word1, int64_t asset_word2, int64_t asset_word3,
                  int64_t payee_word0, int64_t payee_word1,
                  int64_t payee_word2, int64_t payee_word3)
{
    lxp_program_amount fee =
        lxp_program_amount_from_parts((uint64_t)fee_high, (uint64_t)fee_low);
    lxp_program_asset asset = lxp_program_asset_from_words(
        (uint64_t)asset_word0, (uint64_t)asset_word1, (uint64_t)asset_word2,
        (uint64_t)asset_word3);
    lxp_program_account payee = lxp_program_account_from_words(
        (uint64_t)payee_word0, (uint64_t)payee_word1, (uint64_t)payee_word2,
        (uint64_t)payee_word3);
    uint8_t configured[CONFIGURED_EVENT_BYTES];
    uint8_t counter[8];
    lxp_program_status status;
    size_t index;
    if (lxp_program_amount_is_zero(fee)) return LXP_PROGRAM_ERR_ZERO_AMOUNT;
    if (lxp_program_bytes32_is_zero(asset.bytes) ||
        lxp_program_bytes32_is_zero(payee.bytes))
        return LXP_PROGRAM_ERR_RESERVED_IDENTIFIER;
    lxp_program_amount_to_be(fee, configured);
    lxp_program_copy(configured + 16, asset.bytes,
                     (size_t)LXP_PROGRAM_ID_BYTES);
    lxp_program_copy(configured + 48, payee.bytes,
                     (size_t)LXP_PROGRAM_ID_BYTES);
    for (index = 0U; index < sizeof(counter); ++index) counter[index] = 0U;
    status = lxp_program_storage_write((const uint8_t *)key_fee,
                                       sizeof(key_fee) - 1U, configured,
                                       (size_t)LXP_PROGRAM_AMOUNT_BYTES);
    if (status != LXP_PROGRAM_OK) return status;
    status = lxp_program_storage_write((const uint8_t *)key_asset,
                                       sizeof(key_asset) - 1U, configured + 16,
                                       (size_t)LXP_PROGRAM_ID_BYTES);
    if (status != LXP_PROGRAM_OK) return status;
    status = lxp_program_storage_write((const uint8_t *)key_payee,
                                       sizeof(key_payee) - 1U, configured + 48,
                                       (size_t)LXP_PROGRAM_ID_BYTES);
    if (status != LXP_PROGRAM_OK) return status;
    status = lxp_program_storage_write((const uint8_t *)key_count,
                                       sizeof(key_count) - 1U, counter,
                                       sizeof(counter));
    if (status != LXP_PROGRAM_OK) return status;
    return lxp_program_event_emit((const uint8_t *)topic_configured,
                                  sizeof(topic_configured) - 1U, configured,
                                  sizeof(configured));
}

LXP_PROGRAM_EXPORT("count")
int64_t count(void)
{
    uint64_t counter = 0U;
    lxp_program_status status = load_counter(&counter);
    if (status != LXP_PROGRAM_OK) return (int64_t)status;
    if (counter > (uint64_t)INT64_MAX) return QUICKSTART_ERR_COUNTER_OVERFLOW;
    return (int64_t)counter;
}

LXP_PROGRAM_EXPORT("settle")
int64_t settle(int64_t digest_word0, int64_t digest_word1,
               int64_t digest_word2, int64_t digest_word3)
{
    lxp_program_digest digest = lxp_program_digest_from_words(
        (uint64_t)digest_word0, (uint64_t)digest_word1,
        (uint64_t)digest_word2, (uint64_t)digest_word3);
    lxp_program_receipt receipt;
    lxp_program_asset asset;
    lxp_program_account payee;
    lxp_program_amount fee;
    uint8_t fee_bytes[LXP_PROGRAM_AMOUNT_BYTES];
    uint8_t settled[SETTLED_EVENT_BYTES];
    uint64_t counter = 0U;
    lxp_program_status status;
    status = load_exact(key_fee, sizeof(key_fee) - 1U, fee_bytes,
                        sizeof(fee_bytes));
    if (status != LXP_PROGRAM_OK) return (int64_t)status;
    fee = lxp_program_amount_from_be(fee_bytes);
    status = load_exact(key_asset, sizeof(key_asset) - 1U, asset.bytes,
                        (size_t)LXP_PROGRAM_ID_BYTES);
    if (status != LXP_PROGRAM_OK) return (int64_t)status;
    status = load_exact(key_payee, sizeof(key_payee) - 1U, payee.bytes,
                        (size_t)LXP_PROGRAM_ID_BYTES);
    if (status != LXP_PROGRAM_OK) return (int64_t)status;
    status = lxp_program_receipt_read(digest, &receipt);
    if (status != LXP_PROGRAM_OK) return (int64_t)status;
    if (receipt.result_code != 0) return QUICKSTART_ERR_RECEIPT_FAILED;
    if (!lxp_program_bytes_equal(receipt.asset.bytes, asset.bytes,
                                 (size_t)LXP_PROGRAM_ID_BYTES))
        return QUICKSTART_ERR_ASSET_MISMATCH;
    if (lxp_program_amount_cmp(receipt.amount, fee) < 0)
        return QUICKSTART_ERR_UNDERPAID;
    status = load_counter(&counter);
    if (status != LXP_PROGRAM_OK) return (int64_t)status;
    if (counter == UINT64_MAX) return QUICKSTART_ERR_COUNTER_OVERFLOW;
    counter += 1U;
    lxp_program_write_u64_be(settled, counter);
    lxp_program_amount_to_be(receipt.amount, settled + 8);
    lxp_program_copy(settled + 24, digest.bytes,
                     (size_t)LXP_PROGRAM_DIGEST_BYTES);
    status = lxp_program_storage_write((const uint8_t *)key_count,
                                       sizeof(key_count) - 1U, settled, 8U);
    if (status != LXP_PROGRAM_OK) return (int64_t)status;
    status = lxp_program_transfer_402(asset, payee, fee);
    if (status != LXP_PROGRAM_OK) return (int64_t)status;
    status = lxp_program_event_emit((const uint8_t *)topic_settled,
                                    sizeof(topic_settled) - 1U, settled,
                                    sizeof(settled));
    if (status != LXP_PROGRAM_OK) return (int64_t)status;
    if (counter > (uint64_t)INT64_MAX) return QUICKSTART_ERR_COUNTER_OVERFLOW;
    return (int64_t)counter;
}

LXP_PROGRAM_EXPORT("forward")
int32_t forward(int64_t callee_word0, int64_t callee_word1,
                int64_t callee_word2, int64_t callee_word3, int64_t note)
{
    lxp_program_id callee = lxp_program_id_from_words(
        (uint64_t)callee_word0, (uint64_t)callee_word1,
        (uint64_t)callee_word2, (uint64_t)callee_word3);
    lxp_program_capability grants[2];
    lxp_program_capability_set narrowed;
    uint8_t encoded[FORWARD_CAPABILITY_BYTES];
    uint8_t input[FORWARD_INPUT_BYTES];
    size_t encoded_length = 0U;
    lxp_program_status status;
    status = lxp_program_capability_set_init(&narrowed, grants, 2U);
    if (status != LXP_PROGRAM_OK) return status;
    status = lxp_program_capability_set_push(
        &narrowed, lxp_program_capability_storage_read());
    if (status != LXP_PROGRAM_OK) return status;
    status = lxp_program_capability_set_push(
        &narrowed, lxp_program_capability_emit_event());
    if (status != LXP_PROGRAM_OK) return status;
    status = lxp_program_capability_set_encode(&narrowed, encoded,
                                               sizeof(encoded),
                                               &encoded_length);
    if (status != LXP_PROGRAM_OK) return status;
    lxp_program_write_u64_be(input, (uint64_t)note);
    return lxp_program_call(callee, input, sizeof(input), encoded,
                            encoded_length);
}

LXP_PROGRAM_EXPORT("reset")
int32_t reset(void)
{
    return lxp_program_storage_delete((const uint8_t *)key_count,
                                      sizeof(key_count) - 1U);
}

LXP_PROGRAM_EXPORT(LXP_PROGRAM_ENTRYPOINT)
int64_t layerx_main(int64_t selector)
{
    switch (selector) {
    case ENTRY_SELECTOR_COUNT:
        return count();
    case ENTRY_SELECTOR_RESET:
        return (int64_t)reset();
    default:
        return (int64_t)LXP_PROGRAM_ERR_INVALID;
    }
}

LXP_PROGRAM_EXPORT(LXP_PROGRAM_CALL_RESERVE_EXPORT)
int32_t layerx_reserve(int32_t length)
{
    return lxp_program_reserve_call_input(length);
}

/*
 * The callee half of forward(). It holds only the storage-read and emit-event
 * grants forward() narrows to, so it reads the counter and files the caller's
 * note as evidence without ever reaching for an authority it was not handed.
 */
LXP_PROGRAM_EXPORT(LXP_PROGRAM_CALL_ENTRY_EXPORT)
int32_t layerx_call(int32_t input_pointer, int32_t input_length)
{
    const uint8_t *input = NULL;
    size_t length = 0U;
    uint8_t noted[NOTED_EVENT_BYTES];
    uint64_t counter = 0U;
    lxp_program_status status =
        lxp_program_call_input(input_pointer, input_length, &input, &length);
    if (status != LXP_PROGRAM_OK) return status;
    if (length != (size_t)FORWARD_INPUT_BYTES) return LXP_PROGRAM_ERR_INVALID;
    status = load_counter(&counter);
    if (status != LXP_PROGRAM_OK) return status;
    lxp_program_write_u64_be(noted, counter);
    lxp_program_copy(noted + 8, input, (size_t)FORWARD_INPUT_BYTES);
    return lxp_program_event_emit((const uint8_t *)topic_noted,
                                  sizeof(topic_noted) - 1U, noted,
                                  sizeof(noted));
}
