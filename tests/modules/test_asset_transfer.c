#include "layerx/lx_asset.h"
#include "layerx/lxp_kernel.h"
#include "layerx/lxp_receipt.h"

#include <string.h>

static lxp_result apply_capability(lxp_kernel *kernel,
                                   const lxp_transfer_set *set,
                                   lxp_receipt *receipt)
{
    lxp_transfer_set_result result;
    lxp_transfer_context context = set->context;
    lxp_result status;
    (void)kernel;
    status = lxp_apply_transfer_set((lxp_transfer_leg *)set->legs,
                                    set->leg_count, &context, &result);
    if (status == LXP_OK)
        (void)memcpy(receipt->transfer_set_root, result.transfer_set_root, 32U);
    return status;
}

static int balances(const lx_account *from, const lx_account *to,
                    uint64_t from_value, uint64_t to_value)
{
    return from->balance.lo == from_value && to->balance.lo == to_value;
}

int main(void)
{
    lx_asset_record asset;
    lxp_transfer_asset_state asset_state;
    lx_account_registry accounts;
    lx_account *from;
    lx_account *to;
    const char *from_name = "agent:did:key:a:main";
    const char *to_name = "agent:did:key:b:main";
    uint8_t from_id[32];
    uint8_t to_id[32];
    lx_asset_transfer_request request;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_receipt receipt;
    lxp_arena arena;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;

    (void)memset(&asset, 0, sizeof(asset));
    asset.asset_id[0] = 1U;
    asset.symbol_length = 1U;
    (void)memcpy(asset.symbol, "A", 2U);
    asset.custody_kind = LX_ASSET_CUSTODY_PAXEER;
    asset.custody_reference[0] = 1U;
    asset.custody_reference_length = 1U;
    if (lx_asset_transfer_state(&asset, &asset_state) != LXP_OK ||
        lx_account_registry_init(&accounts) != LXP_OK ||
        lx_account_id_from_string((const uint8_t *)from_name, strlen(from_name),
                                  from_id) != LXP_OK ||
        lx_account_id_from_string((const uint8_t *)to_name, strlen(to_name),
                                  to_id) != LXP_OK ||
        lx_account_open(&accounts, (const uint8_t *)from_name, strlen(from_name),
                        from_id, 1U, LX_ACCOUNT_OPEN_CREDIT, NULL, &from) != LXP_OK ||
        lx_account_open(&accounts, (const uint8_t *)to_name, strlen(to_name),
                        to_id, 1U, LX_ACCOUNT_OPEN_CREDIT, NULL, &to) != LXP_OK ||
        lxp_ledger_bootstrap_balance(from, asset.asset_id,
                                     (lxp_u128){ 0U, 100U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(to, asset.asset_id,
                                     (lxp_u128){ 0U, 0U }, 0U) != LXP_OK ||
        lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_asset_module_iface()) != LXP_OK ||
        lxp_kernel_set_capabilities(&kernel, NULL, apply_capability) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_ASSET, 10U, 0U, 1U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    (void)memset(&request, 0, sizeof(request));
    request.from = from;
    request.to = to;
    request.asset = &asset;
    request.amount = (lxp_u128){ 0U, 25U };
    request.context.assets = &asset_state;
    request.context.asset_count = 1U;
    request.context.sequence_account = from;
    (void)memcpy(request.context.authorized_from, from_id, 32U);
    if (lx_asset_validate(&request) != LXP_OK || !balances(from, to, 100U, 0U) ||
        lx_asset_send_execute(&ctx, &request, &receipt) != LXP_OK ||
        !balances(from, to, 75U, 25U)) return 1;
    request.direct_balance_write = true;
    if (lx_asset_validate(&request) != LXP_ERR_BALANCE_BYPASS ||
        !balances(from, to, 75U, 25U)) return 1;
    request.direct_balance_write = false;
    request.amount = (lxp_u128){ 0U, 76U };
    if (lx_asset_validate(&request) != LXP_ERR_INSUFFICIENT_BALANCE ||
        !balances(from, to, 75U, 25U)) return 1;
    if (lxp_ledger_bootstrap_balance(to, asset.asset_id,
            (lxp_u128){ UINT64_MAX, UINT64_MAX }, 0U) != LXP_OK) return 1;
    request.amount = (lxp_u128){ 0U, 1U };
    if (lx_asset_validate(&request) != LXP_ERR_OVERFLOW ||
        from->balance.lo != 75U || to->balance.lo != UINT64_MAX) return 1;
    if (lxp_ledger_bootstrap_balance(to, asset.asset_id,
            (lxp_u128){ 0U, 25U }, 0U) != LXP_OK) return 1;
    request.amount = (lxp_u128){ 0U, 0U };
    if (lx_asset_validate(&request) != LXP_ERR_INVALID_AMOUNT ||
        !balances(from, to, 75U, 25U)) return 1;
    request.amount = (lxp_u128){ 0U, 1U };
    asset.asset_id[0] = 2U;
    if (lx_asset_validate(&request) != LXP_ERR_ASSET_MISMATCH ||
        !balances(from, to, 75U, 25U)) return 1;
    asset.asset_id[0] = 1U;
    request.payer_grant = NULL;
    if (lx_asset_receive_execute(&ctx, &request, &receipt) !=
        LXP_ERR_NO_PAYER_GRANT) return 1;
    if (lxp_state_store_destroy(&state) != LXP_OK) return 1;
    return 0;
}
