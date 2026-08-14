#include "layerx/lxp_gateway.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_hash.h"

#include <string.h>

lxp_result lxp_gateway_invoice_state(
    const lxp_gateway_invoice_registry *registry,
    const uint8_t invoice_id[32],
    const uint8_t idempotency_key[32],
    lxp_receipt *receipt,
    bool *settled)
{
    size_t i;
    if (registry == NULL || invoice_id == NULL || idempotency_key == NULL ||
        receipt == NULL || settled == NULL)
        return LXP_ERR_NON_CANONICAL;
    *settled = false;
    for (i = 0U; i < registry->count; ++i) {
        if (lxp_ct_memcmp(registry->records[i].invoice_id,
                          invoice_id, 32U) == 0 &&
            lxp_ct_memcmp(registry->records[i].idempotency_key,
                          idempotency_key, 32U) == 0) {
            *receipt = registry->records[i].receipt;
            *settled = true;
            return LXP_OK;
        }
    }
    return LXP_OK;
}

static lxp_result requirement_matches_send(
    const lxp_payment_requirement *requirement,
    const lxp_send *send,
    const lxp_send_environment *environment)
{
    size_t i;
    if (requirement == NULL || send == NULL || environment == NULL)
        return LXP_ERR_NON_CANONICAL;
    if (send->expires_at == 0U || send->expires_at > requirement->expiry ||
        environment->batch_timestamp > requirement->expiry ||
        environment->batch_timestamp > send->expires_at)
        return LXP_ERR_EXPIRED;
    if (lxp_ct_memcmp(send->to, requirement->recipient, 32U) != 0 ||
        lxp_ct_memcmp(send->asset, requirement->asset, 32U) != 0 ||
        lxp_u128_cmp(send->amount, requirement->amount) != 0)
        return LXP_ERR_GRANT_SCOPE_VIOLATION;
    if (lxp_ct_memcmp(send->context_hash,
                      requirement->purpose_hash, 32U) != 0)
        return LXP_ERR_CONTEXT_MISMATCH;
    for (i = 0U; i < send->condition_count; ++i) {
        uint32_t condition_bit;
        if (send->conditions[i].kind == 0U ||
            send->conditions[i].kind >= 32U)
            return LXP_ERR_CONDITION_UNMET;
        condition_bit = UINT32_C(1) << send->conditions[i].kind;
        if ((requirement->acceptable_conditions & condition_bit) == 0U)
            return LXP_ERR_CONDITION_UNMET;
    }
    return LXP_OK;
}

static lxp_result build_signed_receipt(
    const lxp_send *send,
    const lxp_send_receipt_projection *projection,
    const uint8_t previous_state_root[32],
    const uint8_t resulting_state_root[32],
    lxp_gateway_settlement_context *context,
    lxp_receipt *receipt)
{
    uint8_t encoded[512];
    uint8_t authorization[512];
    uint8_t transaction_id[32];
    uint8_t authorization_hash[32];
    size_t encoded_length = 0U;
    size_t authorization_length = 0U;
    lxp_ledger_receipt_input input;
    lxp_result status = lxp_send_encode(
        send, encoded, sizeof(encoded), &encoded_length);
    if (status == LXP_OK)
        status = lxp_hash_activity_id(
            encoded, encoded_length, transaction_id);
    if (status == LXP_OK)
        status = lxp_send_authorization_message(
            send, authorization, sizeof(authorization),
            &authorization_length);
    if (status == LXP_OK)
        status = lxp_hash_domain(
            LXP_DOMAIN_SIGNATURE_PREIMAGE, authorization,
            authorization_length, authorization_hash);
    if (status != LXP_OK) return status;
    (void)memset(&input, 0, sizeof(input));
    (void)memcpy(input.transaction_id, transaction_id, 32U);
    input.operation = (uint8_t)LX_ASSET_SEND;
    input.global_sequence = context->global_sequence;
    (void)memcpy(input.asset, send->asset, 32U);
    input.amount = send->amount;
    (void)memcpy(input.from, send->from, 32U);
    input.from_balance_before = projection->from_before;
    input.from_balance_after = projection->from_after;
    input.from_sequence = send->sequence;
    (void)memcpy(input.to, send->to, 32U);
    input.to_balance_before = projection->to_before;
    input.to_balance_after = projection->to_after;
    (void)memcpy(input.transfer_set_root,
                 projection->transfer_set_root, 32U);
    (void)memcpy(input.authorization_hash, authorization_hash, 32U);
    (void)memcpy(input.context_hash, send->context_hash, 32U);
    (void)memcpy(input.previous_state_root, previous_state_root, 32U);
    (void)memcpy(input.resulting_state_root, resulting_state_root, 32U);
    (void)memcpy(input.batch_id, context->batch_id, 32U);
    input.timestamp = context->send_environment->batch_timestamp;
    input.leg_count = 1U;
    status = lxp_ledger_receipt_build(receipt, &input);
    if (status == LXP_OK)
        status = lxp_receipt_sign(
            receipt, context->sequencer_private_key, context->arena);
    return status;
}

