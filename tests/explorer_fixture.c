#include "layerx/lxp_batch.h"
#include "layerx/lx_asset.h"
#include "layerx/lxp_hash.h"
#include "layerx/lxp_merkle.h"
#include "layerx/lxp_receipt.h"

#include <openssl/evp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

enum {
    EXPLORER_FIXTURE_VERSION = 1,
    EXPLORER_PROOF_VERSION = 1,
    EXPLORER_PROOF_PREFIX_BYTES = 10
};

static int public_key_for(
    const uint8_t private_key[32], uint8_t public_key[32])
{
    EVP_PKEY *key = EVP_PKEY_new_raw_private_key(
        EVP_PKEY_ED25519, NULL, private_key, 32U);
    size_t length = 32U;
    int ok = key != NULL && EVP_PKEY_get_raw_public_key(
        key, public_key, &length) == 1 && length == 32U;
    EVP_PKEY_free(key);
    return ok ? 0 : 1;
}

static int write_bytes(const void *bytes, size_t length)
{
    return length == 0U || fwrite(bytes, 1U, length, stdout) == length ? 0 : 1;
}

static int write_u32(uint32_t value)
{
    const uint8_t bytes[4] = {
        (uint8_t)(value >> 24U),
        (uint8_t)(value >> 16U),
        (uint8_t)(value >> 8U),
        (uint8_t)value
    };
    return write_bytes(bytes, sizeof(bytes));
}

static int write_u64(uint64_t value)
{
    const uint8_t bytes[8] = {
        (uint8_t)(value >> 56U),
        (uint8_t)(value >> 48U),
        (uint8_t)(value >> 40U),
        (uint8_t)(value >> 32U),
        (uint8_t)(value >> 24U),
        (uint8_t)(value >> 16U),
        (uint8_t)(value >> 8U),
        (uint8_t)value
    };
    return write_bytes(bytes, sizeof(bytes));
}

static int write_sized(const uint8_t *bytes, size_t length)
{
    if (length > UINT32_MAX) return 1;
    return write_u32((uint32_t)length) != 0 || write_bytes(bytes, length) != 0;
}

static int encode_public_proof(
    const lxp_merkle_proof *proof, uint8_t *bytes, size_t capacity,
    size_t *encoded_length)
{
    size_t length;
    size_t i;
    if (proof == NULL || bytes == NULL || encoded_length == NULL ||
        proof->depth > LXP_MERKLE_MAX_DEPTH)
        return 1;
    length = EXPLORER_PROOF_PREFIX_BYTES + (size_t)proof->depth * 32U;
    if (length > capacity) return 1;
    bytes[0] = EXPLORER_PROOF_VERSION;
    bytes[1] = (uint8_t)(proof->leaf_index >> 24U);
    bytes[2] = (uint8_t)(proof->leaf_index >> 16U);
    bytes[3] = (uint8_t)(proof->leaf_index >> 8U);
    bytes[4] = (uint8_t)proof->leaf_index;
    bytes[5] = (uint8_t)(proof->leaf_count >> 24U);
    bytes[6] = (uint8_t)(proof->leaf_count >> 16U);
    bytes[7] = (uint8_t)(proof->leaf_count >> 8U);
    bytes[8] = (uint8_t)proof->leaf_count;
    bytes[9] = proof->depth;
    for (i = 0U; i < proof->depth; ++i)
        (void)memcpy(bytes + EXPLORER_PROOF_PREFIX_BYTES + i * 32U,
                     proof->siblings[i], 32U);
    *encoded_length = length;
    return 0;
}

