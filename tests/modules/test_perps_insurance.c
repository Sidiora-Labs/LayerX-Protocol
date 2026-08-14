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

int main(void)
{
    uint8_t asset_id[32] = { 1U };
    uint8_t market_id[32] = { 2U };
    lxp_transfer_asset_state asset = { { 1U }, true, false };
    lx_account insurance;
    lx_account liquidity;
    lx_account margins[3];
    lx_perps_position positions[3];
    lx_perps_adl_candidate candidates[3];
    lx_perps_deficit_store deficits;
    lxp_state_store state;
    lxp_state_journal journal;
    lxp_kernel kernel;
    lxp_module_ctx ctx;
    lxp_arena arena;
    lxp_receipt receipt;
    lxp_u128 remaining;
    uint8_t arena_bytes[4096];
    uint64_t parameters = 1U;
    size_t i;
    uint64_t before;

    account_init(&insurance, 9U, LX_ACCOUNT_SYSTEM_INSURANCE, asset_id, 100U);
    account_init(&liquidity, 8U, LX_ACCOUNT_SYSTEM_LIQUIDITY, asset_id, 0U);
    (void)memset(&deficits, 0, sizeof(deficits));
    if (lxp_state_store_init(&state, 0U) != LXP_OK ||
        lxp_kernel_create(&kernel, &state, &journal, &parameters, 0U) != LXP_OK ||
        lxp_kernel_register_module(&kernel, lx_perps_module_iface()) != LXP_OK ||
        lxp_kernel_set_capabilities(&kernel, NULL, apply_capability) != LXP_OK ||
        lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK ||
        lxp_module_ctx_init(&ctx, &kernel, LXP_MODULE_PERPS, 100U, 0U, 1U,
                            1000U, &arena, true) != LXP_OK ||
        lx_perps_insurance_cover(
            &ctx, &insurance, &liquidity, &asset, (lxp_u128){ 0U, 30U },
            (lxp_transfer_context){ 0 }, &receipt) != LXP_OK ||
        captured.leg_count != 1U || captured.legs[0].amount.lo != 30U ||
        insurance.balance.lo != 70U || liquidity.balance.lo != 30U)
        return 1;
    before = insurance.balance.lo;
    if (lx_perps_insurance_cover(
            &ctx, &insurance, &liquidity, &asset, (lxp_u128){ 0U, 71U },
            (lxp_transfer_context){ 0 }, &receipt) !=
            LXP_ERR_INSUFFICIENT_BALANCE || insurance.balance.lo != before ||
        lx_perps_deficit_record(&deficits, market_id, insurance.id,
                                (lxp_u128){ 0U, 71U }, 10U) != LXP_OK ||
        deficits.count != 1U || deficits.deficits[0].amount.lo != 71U)
        return 1;
    for (i = 0U; i < 3U; ++i) {
        account_init(&margins[i], (uint8_t)(i + 1U),
                     LX_ACCOUNT_AGENT_MARGIN, asset_id, 30U);
        (void)memset(&positions[i], 0, sizeof(positions[i]));
        positions[i].position_id[0] = (uint8_t)(3U - i);
        (void)memcpy(positions[i].margin_account_id, margins[i].id, 32U);
        positions[i].open = true;
        candidates[i].position = &positions[i];
        candidates[i].margin_account = &margins[i];
        candidates[i].maximum_contribution = (lxp_u128){ 0U, 30U };
    }
    if (lx_perps_adl_execute(
            &ctx, candidates, 3U, &liquidity, &asset,
            (lxp_u128){ 0U, 70U }, (lxp_transfer_context){ 0 }, &receipt,
            &remaining) != LXP_OK || !lxp_u128_is_zero(remaining) ||
        captured.leg_count != 3U ||
        captured.legs[0].from->id[0] != 3U ||
        captured.legs[1].from->id[0] != 2U ||
        captured.legs[2].from->id[0] != 1U ||
        captured.legs[0].amount.lo != 30U ||
        captured.legs[1].amount.lo != 30U ||
        captured.legs[2].amount.lo != 10U ||
        liquidity.balance.lo != 100U || margins[0].balance.lo != 20U ||
        margins[1].balance.lo != 0U || margins[2].balance.lo != 0U)
        return 1;
    for (i = 1U; i <= 256U; ++i) {
        lx_account source;
        lx_account destination;
        uint64_t amount = (i * UINT64_C(1103515245) + UINT64_C(12345)) % 97U + 1U;
        account_init(&source, 20U, LX_ACCOUNT_SYSTEM_INSURANCE,
                     asset_id, amount);
        account_init(&destination, 21U, LX_ACCOUNT_SYSTEM_LIQUIDITY,
                     asset_id, 0U);
        if (lx_perps_insurance_cover(
                &ctx, &source, &destination, &asset,
                (lxp_u128){ 0U, amount }, (lxp_transfer_context){ 0 },
                &receipt) != LXP_OK || source.balance.lo != 0U ||
            destination.balance.lo != amount)
            return 1;
    }
    if (lxp_state_store_destroy(&state) != LXP_OK) return 1;
    return 0;
}
