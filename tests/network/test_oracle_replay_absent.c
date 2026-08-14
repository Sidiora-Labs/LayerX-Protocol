#include "layerx/lxp_hash.h"
#include "layerx/lxp_u128.h"

#include <string.h>

static void put_u64(uint8_t bytes[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i)
        bytes[i] = (uint8_t)(value >> ((7U - i) * 8U));
}

static int replay(bool delayed, uint8_t root[32])
{
    uint8_t payloads[2][72];
    uint8_t leaf[32];
    uint8_t chain[64];
    uint8_t current[32] = { 0U };
    volatile uint64_t elapsed = 0U;
    uint64_t delay;
    size_t i;

    (void)memset(payloads, 0, sizeof(payloads));
    payloads[0][0] = 1U;
    payloads[1][0] = 1U;
    put_u64(payloads[0] + 32U, 1U);
    put_u64(payloads[1] + 32U, 2U);
    if (lxp_u128_to_be((lxp_u128){ 0U, 100U }, payloads[0] + 40U) != LXP_OK ||
        lxp_u128_to_be((lxp_u128){ 0U, 101U }, payloads[1] + 40U) != LXP_OK)
        return 1;
    put_u64(payloads[0] + 56U, 1000U);
    put_u64(payloads[1] + 56U, 1100U);
    put_u64(payloads[0] + 64U, 42U);
    put_u64(payloads[1] + 64U, 42U);
    if (delayed)
        for (delay = 0U; delay < UINT64_C(1000000); ++delay)
            elapsed += delay;
    for (i = 0U; i < 2U; ++i) {
        if (lxp_hash_payload(payloads[i], sizeof(payloads[i]), leaf) != LXP_OK)
            return 1;
        (void)memcpy(chain, current, 32U);
        (void)memcpy(chain + 32U, leaf, 32U);
        if (lxp_hash_domain(LXP_DOMAIN_STATE_NODE, chain, sizeof(chain),
                            current) != LXP_OK)
            return 1;
    }
    (void)memcpy(root, current, 32U);
    (void)elapsed;
    return 0;
}

int main(void)
{
    uint8_t immediate[32];
    uint8_t delayed[32];
    if (replay(false, immediate) != 0 || replay(true, delayed) != 0 ||
        memcmp(immediate, delayed, 32U) != 0)
        return 1;
    return 0;
}
