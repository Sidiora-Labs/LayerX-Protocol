#include "layerx/programs.h"

#include "layerx/lxp_kernel.h"

#include <string.h>

static int exercise(uint16_t ordinal, size_t payload_length,
                    lxp_result expected, size_t expected_writes)
{
    uint8_t arena_bytes[4096];
    uint8_t payload[40];
    lxp_arena arena;
    lxp_state_store store;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_effect_buffer effects;
    lxp_activity activity;
    lxp_authority_resolved authority;
    const lxp_module_registration *registration;
    lxp_result module_result = LXP_OK;
    uint64_t parameters = 1U;
    (void)memset(payload, 0x31, sizeof(payload));
    (void)memset(&activity, 0, sizeof(activity));
    (void)memset(&authority, 0, sizeof(authority));
    (void)memset(authority.principal, 0x42, sizeof(authority.principal));
    activity.activity_type = ((uint32_t)LXP_MODULE_PROGRAMS << 16U) | ordinal;
    activity.payload = (lxp_byte_span){ payload, payload_length };
    if (lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_state_store_init(&store, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &store, &journal, &parameters, 0U) !=
            LXP_OK ||
        lxp_kernel_register_module(&kernel, programs_module_registration()) !=
            LXP_OK ||
        lxp_kernel_module_for_activity(&kernel, activity.activity_type, 0U,
                                       &registration) != LXP_OK ||
        registration->abi_version != LX_PROGRAMS_ABI_VERSION ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_PROGRAMS, 1U, 0U, 9U,
                            1000U, &arena, false) != LXP_OK ||
        lxp_effect_buffer_init(&effects) != LXP_OK ||
        lxp_module_ctx_bind_effects(&ctx, &effects) != LXP_OK ||
        lxp_kernel_dispatch(registration, &ctx, &activity, &authority,
                            &effects, &module_result) != LXP_OK)
        return 1;
    if (module_result != expected) return 1;
    if (module_result == LXP_OK) {
        if (lxp_module_ctx_commit(&ctx) != LXP_OK || effects.count != 1U ||
            effects.effects[0].module_id != LXP_MODULE_PROGRAMS ||
            effects.effects[0].event_type != ordinal ||
            kernel.module_kv_count != expected_writes)
            return 1;
    } else {
        lxp_module_ctx_rollback(&ctx);
        if (kernel.module_kv_count != 0U || effects.count != 0U) return 1;
    }
    return lxp_state_store_destroy(&store) == LXP_OK ? 0 : 1;
}

int main(void)
{
    if (programs_module_registration() != lx_programs_module_iface()) return 1;
    if (exercise(3U, 40U, LXP_OK, 1U) != 0) return 1;
    if (exercise(4U, 32U, LXP_OK, 0U) != 0) return 1;
    if (exercise(3U, 31U, LXP_ERR_TRUNCATED, 0U) != 0) return 1;
    return 0;
}
