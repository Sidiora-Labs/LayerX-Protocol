#include "layerx/lx_asset.h"
#include "layerx/lxp_bridge.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_hash.h"
#include "layerx/lxp_receipt.h"

#include <string.h>

static void store_u32(uint8_t out[4], uint32_t value)
{
    out[0] = (uint8_t)(value >> 24U);
    out[1] = (uint8_t)(value >> 16U);
    out[2] = (uint8_t)(value >> 8U);
    out[3] = (uint8_t)value;
}

lxp_result lx_deposit_proof_commitment(const lx_deposit_proof *proof,
                                       uint8_t commitment[32])
{
    static const uint8_t tag[] = "LX:DEPOSIT:PROOF:v1";
    uint8_t input[sizeof(tag) - 1U + 32U * 6U + 16U + 4U + 2U + 1U];
    uint8_t amount[16];
    size_t cursor = 0U;
    lxp_result status;
    if (proof == NULL || commitment == NULL) return LXP_ERR_NON_CANONICAL;
    status = lxp_u128_to_be(proof->amount, amount);
    if (status != LXP_OK) return status;
    (void)memcpy(input + cursor, tag, sizeof(tag) - 1U);
    cursor += sizeof(tag) - 1U;
#define COPY_FIELD(field) do { \
    (void)memcpy(input + cursor, proof->field, 32U); cursor += 32U; \
} while (0)
    COPY_FIELD(deposit_id);
    COPY_FIELD(custody_reference);
    COPY_FIELD(asset_id);
    (void)memcpy(input + cursor, amount, sizeof(amount)); cursor += sizeof(amount);
    COPY_FIELD(checkpoint_id);
    COPY_FIELD(checkpoint_state_root);
#undef COPY_FIELD
    store_u32(input + cursor, proof->network_id); cursor += 4U;
    input[cursor++] = (uint8_t)(proof->protocol_version >> 8U);
    input[cursor++] = (uint8_t)proof->protocol_version;
    input[cursor++] = proof->finalized ? 1U : 0U;
    return lxp_hash_sha256(input, cursor, commitment);
}

lxp_result lx_bridge_verify_deposit(const lx_deposit_proof *proof,
                                    const lx_checkpoint_registry *checkpoints,
                                    uint32_t network_id,
                                    uint16_t protocol_version)
{
    uint8_t expected[32];
    size_t i;
    if (proof == NULL || checkpoints == NULL || !proof->finalized ||
        proof->network_id != network_id ||
        proof->protocol_version != protocol_version ||
        lxp_ct_is_zero(proof->deposit_id, 32U) ||
        lxp_ct_is_zero(proof->custody_reference, 32U) ||
        lxp_u128_is_zero(proof->amount) ||
        lx_deposit_proof_commitment(proof, expected) != LXP_OK ||
        memcmp(expected, proof->commitment, 32U) != 0)
        return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
    for (i = 0U; i < checkpoints->count; ++i)
        if (checkpoints->checkpoints[i].finalized &&
            memcmp(checkpoints->checkpoints[i].checkpoint_id,
                   proof->checkpoint_id, 32U) == 0 &&
            memcmp(checkpoints->checkpoints[i].state_root,
                   proof->checkpoint_state_root, 32U) == 0) return LXP_OK;
    return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
}

static bool nullifier_seen(const lx_deposit_nullifier_store *store,
                           const lx_deposit_proof *proof)
{
    uint8_t nullifier[32];
    size_t i;
    if (store == NULL || lxp_deposit_nullifier(proof, nullifier) != LXP_OK)
        return true;
    for (i = 0U; i < store->count; ++i)
        if (memcmp(store->nullifiers[i], nullifier, 32U) == 0) return true;
    return false;
}

lxp_result lx_deposit_nullifier_consume(lx_deposit_nullifier_store *store,
                                        const lx_deposit_proof *proof)
{
    uint8_t nullifier[32];
    if (store == NULL || proof == NULL) return LXP_ERR_NON_CANONICAL;
    if (nullifier_seen(store, proof)) return LXP_ERR_DEPOSIT_ALREADY_CREDITED;
    if (store->count == LX_DEPOSIT_NULLIFIER_CAPACITY)
        return LXP_ERR_ARENA_EXHAUSTED;
    if (lxp_deposit_nullifier(proof, nullifier) != LXP_OK)
        return LXP_ERR_NON_CANONICAL;
    (void)memcpy(store->nullifiers[store->count++], nullifier, 32U);
    return LXP_OK;
}

lxp_result lx_asset_deposit_credit(lxp_module_ctx *ctx,
                                   const lx_asset_transfer_request *request,
                                   const lx_deposit_proof *proof,
                                   const lx_checkpoint_registry *checkpoints,
                                   lx_deposit_nullifier_store *nullifiers,
                                   uint32_t network_id,
                                   uint16_t protocol_version,
                                   lxp_receipt *receipt)
{
    lxp_transfer_set set;
    lxp_result status;
    if (ctx == NULL || request == NULL || request->from == NULL ||
        request->to == NULL || request->asset == NULL || nullifiers == NULL)
        return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
    status = lx_bridge_verify_deposit(proof, checkpoints, network_id,
                                      protocol_version);
    if (status != LXP_OK) return status;
    if (nullifier_seen(nullifiers, proof))
        return LXP_ERR_DEPOSIT_ALREADY_CREDITED;
    if (nullifiers->count == LX_DEPOSIT_NULLIFIER_CAPACITY)
        return LXP_ERR_ARENA_EXHAUSTED;
    if (request->from->kind != LX_ACCOUNT_SYSTEM_PAXEER_RESERVE ||
        request->to->kind != LX_ACCOUNT_AGENT_MAIN ||
        memcmp(request->asset->asset_id, proof->asset_id, 32U) != 0 ||
        lxp_u128_cmp(request->amount, proof->amount) != 0)
        return LXP_ERR_GRANT_SCOPE_VIOLATION;
    (void)memset(&set, 0, sizeof(set));
    set.leg_count = 1U;
    set.legs[0].from = request->from;
    set.legs[0].to = request->to;
    (void)memcpy(set.legs[0].asset_id, proof->asset_id, 32U);
    set.legs[0].amount = proof->amount;
    set.legs[0].reason = LXP_REASON_DEPOSIT;
    set.context = request->context;
    set.context.protocol_system_capability = true;
    status = lxp_ctx_emit_transfer_set(ctx, &set, receipt);
    if (status != LXP_OK) return status;
    return lx_deposit_nullifier_consume(nullifiers, proof);
}

