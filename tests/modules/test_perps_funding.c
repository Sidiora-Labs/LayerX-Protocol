#include "layerx/lx_perps.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static lxp_result apply_capability(lxp_kernel *kernel,
                                   const lxp_transfer_set *set,
                                   lxp_receipt *receipt)
{
    lxp_transfer_set_result result;
    lxp_transfer_context context = set->context;
    lxp_result status;
    (void)kernel;
    if (set->leg_count != 1U ||
        set->legs[0].reason != LXP_REASON_FUNDING)
        return LXP_FATAL_INVARIANT;
    status = lxp_apply_transfer_set((lxp_transfer_leg *)set->legs,
                                    set->leg_count, &context, &result);
    if (status == LXP_OK)
        (void)memcpy(receipt->transfer_set_root, result.transfer_set_root, 32U);
    return status;
}

static lx_perps_market market_make(void)
{
    lx_perps_market market;
    (void)memset(&market, 0, sizeof(market));
    market.market_id[0] = 1U;
    market.quote_asset[0] = 2U;
    market.contract_size = (lxp_u128){ 0U, 1U };
    market.tick_size = (lxp_u128){ 0U, 1U };
    market.lot_size = (lxp_u128){ 0U, 1U };
    market.initial_margin_ratio_bps = 1000U;
    market.maintenance_margin_ratio_bps = 500U;
    market.funding_interval_ms = 100U;
    market.maximum_oracle_staleness_ms = 100U;
    market.minimum_price = (lxp_u128){ 0U, 1U };
    market.maximum_price = (lxp_u128){ 0U, 1000U };
    market.permitted_oracle_key_count = 1U;
    market.permitted_oracle_keys[0][0] = 1U;
    market.parameter_version = 1U;
    return market;
}

int main(void)
{
    lx_perps_market market = market_make();
    lxp_i128 pnl;
    lxp_i128 rate;
    lxp_i128 index = { false, { 0U, 10U } };
    lxp_i128 expected;
    lx_account long_account;
    lx_account short_account;
    lxp_transfer_asset_state asset = { { 3U }, true, false };
    lx_perps_funding_tick_request request;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    lxp_receipt receipt;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;
    uint64_t last_timestamp = 100U;
    uint32_t bps;

    for (bps = 0U; bps <= LXP_BASIS_POINTS_ONE; bps += 100U) {
        if (lx_perps_pnl_compute(
                LX_PERPS_SIDE_BUY, (lxp_u128){ 0U, 100U },
                (lxp_u128){ 0U, 101U }, (lxp_u128){ 0U, bps + 1U },
                (lxp_u128){ 0U, 3U }, &pnl) != LXP_OK || pnl.negative)
            return 1;
    }
    if (lx_perps_pnl_compute(
            LX_PERPS_SIDE_BUY, (lxp_u128){ 0U, 100U },
            (lxp_u128){ 0U, 99U }, (lxp_u128){ 0U, 1U },
            (lxp_u128){ 0U, 3U }, &pnl) != LXP_OK ||
        !pnl.negative || pnl.magnitude.lo != 1U ||
        lx_perps_funding_rate(&market, (lxp_u128){ 0U, 105U },
                              (lxp_u128){ 0U, 100U }, 1000U, &rate) != LXP_OK ||
        rate.negative || rate.magnitude.lo != 500U ||
        lx_perps_funding_index_update(index, rate, 2U, &expected) != LXP_OK ||
        expected.negative || expected.magnitude.lo != 1010U)
        return 1;
    (void)memset(&long_account, 0, sizeof(long_account));
    (void)memset(&short_account, 0, sizeof(short_account));
    long_account.id[0] = 4U;
    short_account.id[0] = 5U;
    long_account.kind = LX_ACCOUNT_SYSTEM_FUNDING_LONG;
    short_account.kind = LX_ACCOUNT_SYSTEM_FUNDING_SHORT;
    if (lxp_ledger_bootstrap_balance(&long_account, asset.asset_id,
                                     (lxp_u128){ 0U, 1000U }, 0U) != LXP_OK ||
        lxp_ledger_bootstrap_balance(&short_account, asset.asset_id,
                                     (lxp_u128){ 0U, 1000U }, 0U) != LXP_OK ||
        lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_perps_module_iface()) != LXP_OK ||
        lxp_kernel_set_capabilities(&kernel, NULL, apply_capability) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_PERPS, 300U, 0U, 1U,
                            1000U, &arena, true) != LXP_OK)
        return 1;
    (void)memset(&request, 0, sizeof(request));
    request.market = &market;
    request.long_funding_account = &long_account;
    request.short_funding_account = &short_account;
    request.asset = &asset;
    request.funding_rate_bps = rate;
    request.open_notional = (lxp_u128){ 0U, 1000U };
    request.last_funding_timestamp_ms = &last_timestamp;
    request.funding_index = &index;
    if (lx_perps_funding_tick_execute(&ctx, &request, &receipt) != LXP_OK ||
        long_account.balance.lo != 950U || short_account.balance.lo != 1050U ||
        index.magnitude.lo != 1010U || last_timestamp != 300U)
        return 1;
    market.halted = true;
    if (lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_PERPS, 400U, 0U, 1U,
                            1000U, &arena, true) != LXP_OK ||
        lx_perps_funding_tick_execute(&ctx, &request, &receipt) !=
            LXP_ERR_MARKET_HALTED ||
        long_account.balance.lo != 950U || short_account.balance.lo != 1050U ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