int main(void)
{
    static const uint8_t sequencer_private_key[32] = {3U};
    static const uint8_t canonical_activity[] = {1U, 2U, 3U, 4U};
    static const uint8_t other_activity[] = {5U, 6U};
    static const uint8_t envelope_magic[4] = {'L', 'X', 'E', 'F'};
    static uint8_t arena_bytes[LXP_MAX_ACTIVITY_BYTES + 65536U];
    static uint8_t receipt_bytes[LXP_MAX_ACTIVITY_BYTES];
    uint8_t header_bytes[LXP_BATCH_HEADER_ENCODED_SIZE];
    uint8_t proof_bytes[
        EXPLORER_PROOF_PREFIX_BYTES + LXP_MERKLE_MAX_DEPTH * 32U];
    uint8_t sequencer_public_key[32];
    uint8_t activity_hashes[2][32];
    uint8_t activity_root[32];
    uint8_t header_signature[64];
    lxp_merkle_proof activity_proof;
    lxp_ledger_receipt_input input;
    lxp_receipt receipt;
    lxp_batch_header header;
    lxp_sequencer_authorization authorization;
    lxp_byte_span encoded;
    lxp_arena arena;
    size_t receipt_length;
    size_t proof_length;
    size_t mark;

    if (lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        public_key_for(sequencer_private_key, sequencer_public_key) != 0 ||
        lxp_merkle_leaf_hash(
            canonical_activity, sizeof(canonical_activity),
            activity_hashes[0]) != LXP_OK ||
        lxp_merkle_leaf_hash(
            other_activity, sizeof(other_activity),
            activity_hashes[1]) != LXP_OK ||
        lxp_merkle_proof_generate(
            (const uint8_t (*)[32])activity_hashes, 2U, 0U, &arena,
            &activity_proof, activity_root) != LXP_OK ||
        lxp_merkle_proof_verify(
            activity_hashes[0], &activity_proof, activity_root) != LXP_OK ||
        encode_public_proof(
            &activity_proof, proof_bytes, sizeof(proof_bytes),
            &proof_length) != 0)
        return 1;

    (void)memset(&input, 0, sizeof(input));
    if (lxp_hash_activity_id(
            canonical_activity, sizeof(canonical_activity),
            input.transaction_id) != LXP_OK)
        return 1;
    input.operation = (uint8_t)LX_ASSET_SEND;
    input.global_sequence = 9U;
    input.asset[0] = 4U;
    input.amount = (lxp_u128){0U, 25U};
    input.from[0] = 5U;
    input.from_balance_before = (lxp_u128){0U, 100U};
    input.from_balance_after = (lxp_u128){0U, 75U};
    input.to[0] = 6U;
    input.to_balance_before = (lxp_u128){0U, 10U};
    input.to_balance_after = (lxp_u128){0U, 35U};
    input.transfer_set_root[0] = 7U;
    input.authorization_hash[0] = 8U;
    input.context_hash[0] = 9U;
    input.previous_state_root[0] = 10U;
    input.resulting_state_root[0] = 20U;
    input.batch_id[0] = 11U;
    input.timestamp = 1000U;
    input.leg_count = 1U;
    if (lxp_ledger_receipt_build(&receipt, &input) != LXP_OK ||
        lxp_receipt_sign(&receipt, sequencer_private_key, &arena) != LXP_OK ||
        lxp_receipt_verify(
            &receipt, sequencer_public_key, &arena) != LXP_OK)
        return 1;
    mark = lxp_arena_mark(&arena);
    if (lxp_receipt_encode(&receipt, true, &arena, &encoded) != LXP_OK ||
        encoded.length > sizeof(receipt_bytes))
        return 1;
    receipt_length = encoded.length;
    (void)memcpy(receipt_bytes, encoded.bytes, receipt_length);
    if (lxp_arena_reset(&arena, mark) != LXP_OK) return 1;

    (void)memset(&authorization, 0, sizeof(authorization));
    (void)memcpy(authorization.public_key, sequencer_public_key, 32U);
    (void)memcpy(authorization.sequencer_id, sequencer_public_key, 32U);
    authorization.first_batch_number = 3U;
    authorization.last_batch_number = 3U;
    authorization.authorized = 1U;
    (void)memset(&header, 0, sizeof(header));
    header.protocol_version = LXP_PROTOCOL_VERSION;
    header.network_id = 42U;
    header.epoch = 2U;
    header.batch_number = 3U;
    header.first_sequence = 9U;
    header.last_sequence = 9U;
    (void)memcpy(header.previous_state_root, input.previous_state_root, 32U);
    (void)memcpy(header.resulting_state_root, input.resulting_state_root, 32U);
    (void)memcpy(header.activity_merkle_root, activity_root, 32U);
    header.receipt_merkle_root[0] = 13U;
    header.event_merkle_root[0] = 14U;
    header.data_availability_root[0] = 15U;
    header.oracle_root[0] = 16U;
    header.timestamp_ms = 1000U;
    (void)memcpy(header.sequencer_id, authorization.sequencer_id, 32U);
    if (lxp_batch_sign(
            &header, sequencer_private_key, &authorization,
            header_signature, &arena) != LXP_OK ||
        lxp_batch_verify_signature(
            &header, header_signature, sizeof(header_signature),
            &authorization, &arena) != LXP_OK)
        return 1;
    mark = lxp_arena_mark(&arena);
    if (lxp_batch_header_encode(&header, &arena, &encoded) != LXP_OK ||
        encoded.length != sizeof(header_bytes))
        return 1;
    (void)memcpy(header_bytes, encoded.bytes, sizeof(header_bytes));
    if (lxp_arena_reset(&arena, mark) != LXP_OK) return 1;

    if (write_bytes(envelope_magic, sizeof(envelope_magic)) != 0 ||
        fputc(EXPLORER_FIXTURE_VERSION, stdout) == EOF ||
        write_sized(receipt_bytes, receipt_length) != 0 ||
        write_bytes(sequencer_public_key, sizeof(sequencer_public_key)) != 0 ||
        write_bytes(receipt.batch_id, sizeof(receipt.batch_id)) != 0 ||
        write_bytes(receipt.asset, sizeof(receipt.asset)) != 0 ||
        write_bytes(
            receipt.previous_state_root,
            sizeof(receipt.previous_state_root)) != 0 ||
        write_bytes(
            receipt.resulting_state_root,
            sizeof(receipt.resulting_state_root)) != 0 ||
        write_sized(canonical_activity, sizeof(canonical_activity)) != 0 ||
        write_sized(proof_bytes, proof_length) != 0 ||
        write_sized(header_bytes, sizeof(header_bytes)) != 0 ||
        write_bytes(header_signature, sizeof(header_signature)) != 0 ||
        write_bytes(
            authorization.sequencer_id,
            sizeof(authorization.sequencer_id)) != 0 ||
        write_u64(authorization.first_batch_number) != 0 ||
        write_u64(authorization.last_batch_number) != 0 ||
        write_bytes(activity_root, sizeof(activity_root)) != 0 ||
        fflush(stdout) != 0)
        return 1;
    return 0;
}
