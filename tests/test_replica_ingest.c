#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_replica.h"

#include <openssl/evp.h>
#include <stdbool.h>
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

static int make_body(lxp_batch_body *body, uint64_t batch_number,
                     uint64_t sequence, const uint8_t previous_root[32],
                     const uint8_t private_key[32],
                     const lxp_sequencer_authorization *authorization,
                     lxp_arena *arena, lxp_byte_span *encoded)
{
    static const uint8_t content[] = { 1U, 2U, 3U };
    (void)memset(body, 0, sizeof(*body));
    body->header.protocol_version = 1U;
    body->header.network_id = 55U;
    body->header.epoch = 1U;
    body->header.batch_number = batch_number;
    body->header.first_sequence = sequence;
    body->header.last_sequence = sequence;
    (void)memcpy(body->header.previous_state_root, previous_root, 32U);
    body->header.resulting_state_root[0] = (uint8_t)(batch_number + 1U);
    body->header.timestamp_ms = 100U + batch_number;
    (void)memcpy(body->header.sequencer_id, authorization->sequencer_id, 32U);
    body->activities = (lxp_byte_span){ content, sizeof(content) };
    body->receipts = (lxp_byte_span){ content, sizeof(content) };
    body->events = (lxp_byte_span){ content, sizeof(content) };
    body->oracle_inputs = (lxp_byte_span){ content, sizeof(content) };
    body->state_diff = (lxp_byte_span){ content, sizeof(content) };
    body->recovery_metadata = (lxp_byte_span){ content, sizeof(content) };
    if (lxp_batch_sign(&body->header, private_key, authorization,
                       body->sequencer_signature, arena) != LXP_OK)
        return 1;
    return lxp_batch_body_encode(body, arena, encoded) == LXP_OK ? 0 : 1;
}

int main(void)
{
    uint8_t build_storage[32768];
    uint8_t ingest_storage[32768];
    uint8_t private_key[32] = { 4U };
    uint8_t zero_root[32] = { 0U };
    uint8_t first_bytes[4096];
    uint8_t second_bytes[4096];
    size_t first_length;
    size_t second_length;
    lxp_arena build_arena;
    lxp_arena ingest_arena;
    lxp_batch_body first;
    lxp_batch_body second;
    lxp_byte_span encoded;
    lxp_sequencer_authorization authorization;
    lxp_replica replica;
    lxp_log log;
    lxp_log_record_header record;
    uint8_t stored[4096];
    bool ack = true;
    char directory[] = "/tmp/lxp-replica-ingest-XXXXXX";
    char path[128];
    (void)memset(&authorization, 0, sizeof(authorization));
    if (public_key_for(private_key, authorization.public_key) != 0) return 1;
    (void)memcpy(authorization.sequencer_id, authorization.public_key, 32U);
    authorization.first_batch_number = 0U;
    authorization.last_batch_number = 10U;
    authorization.authorized = 1U;
    if (lxp_arena_init(&build_arena, build_storage, sizeof(build_storage)) !=
        LXP_OK || make_body(&first, 0U, 0U, zero_root, private_key,
                            &authorization, &build_arena, &encoded) != 0)
        return 1;
    first_length = encoded.length;
    (void)memcpy(first_bytes, encoded.bytes, first_length);
    if (lxp_arena_reset(&build_arena, 0U) != LXP_OK ||
        make_body(&second, 1U, 1U, first.header.resulting_state_root,
                  private_key, &authorization, &build_arena, &encoded) != 0)
        return 1;
    second_length = encoded.length;
    (void)memcpy(second_bytes, encoded.bytes, second_length);
    if (mkdtemp(directory) == NULL ||
        snprintf(path, sizeof(path), "%s/%020u.lxp", directory, 0U) < 0 ||
        lxp_log_segment_create(&log, directory, 0U, 32768U) != LXP_OK ||
        lxp_replica_init(&replica, &log) != LXP_OK ||
        lxp_arena_init(&ingest_arena, ingest_storage,
                       sizeof(ingest_storage)) != LXP_OK) return 1;
    if (lxp_replica_ingest_batch(&replica, second_bytes, second_length, 55U,
                                 &authorization, &ingest_arena, &ack) !=
            LXP_ERR_BATCH_GAP || ack || replica.has_head ||
        lxp_replica_ingest_batch(&replica, first_bytes, first_length - 1U, 55U,
                                 &authorization, &ingest_arena, &ack) ==
            LXP_OK || ack || replica.has_head ||
        lxp_replica_ingest_batch(&replica, first_bytes, first_length, 55U,
                                 &authorization, &ingest_arena, &ack) !=
            LXP_OK || !ack || !replica.has_head ||
        replica.durable_batch_count != 1U ||
        lxp_replica_ingest_batch(&replica, first_bytes, first_length, 55U,
                                 &authorization, &ingest_arena, &ack) !=
            LXP_ERR_BATCH_GAP || ack || replica.durable_batch_count != 1U)
        return 1;
    if (lxp_replica_ingest_batch(&replica, second_bytes, second_length, 55U,
                                 &authorization, &ingest_arena, &ack) !=
            LXP_OK || !ack || replica.durable_batch_count != 2U ||
        lxp_log_read(&log, 0U, &record, stored, sizeof(stored)) != LXP_OK ||
        record.record_kind != (uint8_t)LXP_LOG_BATCH_BODY ||
        record.body_length != first_length ||
        memcmp(stored, first_bytes, first_length) != 0)
        return 1;
    if (lxp_log_close(&log) != LXP_OK || unlink(path) != 0 ||
        rmdir(directory) != 0) return 1;
    return 0;
}
