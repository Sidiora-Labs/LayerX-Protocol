#include "layerx/lxp_activity.h"

#include <stdint.h>
#include <string.h>

int lxp_fuzz_activity(const uint8_t *data, size_t size);

int main(void)
{
    uint8_t storage[LXP_MAX_ACTIVITY_BYTES];
    uint8_t second_storage[LXP_MAX_ACTIVITY_BYTES];
    const uint8_t did[] = "did:lxp:alice";
    const uint8_t authority[] = { 1U, 2U, 3U };
    const uint8_t payload[] = { 4U, 5U, 6U };
    const uint8_t signature[64] = { 7U };
    lxp_activity original;
    lxp_activity decoded;
    lxp_arena arena;
    lxp_arena second_arena;
    lxp_byte_span encoded;
    lxp_byte_span reencoded;
    uint8_t first_id[32];
    uint8_t changed_id[32];
    uint8_t changed[LXP_MAX_ACTIVITY_BYTES];
    (void)memset(&original, 0, sizeof(original));
    original.protocol_version = LXP_PROTOCOL_VERSION;
    original.network_id = 42U;
    original.activity_type = UINT32_C(0x12340056);
    original.actor_did = (lxp_byte_span){ did, sizeof(did) - 1U };
    original.authority = (lxp_byte_span){ authority, sizeof(authority) };
    original.account_sequence = 9U;
    original.timestamp_bound = (lxp_timestamp_bound){ 100U, 200U };
    original.idempotency_key[0] = 8U;
    original.fee_limit = (lxp_u128){ 1U, 2U };
    original.payload_hash[0] = 10U;
    original.payload = (lxp_byte_span){ payload, sizeof(payload) };
    original.signature = (lxp_byte_span){ signature, sizeof(signature) };
    if (lxp_arena_init(&arena, storage, sizeof(storage)) != LXP_OK ||
        lxp_activity_encode(&original, &arena, &encoded) != LXP_OK ||
        lxp_activity_decode(encoded.bytes, encoded.length, &decoded) != LXP_OK ||
        lxp_arena_init(&second_arena, second_storage, sizeof(second_storage)) !=
            LXP_OK || lxp_activity_encode(&decoded, &second_arena, &reencoded) !=
            LXP_OK || reencoded.length != encoded.length ||
        memcmp(reencoded.bytes, encoded.bytes, encoded.length) != 0) return 1;
    if (lxp_activity_module_id(decoded.activity_type) != 0x1234U ||
        lxp_activity_type_ordinal(decoded.activity_type) != 0x0056U ||
        decoded.fee_limit.hi != 1U || decoded.fee_limit.lo != 2U) return 1;
    if (lxp_activity_id(encoded.bytes, encoded.length, first_id) != LXP_OK)
        return 1;
    (void)memcpy(changed, encoded.bytes, encoded.length);
    changed[4] ^= 1U;
    if (lxp_activity_id(changed, encoded.length, changed_id) != LXP_OK ||
        memcmp(first_id, changed_id, sizeof(first_id)) == 0) return 1;
    (void)memcpy(changed, encoded.bytes, encoded.length);
    changed[5] = 2U;
    if (lxp_activity_decode(changed, encoded.length, &decoded) !=
        LXP_ERR_MALFORMED_ENVELOPE) return 1;
    (void)memcpy(changed, encoded.bytes, encoded.length);
    changed[5] = 13U;
    if (lxp_activity_decode(changed, encoded.length, &decoded) !=
        LXP_ERR_MALFORMED_ENVELOPE) return 1;
    if (lxp_activity_decode(encoded.bytes, encoded.length - 1U, &decoded) !=
        LXP_ERR_MALFORMED_ENVELOPE ||
        lxp_fuzz_activity(encoded.bytes, encoded.length) != 0) return 1;
    return 0;
}
