#include "layerx/lx_budget.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static uint16_t last_reason;

static lxp_result apply_capability(lxp_kernel *kernel,
                                   const lxp_transfer_set *set,
                                   lxp_receipt *receipt)
{
    lxp_transfer_set_result result;
    lxp_transfer_context context = set->context;
    lxp_result status;
    (void)kernel;
    last_reason = set->legs[0].reason;
    status = lxp_apply_transfer_set((lxp_transfer_leg *)set->legs,
                                    set->leg_count, &context, &result);
    if (status == LXP_OK)
        (void)memcpy(receipt->transfer_set_root, result.transfer_set_root, 32U);
    return status;
}

int main(void)
{
    lx_account budget_account;
    lx_account recipient;
    lx_asset_record asset;
    lxp_transfer_asset_state asset_state;
    lx_budget_store store;
    lx_budget_spend_request request;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    lxp_receipt receipt;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;
    lxp_u128 remaining;

    (void)memset(&budget_account, 0, sizeof(budget_account));
    (void)memset(&recipient, 0, sizeof(recipient));
    (void)memset(&asset, 0, sizeof(asset));
    (void)memset(&store, 0, sizeof(store));
    budget_account.id[0] = 1U;
    budget_account.kind = LX_ACCOUNT_AGENT_BUDGET;
    recipient.id[0] = 2U;
    recipient.kind = LX_ACCOUNT_AGENT_MAIN;
    asset.asset_id[0] = 3U;
    if (lxp_ledger_bootstrap_balance(&budget_account, asset.asset_id,
                                     (lxp_u128){ 0U, 200U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&recipient, asset.asset_id,
                                     (lxp_u128){ 0U, 0U }, 0U) != LXP_OK ||
        lx_asset_transfer_state(&asset, &asset_state) != LXP_OK ||
        lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_budget_module_iface()) != LXP_OK ||
        lxp_kernel_set_capabilities(&kernel, NULL, apply_capability) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_BUDGET, 199U, 0U, 1U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    store.count = 1U;
    store.records[0].budget_id[0] = 4U;
    (void)memcpy(store.records[0].budget_account, budget_account.id, 32U);
    (void)memcpy(store.records[0].asset_id, asset.asset_id, 32U);
    store.records[0].per_period_limit = (lxp_u128){ 0U, 100U };
    store.records[0].configured_period_limit = (lxp_u128){ 0U, 100U };
    store.records[0].period_length = 100U;
    store.records[0].period_start = 100U;
    store.records[0].rollover_policy = LX_BUDGET_ROLLOVER_NONE;
    store.records[0].expiry = 1000U;
    (void)memset(&request, 0, sizeof(request));
    request.store = &store;
    request.budget_id = store.records[0].budget_id;
    request.budget_account = &budget_account;
    request.recipient = &recipient;
    request.asset = &asset;
    request.amount = (lxp_u128){ 0U, 100U };
    request.context.assets = &asset_state;
    request.context.asset_count = 1U;
    request.context.sequence_account = &budget_account;
    (void)memcpy(request.context.authorized_from, budget_account.id, 32U);
    if (lx_budget_spend_execute(&ctx, &request, &receipt) != LXP_OK ||
        last_reason != LXP_REASON_BUDGET_SPEND || budget_account.balance.lo != 100U ||
        recipient.balance.lo != 100U || store.records[0].spent_this_period.lo != 100U)
        return 1;
    request.amount = (lxp_u128){ 0U, 1U };
    request.context.actor_sequence = budget_account.next_sequence;
    if (lx_budget_spend_execute(&ctx, &request, &receipt) !=
            LXP_ERR_BUDGET_ALLOWANCE_EXCEEDED || budget_account.balance.lo != 100U)
        return 1;
    if (lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_BUDGET, 200U, 0U, 2U,
                            1000U, &arena, true) != LXP_OK ||
        lx_budget_spend_execute(&ctx, &request, &receipt) != LXP_OK ||
        store.records[0].period_start != 200U ||
        store.records[0].spent_this_period.lo != 1U)
        return 1;
    budget_account.balance = (lxp_u128){ 0U, 0U };
    request.amount = (lxp_u128){ 0U, 1U };
    request.context.actor_sequence = budget_account.next_sequence;
    if (lx_budget_spend_execute(&ctx, &request, &receipt) !=
            LXP_ERR_INSUFFICIENT_BUDGET_FUNDS ||
        lx_budget_remaining(&store.records[0], &budget_account,
                            &remaining) != LXP_OK ||
        !lxp_u128_is_zero(remaining) ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
