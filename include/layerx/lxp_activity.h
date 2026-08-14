#ifndef LAYERX_LXP_ACTIVITY_H
#define LAYERX_LXP_ACTIVITY_H

#include "layerx/lxp_codec.h"
#include "layerx/lxp_hash.h"
#include "layerx/lxp_u128.h"

#include <stddef.h>
#include <stdint.h>

typedef struct lxp_timestamp_bound {
    uint64_t not_before;
    uint64_t not_after;
} lxp_timestamp_bound;

typedef struct lxp_activity {
    uint16_t protocol_version;
    uint32_t network_id;
    uint32_t activity_type;
    lxp_byte_span actor_did;
    lxp_byte_span authority;
    uint64_t account_sequence;
    lxp_timestamp_bound timestamp_bound;
    uint8_t idempotency_key[32];
    lxp_u128 fee_limit;
    uint8_t payload_hash[32];
    lxp_byte_span payload;
    lxp_byte_span signature;
} lxp_activity;
#define lxp_activity lxp_activity

uint16_t lxp_activity_module_id(uint32_t activity_type);
uint16_t lxp_activity_type_ordinal(uint32_t activity_type);
lxp_result lxp_activity_encode(const lxp_activity *activity, lxp_arena *arena,
                               lxp_byte_span *encoded);
lxp_result lxp_activity_decode(const uint8_t *bytes, size_t length,
                               lxp_activity *activity);
lxp_result lxp_activity_id(const uint8_t *canonical_activity, size_t length,
                           uint8_t identifier[32]);

/* Normative envelope check order: version, network, then payload binding. */
lxp_result lxp_activity_check_envelope(const lxp_activity *activity,
                                       uint32_t executing_network_id);
lxp_result lxp_activity_verify_payload_hash(const lxp_activity *activity);
lxp_result lxp_activity_signing_preimage(const lxp_activity *activity,
                                         uint8_t preimage_hash[32]);
lxp_result lxp_activity_verify_signature(const lxp_activity *activity);

#endif
