#include "layerx/lx_perps.h"
#include "layerx/lxp_kernel.h"

#include <string.h>

static lxp_transfer_set captured;

static lxp_result apply_capability(lxp_kernel *kernel,
                                   const lxp_transfer_set *set,
                                   lxp_receipt *receipt)
{
    lxp_transfer_set_result result;
    lxp_transfer_context context = set->context;
    lxp_result status;
    (void)kernel;
    captured = *set;
    status = lxp_apply_transfer_set((lxp_transfer_leg *)set->legs,
                                    set->leg_count, &context, &result);
    if (status == LXP_OK)
        (void)memcpy(receipt->transfer_set_root, result.transfer_set_root, 32U);
    return status;
}

static void account_init(lx_account *account, uint8_t id,
                         lx_account_kind kind, const uint8_t asset[32],
                         uint64_t balance)
{
    (void)memset(account, 0, sizeof(*account));
    account->id[0] = id;
    account->kind = kind;
    (void)lxp_ledger_bootstrap_balance(account, asset,
                                       (lxp_u128){ 0U, balance }, 0U);
}

static lx_perps_market market_make(void)
{
    lx_perps_market market;
    (void)memset(&market, 0, sizeof(market));
    market.market_id[0] = 1U;
    market.quote_asset[0] = 7U;
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
    uint8_t asset_id[32] = { 7U };
    lxp_transfer_asset_state asset = { { 0U }, true, false };
    lx_perps_market market = market_make();
    lx_perps_position position;
    lx_account margin;
    lx_account liquidity;
    lx_account liquidator;
    lx_account insurance;
    lx_account owner;
    lx_perps_liquidation_request request;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    lxp_receipt receipt;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;
    bool liquidatable;

    (void)memcpy(asset.asset_id, asset_id, 32U);
    (void)memset(&position, 0, sizeof(position));
    position.position_id[0] = 1U;
    position.market_id[0] = 1U;
    position.side = LX_PERPS_SIDE_BUY;
    position.size = (lxp_u128){ 0U, 100U };
    position.entry_notional = (lxp_u128){ 0U, 10000U };
    position.open = true;
    account_init(&margin, 1U, LX_ACCOUNT_AGENT_MARGIN, asset_id, 100U);
    account_init(&liquidity, 2U, LX_ACCOUNT_SYSTEM_LIQUIDITY, asset_id, 0U);
    account_init(&liquidator, 3U, LX_ACCOUNT_AGENT_MAIN, asset_id, 0U);
    account_init(&insurance, 4U, LX_ACCOUNT_SYSTEM_INSURANCE, asset_id, 100U);
    account_init(&owner, 5U, LX_ACCOUNT_AGENT_MAIN, asset_id, 0U);
    (void)memset(&request, 0, sizeof(request));
    request.position = &position;
    request.market = &market;
    request.margin_account = &margin;
    request.market_liquidity_account = &liquidity;
    request.liquidator_main_account = &liquidator;
    request.insurance_account = &insurance;
    request.owner_main_account = &owner;
    request.asset = &asset;
    request.mark_price = (lxp_u128){ 0U, 100U };
    request.price_scale = (lxp_u128){ 0U, 1U };
    request.trading_loss = (lxp_u128){ 0U, 30U };
    request.liquidation_fee_bps = 10U;
    request.liquidator_share_bps = 6000U;
    if (lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_perps_module_iface()) != LXP_OK ||
        lxp_kernel_set_capabilities(&kernel, NULL, apply_capability) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_PERPS, 100U, 0U, 1U,
                            1000U, &arena, true) != LXP_OK ||
        lx_perps_maintenance_check(&market, &position, request.mark_price,
                                   request.price_scale, margin.balance,
                                   &liquidatable) != LXP_OK || !liquidatable ||
        lx_perps_liquidate_execute(&ctx, &request, &receipt) != LXP_OK ||
        captured.leg_count != 4U ||
        captured.legs[0].reason != LXP_REASON_TRADING_LOSS ||
        captured.legs[0].amount.lo != 30U ||
        captured.legs[1].amount.lo != 6U ||
        captured.legs[2].amount.lo != 4U ||
        captured.legs[3].reason != LXP_REASON_MARGIN_RELEASE ||
        captured.legs[3].amount.lo != 60U || margin.balance.lo != 0U ||
        liquidity.balance.lo != 30U || liquidator.balance.lo != 6U ||
        insurance.balance.lo != 104U || owner.balance.lo != 60U ||
        position.open)
        return 1;
    position.open = true;
    account_init(&margin, 1U, LX_ACCOUNT_AGENT_MARGIN, asset_id, 20U);
    account_init(&liquidity, 2U, LX_ACCOUNT_SYSTEM_LIQUIDITY, asset_id, 0U);
    account_init(&liquidator, 3U, LX_ACCOUNT_AGENT_MAIN, asset_id, 0U);
    account_init(&insurance, 4U, LX_ACCOUNT_SYSTEM_INSURANCE, asset_id, 20U);
    account_init(&owner, 5U, LX_ACCOUNT_AGENT_MAIN, asset_id, 0U);
    request.trading_loss = (lxp_u128){ 0U, 40U };
    if (lx_perps_liquidate_execute(&ctx, &request, &receipt) != LXP_OK ||
        captured.leg_count != 2U ||
        captured.legs[0].amount.lo != 20U ||
        captured.legs[1].reason != LXP_REASON_INSURANCE ||
        captured.legs[1].amount.lo != 20U || margin.balance.lo != 0U ||
        liquidity.balance.lo != 40U || insurance.balance.lo != 0U ||
        lxp_state_store_destroy(&state) != LXP_OK)
        return 1;
    return 0;
}
