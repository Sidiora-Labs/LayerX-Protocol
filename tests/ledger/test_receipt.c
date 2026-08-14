#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_receipt.h"
#include "layerx/lxp_storage.h"

#include <openssl/evp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static uint8_t arena_bytes[LXP_MAX_ACTIVITY_BYTES + 4096U];
static uint8_t log_body[LXP_MAX_ACTIVITY_BYTES];

int main(void)
{
    static const uint8_t seed[32] = { 3U };
    uint8_t public_key[32];
    size_t public_length = 32U;
    EVP_PKEY *key = EVP_PKEY_new_raw_private_key(EVP_PKEY_ED25519, NULL,
                                                  seed, 32U);
    lxp_ledger_receipt_input input;
    lxp_receipt receipt;
    lxp_receipt cached;
    lxp_arena arena;
    lxp_byte_span encoded;
    size_t mark;
    char directory[] = "/tmp/lxp-ledger-receipt-XXXXXX";
    char path[128];
    lxp_log log;
    lxp_log_record_header header;
    uint64_t durable_length;

    if (key == NULL ||
        EVP_PKEY_get_raw_public_key(key, public_key, &public_length) != 1 ||
        public_length != 32U ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        mkdtemp(directory) == NULL ||
        lxp_log_segment_create(&log, directory, 0U,
                               LXP_MAX_ACTIVITY_BYTES + 4096U) != LXP_OK)
        return 1;
    EVP_PKEY_free(key);
    if (snprintf(path, sizeof(path), "%s/%020u.lxp", directory, 0U) < 0)
        return 1;
    (void)memset(&input, 0, sizeof(input));
    input.transaction_id[0] = 1U;
    input.operation = 1U;
    input.global_sequence = 9U;
    input.asset[0] = 2U;
    input.amount = (lxp_u128){ 0U, 25U };
    input.from[0] = 3U;
    input.from_balance_before = (lxp_u128){ 0U, 100U };
    input.from_balance_after = (lxp_u128){ 0U, 75U };
    input.from_sequence = 7U;
    input.to[0] = 4U;
    input.to_balance_before = (lxp_u128){ 0U, 10U };
    input.to_balance_after = (lxp_u128){ 0U, 35U };
    input.transfer_set_root[0] = 5U;
    input.authorization_hash[0] = 6U;
    input.context_hash[0] = 7U;
    input.previous_state_root[0] = 8U;
    input.resulting_state_root[0] = 9U;
    input.batch_id[0] = 10U;
    input.timestamp = 1234U;
    input.leg_count = 1U;
    if (lxp_balance_writer_guard(false) != LXP_ERR_BALANCE_BYPASS ||
        lxp_balance_writer_guard(true) != LXP_OK ||
        lxp_ledger_receipt_issue(&receipt, &input, seed, &arena, &log) != LXP_OK ||
        lxp_receipt_verify(&receipt, public_key, &arena) != LXP_OK)
        return 1;
    durable_length = log.write_offset;
    mark = lxp_arena_mark(&arena);
    if (lxp_receipt_encode(&receipt, true, &arena, &encoded) != LXP_OK ||
        lxp_log_read(&log, 0U, &header, log_body, sizeof(log_body)) != LXP_OK ||
        header.record_kind != LXP_LOG_RECEIPT || header.global_sequence != 9U ||
        header.body_length != encoded.length ||
        memcmp(log_body, encoded.bytes, encoded.length) != 0) return 1;
    cached = receipt;
    if (memcmp(&cached, &receipt, sizeof(receipt)) != 0) return 1;
    (void)lxp_arena_reset(&arena, mark);
    cached.sequencer_signature[0] ^= 1U;
    if (lxp_receipt_verify(&cached, public_key, &arena) != LXP_ERR_BAD_SIGNATURE)
        return 1;
    input.from_balance_after.lo = 74U;
    if (lxp_ledger_receipt_issue(&cached, &input, seed, &arena, &log) !=
            LXP_FATAL_INVARIANT || log.write_offset != durable_length)
        return 1;
    if (lxp_log_close(&log) != LXP_OK || unlink(path) != 0 ||
        rmdir(directory) != 0) return 1;
    return 0;
}
