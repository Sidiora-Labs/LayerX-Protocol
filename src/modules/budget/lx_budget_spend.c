#include "layerx/lx_budget.h"

#include <string.h>

lxp_result lx_budget_allowance_debit(lx_budget_record *record,
                                     lxp_u128 amount)
{
    lxp_u128 available;
    lxp_u128 updated;
    lxp_result status;
    if (record == NULL || lxp_u128_is_zero(amount))
        return LXP_ERR_INVALID_AMOUNT;
    status = lxp_u128_sub(record->per_period_limit,
                          record->spent_this_period, &available);
    if (status != LXP_OK) return LXP_FATAL_INVARIANT;
    if (lxp_u128_cmp(amount, available) > 0)
        return LXP_ERR_BUDGET_ALLOWANCE_EXCEEDED;
    status = lxp_u128_add(record->spent_this_period, amount, &updated);
    if (status != LXP_OK) return status;
    record->spent_this_period = updated;
    return LXP_OK;
}

lxp_result lx_budget_remaining(lx_budget_record *record,
                               const lx_account *budget_account,
                               lxp_u128 *remaining)
{
    lxp_u128 allowance;
    lxp_u128 balance;
    lxp_result status;
    if (record == NULL || budget_account == NULL || remaining == NULL ||
        memcmp(record->budget_account, budget_account->id, 32U) != 0)
        return LXP_ERR_NON_CANONICAL;
    status = lxp_u128_sub(record->per_period_limit,
                          record->spent_this_period, &allowance);
    if (status != LXP_OK) return LXP_FATAL_INVARIANT;
    status = lxp_state_balance_get(budget_account, record->asset_id, &balance);
    if (status != LXP_OK) return status;
    *remaining = lxp_u128_cmp(allowance, balance) < 0 ? allowance : balance;
    return LXP_OK;
}

lxp_result lx_budget_spend_execute(lxp_module_ctx *ctx,
                                   const lx_budget_spend_request *request,
                                   lxp_receipt *receipt)
{
    lx_budget_record *record;
    lx_budget_record updated;
    lxp_transfer_set set;
    lxp_u128 allowance;
    lxp_u128 balance;
    lxp_result status;
    if (ctx == NULL || request == NULL || request->store == NULL ||
        request->budget_id == NULL || request->budget_account == NULL ||
        request->recipient == NULL || request->asset == NULL || receipt == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = lx_budget_lookup(request->store, request->budget_id, &record);
    if (status != LXP_OK || record->closed) return LXP_ERR_UNKNOWN_FIELD;
    if (record->revoked) return LXP_ERR_BUDGET_REVOKED;
    if (record->expiry <= lxp_ctx_batch_timestamp_ms(ctx))
        return LXP_ERR_EXPIRED;
    status = lx_budget_rollover(record, lxp_ctx_batch_timestamp_ms(ctx));
    if (status != LXP_OK) return status;
    if (memcmp(record->budget_account, request->budget_account->id, 32U) != 0 ||
        memcmp(record->asset_id, request->asset->asset_id, 32U) != 0 ||
        request->budget_account->kind != LX_ACCOUNT_AGENT_BUDGET ||
        request->recipient->kind != LX_ACCOUNT_AGENT_MAIN)
        return LXP_ERR_NON_CANONICAL;
    status = lxp_u128_sub(record->per_period_limit,
                          record->spent_this_period, &allowance);
    if (status != LXP_OK) return LXP_FATAL_INVARIANT;
    if (lxp_u128_cmp(request->amount, allowance) > 0)
        return LXP_ERR_BUDGET_ALLOWANCE_EXCEEDED;
    status = lxp_state_balance_get(request->budget_account, record->asset_id,
                                   &balance);
    if (status != LXP_OK) return status;
    if (lxp_u128_cmp(request->amount, balance) > 0)
        return LXP_ERR_INSUFFICIENT_BUDGET_FUNDS;
    updated = *record;
    status = lx_budget_allowance_debit(&updated, request->amount);
    if (status != LXP_OK) return status;

    (void)memset(&set, 0, sizeof(set));
    set.leg_count = 1U;
    set.legs[0].from = request->budget_account;
    set.legs[0].to = request->recipient;
    (void)memcpy(set.legs[0].asset_id, record->asset_id, 32U);
    set.legs[0].amount = request->amount;
    set.legs[0].reason = LXP_REASON_BUDGET_SPEND;
    set.context = request->context;
    set.context.debit_authority_kind = LXP_AUTH_BUDGET_ALLOWANCE;
    status = lxp_ctx_emit_transfer_set(ctx, &set, receipt);
    if (status != LXP_OK) return status;
    *record = updated;
    return LXP_OK;
}
