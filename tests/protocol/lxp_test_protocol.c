#include "layerx/lxp_protocol.h"

#include <stdint.h>
#include <stdio.h>
#include <string.h>

static int check_tags(void)
{
    lxp_domain_tag_id i;
    lxp_domain_tag_id j;

    for (i = 0U; i < (lxp_domain_tag_id)LXP_DOMAIN_TAG_COUNT; ++i) {
        size_t i_length = 0U;
        const uint8_t *i_tag = lxp_domain_tag(i, &i_length);
        if (i_tag == NULL || i_length == 0U) return 1;
        for (j = 0U; j < (lxp_domain_tag_id)LXP_DOMAIN_TAG_COUNT; ++j) {
            size_t j_length = 0U;
            size_t common;
            const uint8_t *j_tag;
            if (i == j) continue;
            j_tag = lxp_domain_tag(j, &j_length);
            if (j_tag == NULL || j_length == 0U) return 1;
            common = i_length < j_length ? i_length : j_length;
            if (memcmp(i_tag, j_tag, common) == 0) {
                fprintf(stderr, "domain tags %u and %u collide by prefix\n",
                        (unsigned)i, (unsigned)j);
                return 1;
            }
        }
    }
    return 0;
}

int main(void)
{
    size_t ignored = 0U;
    if (!lxp_protocol_version_supported((uint16_t)LXP_PROTOCOL_VERSION) ||
        lxp_protocol_version_supported(UINT16_C(0)) ||
        lxp_protocol_version_supported(UINT16_MAX)) return 1;
    if (!lxp_network_id_matches(UINT32_C(17), UINT32_C(17)) ||
        lxp_network_id_matches(UINT32_C(17), UINT32_C(18)) ||
        lxp_network_id_matches(UINT32_C(0), UINT32_C(0))) return 1;
    if (LXP_MAX_ACTIVITY_BYTES == 0 || LXP_MAX_ACTIVITY_BYTES > UINT32_MAX ||
        LXP_MAX_PAYLOAD_BYTES == 0 || LXP_MAX_PAYLOAD_BYTES > UINT32_MAX ||
        LXP_MAX_DID_LENGTH == 0 || LXP_MAX_DID_LENGTH > UINT16_MAX ||
        LXP_MAX_AUTHORITY_CHAIN_DEPTH == 0 ||
        LXP_MAX_AUTHORITY_CHAIN_DEPTH > UINT8_MAX ||
        LXP_MAX_TRANSFER_SET_LEGS == 0 ||
        LXP_MAX_TRANSFER_SET_LEGS > UINT16_MAX ||
        LXP_MAX_EFFECTS == 0 || LXP_MAX_EFFECTS > UINT16_MAX ||
        LXP_MAX_BATCH_ACTIVITIES == 0 ||
        LXP_MAX_BATCH_ACTIVITIES > UINT32_MAX) return 1;
    if (lxp_domain_tag((lxp_domain_tag_id)LXP_DOMAIN_TAG_COUNT, &ignored) != NULL ||
        lxp_domain_tag(LXP_DOMAIN_ACTIVITY_ID, NULL) != NULL) return 1;
    return check_tags();
}
