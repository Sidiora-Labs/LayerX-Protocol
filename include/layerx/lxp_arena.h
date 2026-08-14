#ifndef LAYERX_LXP_ARENA_H
#define LAYERX_LXP_ARENA_H

#include "layerx/lxp_result.h"

#include <stddef.h>
#include <stdint.h>

typedef struct lxp_arena {
    uint8_t *buffer;
    size_t capacity;
    size_t offset;
} lxp_arena;
#define lxp_arena lxp_arena

lxp_result lxp_arena_init(lxp_arena *arena, void *buffer, size_t capacity);
lxp_result lxp_arena_alloc(lxp_arena *arena, size_t size, size_t alignment,
                           void **allocation);
size_t lxp_arena_mark(const lxp_arena *arena);
lxp_result lxp_arena_reset(lxp_arena *arena, size_t mark);

#endif
