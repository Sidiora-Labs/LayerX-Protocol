#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_batch.h"

#include <openssl/evp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static int public_key_for(const uint8_t private_key[32], uint8_t public_key[32])
{
    EVP_PKEY *key = EVP_PKEY_new_raw_private_key(EVP_PKEY_ED25519, NULL,
                                                  private_key, 32U);
    size_t length = 32U;
    int ok = key != NULL && EVP_PKEY_get_raw_public_key(
        key, public_key, &length) == 1 && length == 32U;
    EVP_PKEY_free(key);
    return ok ? 0 : 1;
}

int main(void)
{
    uint8_t arena_storage[8192];
    uint8_t private_key[32] = { 1U };
    uint8_t signature[64];
    uint8_t changed_signature[64];
    uint8_t activity_bytes[3][3] = {{1U,2U,3U},{4U,5U,6U},{7U,8U,9U}};
    uint8_t receipt_bytes[3][2] = {{1U,1U},{2U,2U},{3U,3U}};
    uint8_t event_bytes[1][2] = {{8U,9U}};
    uint8_t oracle_bytes[1][2] = {{10U,11U}};
    uint8_t da_bytes[2][2] = {{12U,13U},{14U,15U}};
    lxp_byte_span activities[3];
    lxp_byte_span receipts[3];
    lxp_byte_span events[1];
    lxp_byte_span oracles[1];
    lxp_byte_span chunks[2];
    lxp_batch_root_inputs root_inputs;
    lxp_batch_roots roots;
    lxp_batch_roots changed_roots;
    lxp_batch_seal_input seal_input;
    lxp_batch_header header;
    lxp_batch_header changed_header;
    lxp_sequencer_authorization authorization;
    lxp_arena arena;
    lxp_log log;
    lxp_log_record_header record;
    uint8_t encoded_header[LXP_BATCH_HEADER_ENCODED_SIZE];
    char directory[] = "/tmp/lxp-batch-seal-XXXXXX";
    char path[128];
    size_t i;
    if (mkdtemp(directory) == NULL ||
        snprintf(path, sizeof(path), "%s/%020u.lxp", directory, 0U) < 0 ||
        lxp_log_segment_create(&log, directory, 0U, 16384U) != LXP_OK ||
        lxp_arena_init(&arena, arena_storage, sizeof(arena_storage)) != LXP_OK)
        return 1;
    for (i = 0U; i < 3U; ++i) {
        activities[i] = (lxp_byte_span){ activity_bytes[i], 3U };
        receipts[i] = (lxp_byte_span){ receipt_bytes[i], 2U };
    }
    events[0] = (lxp_byte_span){ event_bytes[0], 2U };
    oracles[0] = (lxp_byte_span){ oracle_bytes[0], 2U };
    chunks[0] = (lxp_byte_span){ da_bytes[0], 2U };
    chunks[1] = (lxp_byte_span){ da_bytes[1], 2U };
    root_inputs = (lxp_batch_root_inputs){
        activities, 3U, receipts, 3U, events, 1U, oracles, 1U, chunks, 2U
    };
    if (lxp_batch_roots_compute(&root_inputs, &arena, &roots) != LXP_OK)
        return 1;
    (void)memset(&seal_input, 0, sizeof(seal_input));
    seal_input.protocol_version = 1U;
    seal_input.network_id = 44U;
    seal_input.epoch = 2U;
    seal_input.batch_number = 7U;
    seal_input.first_sequence = 21U;
    seal_input.last_sequence = 23U;
    seal_input.previous_state_root[0] = 30U;
    seal_input.resulting_state_root[0] = 31U;
    seal_input.timestamp_ms = 1700000000000U;
    (void)memset(&authorization, 0, sizeof(authorization));
    if (public_key_for(private_key, authorization.public_key) != 0) return 1;
    (void)memcpy(authorization.sequencer_id, authorization.public_key, 32U);
    authorization.first_batch_number = 7U;
    authorization.last_batch_number = 10U;
    authorization.authorized = 1U;
    (void)memcpy(seal_input.sequencer_id, authorization.sequencer_id, 32U);
    if (lxp_batch_seal(&header, &seal_input, &roots, &log, &arena) != LXP_OK ||
        lxp_batch_sign(&header, private_key, &authorization, signature,
                       &arena) != LXP_OK ||
        lxp_batch_verify_signature(&header, signature, sizeof(signature),
                                   &authorization, &arena) != LXP_OK ||
        lxp_log_read(&log, 0U, &record, encoded_header,
                     sizeof(encoded_header)) != LXP_OK ||
        record.record_kind != (uint8_t)LXP_LOG_BATCH_HEADER ||
        record.body_length != LXP_BATCH_HEADER_ENCODED_SIZE)
        return 1;
    activity_bytes[1][1] ^= 1U;
    if (lxp_batch_roots_compute(&root_inputs, &arena, &changed_roots) != LXP_OK ||
        memcmp(roots.activity_merkle_root,
               changed_roots.activity_merkle_root, 32U) == 0)
        return 1;
    changed_header = header;
    (void)memcpy(changed_header.activity_merkle_root,
                 changed_roots.activity_merkle_root, 32U);
    if (lxp_batch_verify_signature(&changed_header, signature,
                                   sizeof(signature), &authorization,
                                   &arena) != LXP_ERR_BAD_SIGNATURE)
        return 1;
    (void)memcpy(changed_signature, signature, sizeof(signature));
    changed_signature[0] ^= 1U;
    if (lxp_batch_verify_signature(&header, changed_signature,
                                   sizeof(changed_signature), &authorization,
                                   &arena) != LXP_ERR_BAD_SIGNATURE ||
        lxp_batch_verify_signature(&header, signature, 0U, &authorization,
                                   &arena) != LXP_ERR_BAD_SIGNATURE)
        return 1;
    authorization.authorized = 0U;
    if (lxp_batch_verify_signature(&header, signature, sizeof(signature),
                                   &authorization, &arena) !=
        LXP_ERR_AUTH_SCOPE) return 1;
    if (lxp_log_close(&log) != LXP_OK || unlink(path) != 0 ||
        rmdir(directory) != 0) return 1;
    return 0;
}
