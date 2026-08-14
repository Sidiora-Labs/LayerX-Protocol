#include "layerx/lxp_arena.h"

#include <stdint.h>

int lxp_sanitizer_smoke(void)
{
    uint8_t bytes[32];
    lxp_arena arena;
    void *allocation = NULL;
    if (lxp_arena_init(&arena, bytes, sizeof(bytes)) != LXP_OK) return 1;
    if (lxp_arena_alloc(&arena, sizeof(uint64_t), _Alignof(uint64_t),
                        &allocation) != LXP_OK) return 1;
    *(uint64_t *)allocation = UINT64_C(0x0102030405060708);
    return *(const uint64_t *)allocation == UINT64_C(0x0102030405060708) ? 0 : 1;
}

int lxp_ci_gate_report(void)
{
#if defined(__SANITIZE_ADDRESS__) || defined(__SANITIZE_THREAD__)
    return 1;
#else
    return 0;
#endif
}

int main(void)
{
    return lxp_sanitizer_smoke();
}