lxp_result lxp_gateway_send_settle(
    const lxp_payment_requirement *requirement,
    const lxp_send *send,
    lxp_gateway_settlement_context *context,
    lxp_receipt *receipt)
{
    lxp_send_receipt_projection projection;
    uint8_t previous_state_root[32];
    uint8_t resulting_state_root[32];
    bool settled = false;
    lxp_result status;
    if (requirement == NULL || send == NULL || context == NULL ||
        context->assets == NULL || context->send_environment == NULL ||
        context->send_environment->accounts == NULL ||
        context->invoices == NULL || context->service_public_key == NULL ||
        context->sequencer_private_key == NULL ||
        context->arena == NULL || receipt == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = lxp_payment_requirement_verify(
        requirement, context->send_environment->network_id,
        context->service_public_key);
    if (status != LXP_OK) return status;
    status = lxp_gateway_invoice_state(
        context->invoices, requirement->invoice_id,
        send->idempotency_key, receipt, &settled);
    if (status != LXP_OK) return status;
    if (settled) return LXP_ERR_IDEMPOTENT_REPLAY;
    if (context->invoices->count == LXP_GATEWAY_INVOICE_CAPACITY)
        return LXP_ERR_ARENA_EXHAUSTED;
    status = requirement_matches_send(
        requirement, send, context->send_environment);
    if (status != LXP_OK) return status;
    status = lx_asset_state_root(
        context->assets, context->send_environment->accounts,
        previous_state_root);
    if (status != LXP_OK) return status;
    status = lxp_send_execute(
        send, context->send_environment, &projection);
    if (status != LXP_OK) return status;
    status = lx_asset_state_root(
        context->assets, context->send_environment->accounts,
        resulting_state_root);
    if (status != LXP_OK) return status;
    status = build_signed_receipt(
        send, &projection, previous_state_root, resulting_state_root,
        context, receipt);
    if (status != LXP_OK) return status;
    (void)memcpy(
        context->invoices->records[context->invoices->count].invoice_id,
        requirement->invoice_id, 32U);
    (void)memcpy(
        context->invoices->records[context->invoices->count].idempotency_key,
        send->idempotency_key, 32U);
    context->invoices->records[context->invoices->count].receipt = *receipt;
    ++context->invoices->count;
    return LXP_OK;
}

lxp_result lxp_gateway_receipt_return(
    const lxp_receipt *receipt,
    lxp_arena *arena,
    lxp_byte_span *canonical_receipt)
{
    if (receipt == NULL || arena == NULL || canonical_receipt == NULL ||
        lxp_ct_is_zero(receipt->sequencer_signature, 64U))
        return LXP_ERR_BAD_SIGNATURE;
    return lxp_receipt_encode(receipt, true, arena, canonical_receipt);
}
