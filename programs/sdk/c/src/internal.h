#ifndef LAYERX_PROGRAM_INTERNAL_H
#define LAYERX_PROGRAM_INTERNAL_H

#include "layerx/program.h"

#include <stdint.h>

/*
 * Linear memory on wasm32 is addressed by a thirty-two bit offset, so every
 * pointer handed to the host is a checked non-negative i32. Lengths are bounded
 * by the caller before they reach this conversion.
 */
static inline int32_t lxp_program_pointer(const void *address)
{
    return (int32_t)(uint32_t)(uintptr_t)address;
}

static inline int32_t lxp_program_length(size_t length)
{
    return (int32_t)(uint32_t)length;
}

lxp_program_status lxp_program_check_key(const uint8_t *key, size_t key_length);

#endif
