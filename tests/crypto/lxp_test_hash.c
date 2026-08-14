#include "layerx/lxp_hash.h"

#include <stdio.h>
#include <string.h>

static int from_hex(const char *hex, uint8_t out[32])
{
    size_t i;
    for (i = 0U; i < 32U; ++i) {
        unsigned value;
        if (sscanf(hex + i * 2U, "%2x", &value) != 1) return 1;
        out[i] = (uint8_t)value;
    }
    return 0;
}

int main(void)
{
    uint8_t produced[32];
    uint8_t expected[32];
    uint8_t domains[LXP_DOMAIN_TAG_COUNT][32];
    lxp_domain_tag_id i;
    lxp_domain_tag_id j;
    if (from_hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", expected) != 0 ||
        lxp_hash_sha256(NULL, 0U, produced) != LXP_OK ||
        memcmp(expected, produced, 32U) != 0) return 1;
    if (from_hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", expected) != 0 ||
        lxp_hash_sha256("abc", 3U, produced) != LXP_OK ||
        memcmp(expected, produced, 32U) != 0) return 1;
    for (i = 0U; i < (lxp_domain_tag_id)LXP_DOMAIN_TAG_COUNT; ++i)
        if (lxp_hash_domain(i, "same", 4U, domains[i]) != LXP_OK) return 1;
    for (i = 0U; i < (lxp_domain_tag_id)LXP_DOMAIN_TAG_COUNT; ++i)
        for (j = (lxp_domain_tag_id)(i + 1U);
             j < (lxp_domain_tag_id)LXP_DOMAIN_TAG_COUNT; ++j)
            if (memcmp(domains[i], domains[j], 32U) == 0) return 1;
    return 0;
}