lxp_result lx_withdrawal_nullifier(const lx_withdrawal_request *request,
                                   uint8_t nullifier[32])
{
    return lxp_withdrawal_nullifier(request, nullifier);
}

bool lx_asset_nullifier_seen(const lx_withdrawal_store *store,
                             const uint8_t nullifier[32])
{
    size_t i;
    if (store == NULL || nullifier == NULL) return false;
    for (i = 0U; i < store->count; ++i)
        if (memcmp(store->records[i].nullifier, nullifier, 32U) == 0)
            return true;
    return false;
}

lxp_result lx_asset_withdraw_request(lxp_module_ctx *ctx,
                                     const lx_asset_transfer_request *transfer,
                                     const lx_withdrawal_request *withdrawal,
                                     lx_withdrawal_store *store,
                                     lxp_receipt *receipt)
{
    lxp_transfer_set set;
    uint8_t nullifier[32];
    lxp_result status;
    if (ctx == NULL || transfer == NULL || withdrawal == NULL || store == NULL ||
        transfer->from == NULL || transfer->to == NULL || transfer->asset == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = lx_withdrawal_nullifier(withdrawal, nullifier);
    if (status != LXP_OK) return status;
    if (lx_asset_nullifier_seen(store, nullifier))
        return LXP_ERR_WITHDRAWAL_ALREADY_SETTLED;
    if (store->count == LX_DEPOSIT_NULLIFIER_CAPACITY)
        return LXP_ERR_ARENA_EXHAUSTED;
    if (transfer->from->kind != LX_ACCOUNT_AGENT_MAIN ||
        transfer->to->kind != LX_ACCOUNT_SYSTEM_PAXEER_WITHDRAWALS ||
        memcmp(withdrawal->account_id, transfer->from->id, 32U) != 0 ||
        memcmp(withdrawal->asset_id, transfer->asset->asset_id, 32U) != 0 ||
        lxp_u128_cmp(withdrawal->amount, transfer->amount) != 0)
        return LXP_ERR_GRANT_SCOPE_VIOLATION;
    (void)memset(&set, 0, sizeof(set));
    set.leg_count = 1U;
    set.legs[0].from = transfer->from;
    set.legs[0].to = transfer->to;
    (void)memcpy(set.legs[0].asset_id, withdrawal->asset_id, 32U);
    set.legs[0].amount = withdrawal->amount;
    set.legs[0].reason = LXP_REASON_WITHDRAWAL;
    set.context = transfer->context;
    status = lxp_ctx_emit_transfer_set(ctx, &set, receipt);
    if (status != LXP_OK) return status;
    (void)memcpy(store->records[store->count].nullifier, nullifier, 32U);
    store->records[store->count].request = *withdrawal;
    store->records[store->count].settled = false;
    ++store->count;
    return LXP_OK;
}

lxp_result lx_asset_withdraw_settle(lxp_module_ctx *ctx,
                                    lx_account *withdrawals,
                                    lx_account *reserve,
                                    const lx_asset_record *asset,
                                    const lx_finalized_checkpoint *checkpoint,
                                    const uint8_t nullifier[32],
                                    lx_withdrawal_store *store,
                                    lxp_transfer_context context,
                                    lxp_receipt *receipt)
{
    lxp_transfer_set set;
    size_t i;
    lxp_result status;
    if (ctx == NULL || withdrawals == NULL || reserve == NULL || asset == NULL ||
        checkpoint == NULL || nullifier == NULL || store == NULL ||
        !checkpoint->finalized) return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
    for (i = 0U; i < store->count; ++i)
        if (memcmp(store->records[i].nullifier, nullifier, 32U) == 0) break;
    if (i == store->count || store->records[i].settled)
        return LXP_ERR_WITHDRAWAL_ALREADY_SETTLED;
    if (memcmp(store->records[i].request.checkpoint_id,
               checkpoint->checkpoint_id, 32U) != 0 ||
        withdrawals->kind != LX_ACCOUNT_SYSTEM_PAXEER_WITHDRAWALS ||
        reserve->kind != LX_ACCOUNT_SYSTEM_PAXEER_RESERVE)
        return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
    (void)memset(&set, 0, sizeof(set));
    set.leg_count = 1U;
    set.legs[0].from = withdrawals;
    set.legs[0].to = reserve;
    (void)memcpy(set.legs[0].asset_id, asset->asset_id, 32U);
    set.legs[0].amount = store->records[i].request.amount;
    set.legs[0].reason = LXP_REASON_WITHDRAWAL;
    set.context = context;
    set.context.protocol_system_capability = true;
    status = lxp_ctx_emit_transfer_set(ctx, &set, receipt);
    if (status == LXP_OK) store->records[i].settled = true;
    return status;
}
