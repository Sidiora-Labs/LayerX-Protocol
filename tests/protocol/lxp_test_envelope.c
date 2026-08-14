#include "layerx/lxp_activity.h"

#include <stdint.h>
#include <string.h>

static void initialize(lxp_activity *activity, const uint8_t *payload,
                       size_t payload_length)
{
    static const uint8_t did[] = "did:lxp:test";
    static const uint8_t authority[] = { 1U };
    static const uint8_t signature[64] = { 2U };
    (void)memset(activity, 0, sizeof(*activity));
    activity->protocol_version = LXP_PROTOCOL_VERSION;
    activity->network_id = 77U;
    activity->activity_type = UINT32_C(0x00010001);
    activity->actor_did = (lxp_byte_span){ did, sizeof(did) - 1U };
    activity->authority = (lxp_byte_span){ authority, sizeof(authority) };
    activity->timestamp_bound = (lxp_timestamp_bound){ 1U, 2U };
    activity->payload = (lxp_byte_span){ payload, payload_length };
    activity->signature = (lxp_byte_span){ signature, sizeof(signature) };
    (void)lxp_hash_payload(payload, payload_length, activity->payload_hash);
}

int main(void)
{
    const uint8_t payload[] = { 3U, 4U, 5U };
    lxp_activity activity;
    uint8_t first_preimage[32];
    uint8_t second_preimage[32];
    initialize(&activity, payload, sizeof(payload));
    if (lxp_activity_check_envelope(&activity, 77U) != LXP_OK ||
        lxp_activity_signing_preimage(&activity, first_preimage) != LXP_OK)
        return 1;
    activity.signature = (lxp_byte_span){ payload, sizeof(payload) };
    if (lxp_activity_signing_preimage(&activity, second_preimage) != LXP_OK ||
        memcmp(first_preimage, second_preimage, sizeof(first_preimage)) != 0)
        return 1;
    activity.protocol_version = UINT16_MAX;
    activity.network_id = 78U;
    activity.payload_hash[0] ^= 1U;
    if (lxp_activity_check_envelope(&activity, 77U) !=
        LXP_ERR_VERSION_UNSUPPORTED) return 1;
    activity.protocol_version = LXP_PROTOCOL_VERSION;
    if (lxp_activity_check_envelope(&activity, 77U) != LXP_ERR_WRONG_NETWORK)
        return 1;
    activity.network_id = 77U;
    if (lxp_activity_check_envelope(&activity, 77U) !=
        LXP_ERR_PAYLOAD_HASH_MISMATCH) return 1;
    return 0;
}
