#include "layerx/lxp_kernel.h"

#include <stdint.h>
#include <string.h>

struct lxp_transfer_set { uint32_t marker; };
struct lxp_receipt { uint32_t marker; };

static lxp_result module_genesis(lxp_module_ctx *ctx, const uint8_t *bytes,
                                 size_t length)
{ (void)ctx; (void)bytes; (void)length; return LXP_OK; }
static lxp_result module_decode(lxp_module_ctx *ctx, uint16_t ordinal,
                                const uint8_t *bytes, size_t length,
                                void **decoded)
{ (void)ctx; (void)ordinal; (void)bytes; (void)length; *decoded = NULL;
  return LXP_OK; }
static lxp_result module_validate(lxp_module_ctx *ctx,
                                  const lxp_activity *activity,
                                  const lxp_authority_resolved *authority,
                                  const void *decoded)
{ (void)ctx; (void)activity; (void)authority; (void)decoded; return LXP_OK; }
static lxp_result module_execute(lxp_module_ctx *ctx,
                                 const lxp_activity *activity,
                                 const lxp_authority_resolved *authority,
                                 const void *decoded,
                                 lxp_effect_buffer *effects)
{ (void)ctx; (void)activity; (void)authority; (void)decoded; (void)effects;
  return LXP_OK; }
static lxp_result module_epoch(lxp_module_ctx *ctx, uint64_t epoch,
                               uint64_t timestamp)
{ (void)ctx; (void)epoch; (void)timestamp; return LXP_OK; }
static lxp_result module_root(lxp_module_ctx *ctx, uint8_t root[32])
{ (void)ctx; (void)memset(root, 0, 32U); return LXP_OK; }

static lxp_result read_parameter(const void *parameters, uint32_t id,
                                 uint64_t *value)
{
    const uint64_t *base = parameters;
    *value = *base + id;
    return LXP_OK;
}

static lxp_result apply_transfer(lxp_kernel *kernel,
                                 const lxp_transfer_set *set,
                                 lxp_receipt *receipt)
{
    (void)kernel;
    receipt->marker = set->marker;
    return LXP_OK;
}

static lxp_result visit(const uint8_t *key, size_t key_length,
                        const uint8_t *value, size_t value_length, void *user)
{
    uint32_t *order = user;
    if (key_length != 1U || value_length != 1U || key[0] != (uint8_t)*order ||
        value[0] != (uint8_t)(key[0] + 10U)) return LXP_FATAL_INVARIANT;
    ++*order;
    return LXP_OK;
}

int main(void)
{
    static const uint32_t types[] = { UINT32_C(0x00010001) };
    lxp_module_iface iface = { LXP_MODULE_ASSET, 1U, "asset", types, 1U,
        module_genesis, module_decode, module_validate, module_execute,
        module_epoch, module_epoch, module_root, NULL };
    lxp_state_store store;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx asset;
    uint64_t parameters = 40U;
    uint8_t arena_bytes[128];
    lxp_arena arena;
    uint8_t key_a = 1U;
    uint8_t key_b = 2U;
    uint8_t value_a = 11U;
    uint8_t value_b = 12U;
    const uint8_t *found;
    size_t found_length;
    uint64_t parameter;
    uint32_t order = 1U;
    lxp_transfer_set set = { 77U };
    lxp_receipt receipt = { 0U };
    void *allocation;
    if (lxp_state_store_init(&store, 0U) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_kernel_create(&kernel, &store, &journal, &parameters, 3U) !=
            LXP_OK || lxp_kernel_register_module(&kernel, &iface) != LXP_OK ||
        lxp_kernel_set_capabilities(&kernel, read_parameter,
                                    apply_transfer) != LXP_OK ||
        lxp_module_ctx_init(&asset, &kernel, LXP_MODULE_ASSET, 900U, 3U,
                            8U, 20U, &arena, true) != LXP_OK ||
        lxp_ctx_kv_put(&asset, &key_b, 1U, &value_b, 1U) != LXP_OK ||
        lxp_ctx_kv_put(&asset, &key_a, 1U, &value_a, 1U) != LXP_OK ||
        lxp_ctx_kv_get(&asset, &key_a, 1U, &found, &found_length) != LXP_OK ||
        found_length != 1U || found[0] != value_a ||
        lxp_ctx_kv_iter(&asset, NULL, 0U, visit, &order) != LXP_OK ||
        order != 3U || lxp_module_ctx_commit(&asset) != LXP_OK ||
        lxp_ctx_batch_timestamp_ms(&asset) != 900U ||
        lxp_ctx_epoch(&asset) != 3U || lxp_ctx_global_sequence(&asset) != 8U ||
        lxp_ctx_read_param(&asset, 2U, &parameter) != LXP_OK ||
        parameter != 42U || lxp_ctx_charge_gas(&asset, 15U) != LXP_OK ||
        lxp_ctx_charge_gas(&asset, 6U) != LXP_ERR_GAS_EXHAUSTED ||
        lxp_ctx_arena_alloc(&asset, 8U, 8U, &allocation) != LXP_OK ||
        allocation == NULL || lxp_ctx_emit_transfer_set(&asset, &set,
                                                        &receipt) != LXP_OK ||
        receipt.marker != 77U || lxp_module_ctx_set_mutable(&asset, false) !=
            LXP_OK || lxp_ctx_kv_put(&asset, &key_a, 1U, &value_a, 1U) !=
            LXP_FATAL_INVARIANT ||
        lxp_ctx_emit_transfer_set(&asset, &set, &receipt) !=
            LXP_ERR_BALANCE_BYPASS ||
        lxp_state_store_destroy(&store) != LXP_OK) return 1;
    return 0;
}
