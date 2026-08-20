#ifndef LAYERX_PROGRAMS_H
#define LAYERX_PROGRAMS_H

#include "layerx/lxp_module.h"

#include <stdint.h>

enum {
    LX_PROGRAMS_DEPLOY = 0x00090001,
    LX_PROGRAMS_UPGRADE = 0x00090002,
    LX_PROGRAMS_CALL = 0x00090003,
    LX_PROGRAMS_REGISTRY = 0x00090004,
    LX_PROGRAMS_ABI_VERSION = 1,
    LX_PROGRAMS_EVENT_DEPLOYED = 1,
    LX_PROGRAMS_EVENT_UPGRADED = 2,
    LX_PROGRAMS_EVENT_CALLED = 3,
    LX_PROGRAMS_EVENT_REGISTRY_READ = 4
};

const lxp_module_iface *programs_module_registration(void);
const lxp_module_iface *lx_programs_module_iface(void);

lxp_result lxp_programs_lifecycle_decode(lxp_module_ctx *ctx,
                                         uint16_t ordinal,
                                         const uint8_t *payload,
                                         size_t payload_length,
                                         void **decoded);
lxp_result lxp_programs_lifecycle_validate(
    lxp_module_ctx *ctx, const lxp_activity *activity,
    const lxp_authority_resolved *authority, const void *decoded);
lxp_result lxp_programs_lifecycle_execute(
    lxp_module_ctx *ctx, const lxp_activity *activity,
    const lxp_authority_resolved *authority, const void *decoded,
    lxp_effect_buffer *effects);
void lxp_programs_lifecycle_release(lxp_module_ctx *ctx, void *decoded);

uint64_t layerx_programs_migration_begin(uint32_t wasm_length,
                                         uint16_t hook_length);
lxp_result layerx_programs_migration_wasm_byte(uint64_t handle, uint8_t byte);
lxp_result layerx_programs_migration_hook_byte(uint64_t handle, uint8_t byte);
lxp_result layerx_programs_migration_execute(uint64_t handle);
void layerx_programs_migration_abort(uint64_t handle);

#endif
