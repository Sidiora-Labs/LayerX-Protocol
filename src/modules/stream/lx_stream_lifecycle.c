#include "layerx/lx_stream.h"

#include "layerx/lxp_crypto.h"

#include <stdbool.h>
#include <string.h>

static lxp_result lifecycle_record(
    const lx_stream_lifecycle_request *request, lx_stream_record **record)
{
    lxp_result status;
    if (request == NULL || request->store == NULL ||
        request->stream_id == NULL || request->authority == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = lx_stream_lookup(request->store, request->stream_id, record);
    if (status != LXP_OK) return status;
    if ((*record)->closed) return LXP_ERR_STREAM_CLOSED;
    if (memcmp(request->authority->principal, (*record)->payer, 32U) != 0)
        return LXP_ERR_UNAUTHORIZED_DEBIT;
    return LXP_OK;
}

lxp_result lx_stream_pause_execute(
    lxp_module_ctx *ctx, const lx_stream_lifecycle_request *request)
{
    lx_stream_record *record;
    lx_stream_record updated;
    lxp_u128 accrued;
    lxp_result status;
    if (ctx == NULL) return LXP_ERR_NON_CANONICAL;
    status = lifecycle_record(request, &record);
    if (status != LXP_OK) return status;
    if (record->paused) return LXP_OK;
    updated = *record;
    if (updated.mode == LX_STREAM_MODE_TIME) {
        status = lx_stream_accrue(&updated,
                                  lxp_ctx_batch_timestamp_ms(ctx), &accrued);
        if (status != LXP_OK) return status;
    }
    updated.paused = true;
    *record = updated;
    return LXP_OK;
}

lxp_result lx_stream_resume_execute(
    lxp_module_ctx *ctx, const lx_stream_lifecycle_request *request)
{
    lx_stream_record *record;
    uint64_t timestamp;
    lxp_result status;
    if (ctx == NULL) return LXP_ERR_NON_CANONICAL;
    status = lifecycle_record(request, &record);
    if (status != LXP_OK) return status;
    if (!record->paused) return LXP_OK;
    timestamp = lxp_ctx_batch_timestamp_ms(ctx);
    if (timestamp < record->last_accrual_timestamp)
        return LXP_ERR_NON_MONOTONIC_TIME;
    record->last_accrual_timestamp = timestamp;
    record->paused = false;
    return LXP_OK;
}

static lxp_result close_accounts_check(
    const lx_stream_lifecycle_request *request,
    const lx_stream_record *record)
{
    if (request->stream_account == NULL || request->payer == NULL ||
        request->recipient == NULL || request->asset == NULL ||
        memcmp(request->stream_account->id, record->stream_account, 32U) != 0 ||
        memcmp(request->payer->id, record->payer, 32U) != 0 ||
        memcmp(request->recipient->id, record->recipient, 32U) != 0 ||
        memcmp(request->asset->asset_id, record->asset_id, 32U) != 0)
        return LXP_ERR_NON_CANONICAL;
    return LXP_OK;
}

static void close_leg(lxp_transfer_leg *leg, lx_account *from,
                      lx_account *to, const uint8_t asset_id[32],
                      lxp_u128 amount, uint16_t reason)
{
    leg->from = from;
    leg->to = to;
    (void)memcpy(leg->asset_id, asset_id, 32U);
    leg->amount = amount;
    leg->reason = reason;
}

lxp_result lx_stream_close_execute(
    lxp_module_ctx *ctx, const lx_stream_lifecycle_request *request,
    lxp_receipt *receipt)
{
    lx_stream_record *record;
    lx_stream_record updated;
    lxp_transfer_set set;
    lxp_u128 accrued;
    lxp_u128 unsettled;
    lxp_u128 balance;
    lxp_u128 payment;
    lxp_u128 refund;
    lxp_u128 settled;
    lxp_result status;
    bool found;
    if (ctx == NULL || request == NULL || receipt == NULL ||
        lxp_ct_is_zero(request->idempotency_key, 32U))
        return LXP_ERR_NON_CANONICAL;
    status = lx_stream_receipt_replay(request->store,
                                      request->idempotency_key,
                                      receipt, &found);
    if (status != LXP_OK || found) return status;
    status = lifecycle_record(request, &record);
    if (status != LXP_OK) return status;
    status = close_accounts_check(request, record);
    if (status != LXP_OK) return status;
    if (request->store->economic_result_count ==
        LX_STREAM_IDEMPOTENCY_CAPACITY) return LXP_ERR_ARENA_EXHAUSTED;
    updated = *record;
    if (updated.mode == LX_STREAM_MODE_TIME && !updated.paused &&
        !updated.underfunded) {
        status = lx_stream_accrue(&updated,
                                  lxp_ctx_batch_timestamp_ms(ctx), &accrued);
        if (status != LXP_OK) return status;
    }
    status = lx_stream_settle_amount(&updated, &unsettled);
    if (status == LXP_OK)
        status = lxp_state_balance_get(request->stream_account,
                                       record->asset_id, &balance);
    if (status != LXP_OK) return status;
    payment = lxp_u128_cmp(unsettled, balance) > 0 ? balance : unsettled;
    status = lxp_u128_sub(balance, payment, &refund);
    if (status != LXP_OK) return status;
    (void)memset(&set, 0, sizeof(set));
    if (!lxp_u128_is_zero(payment)) {
        close_leg(&set.legs[set.leg_count], request->stream_account,
                  request->recipient, record->asset_id, payment,
                  LXP_REASON_STREAM_DRAW);
        ++set.leg_count;
    }
    if (!lxp_u128_is_zero(refund)) {
        close_leg(&set.legs[set.leg_count], request->stream_account,
                  request->payer, record->asset_id, refund,
                  LXP_REASON_STREAM_REFUND);
        ++set.leg_count;
    }
    set.context = request->context;
    set.context.debit_authority_kind = LXP_AUTH_PROTOCOL_MODULE;
    if (set.leg_count != 0U) {
        status = lxp_ctx_emit_transfer_set(ctx, &set, receipt);
        if (status != LXP_OK) return status;
    } else {
        (void)memset(receipt, 0, sizeof(*receipt));
    }
    status = lxp_u128_add(updated.settled_total, payment, &settled);
    if (status != LXP_OK) return status;
    updated.settled_total = settled;
    updated.accrued_total = settled;
    updated.remainder_carry = (lxp_u128){ 0U, 0U };
    updated.closed = true;
    updated.paused = false;
    updated.underfunded = false;
    *record = updated;
    return lx_stream_receipt_record(request->store,
                                    request->idempotency_key, receipt);
}
