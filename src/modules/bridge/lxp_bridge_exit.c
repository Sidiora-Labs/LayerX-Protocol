#include "layerx/lxp_bridge.h"

#include "layerx/lxp_crypto.h"
#include "layerx/lxp_hash.h"

#include <string.h>

static lxp_result exit_balance_leaf(
    const lxp_exit_balance_record *record,
    uint8_t leaf_hash[32])
{
    uint8_t canonical[112];
    uint8_t amount[16];
    size_t cursor = 0U;
    lxp_result status;
    if (record == NULL || leaf_hash == NULL ||
        lxp_ct_is_zero(record->account_id, 32U) ||
        lxp_ct_is_zero(record->asset_id, 32U) ||
        lxp_ct_is_zero(record->payout_recipient, 32U) ||
        lxp_u128_is_zero(record->balance)) return LXP_ERR_NON_CANONICAL;
    status = lxp_u128_to_be(record->balance, amount);
    if (status != LXP_OK) return status;
    (void)memcpy(canonical + cursor, record->account_id, 32U);
    cursor += 32U;
    (void)memcpy(canonical + cursor, record->asset_id, 32U);
    cursor += 32U;
    (void)memcpy(canonical + cursor, amount, sizeof(amount));
    cursor += sizeof(amount);
    (void)memcpy(canonical + cursor, record->payout_recipient, 32U);
    cursor += 32U;
    return lxp_merkle_leaf_hash(canonical, cursor, leaf_hash);
}

static lxp_result exit_withdrawal_id(
    uint32_t network_id,
    const lxp_exit_balance_record *record,
    const uint8_t checkpoint_id[32],
    uint8_t withdrawal_id[32])
{
    static const uint8_t tag[] = "LXP/v1/emergency-withdrawal-id\000";
    uint8_t input[sizeof(tag) - 1U + 100U];
    size_t cursor = 0U;
    if (network_id == 0U || record == NULL || checkpoint_id == NULL ||
        withdrawal_id == NULL) return LXP_ERR_NON_CANONICAL;
    (void)memcpy(input + cursor, tag, sizeof(tag) - 1U);
    cursor += sizeof(tag) - 1U;
    input[cursor++] = (uint8_t)(network_id >> 24U);
    input[cursor++] = (uint8_t)(network_id >> 16U);
    input[cursor++] = (uint8_t)(network_id >> 8U);
    input[cursor++] = (uint8_t)network_id;
    (void)memcpy(input + cursor, record->account_id, 32U);
    cursor += 32U;
    (void)memcpy(input + cursor, record->asset_id, 32U);
    cursor += 32U;
    (void)memcpy(input + cursor, checkpoint_id, 32U);
    cursor += 32U;
    return lxp_hash_sha256(input, cursor, withdrawal_id);
}

lxp_result lxp_exit_eligibility(const lxp_exit_state *state, bool *eligible)
{
    bool liveness_expired;
    if (state == NULL || eligible == NULL || state->liveness_bound_ms == 0U ||
        state->last_finalised_at_ms == 0U ||
        state->now_ms < state->last_finalised_at_ms)
        return LXP_ERR_NON_CANONICAL;
    liveness_expired =
        state->now_ms - state->last_finalised_at_ms >=
            state->liveness_bound_ms;
    *eligible = liveness_expired || state->governance_emergency ||
        state->latest_checkpoint_fraud_accepted;
    return LXP_OK;
}

lxp_result lxp_exit_declare(lxp_exit_state *state)
{
    bool eligible = false;
    lxp_result status = lxp_exit_eligibility(state, &eligible);
    if (status != LXP_OK) return status;
    if (!eligible) return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
    state->declared = true;
    state->discard_after_sequence = state->last_finalised_sequence;
    return LXP_OK;
}

lxp_result lxp_exit_claim_build(
    const lx_finalized_checkpoint *checkpoint,
    const lxp_guarantor_cert *certificate,
    const lxp_exit_balance_record *balance_record,
    const lxp_merkle_proof *balance_proof,
    lxp_arena *arena,
    lxp_exit_claim *claim)
{
    uint8_t certificate_id[32];
    lxp_result status;
    if (checkpoint == NULL || certificate == NULL || balance_record == NULL ||
        balance_proof == NULL || arena == NULL || claim == NULL ||
        !checkpoint->finalized ||
        lxp_ct_memcmp(checkpoint->state_root,
            certificate->checkpoint.header.resulting_state_root, 32U) != 0)
        return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
    status = lxp_checkpoint_certificate_hash(
        &certificate->checkpoint, arena, certificate_id);
    if (status != LXP_OK ||
        lxp_ct_memcmp(certificate_id, checkpoint->checkpoint_id, 32U) != 0)
        return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
    (void)memset(claim, 0, sizeof(*claim));
    claim->checkpoint = checkpoint;
    claim->certificate = certificate;
    claim->balance_record = *balance_record;
    claim->balance_proof = *balance_proof;
    claim->withdrawal.network_id = certificate->checkpoint.header.network_id;
    (void)memcpy(claim->withdrawal.account_id,
                 balance_record->account_id, 32U);
    (void)memcpy(claim->withdrawal.asset_id,
                 balance_record->asset_id, 32U);
    claim->withdrawal.amount = balance_record->balance;
    (void)memcpy(claim->withdrawal.payout_recipient,
                 balance_record->payout_recipient, 32U);
    (void)memcpy(claim->withdrawal.checkpoint_id,
                 checkpoint->checkpoint_id, 32U);
    return exit_withdrawal_id(
        claim->withdrawal.network_id, balance_record,
        checkpoint->checkpoint_id, claim->withdrawal.withdrawal_id);
}

lxp_result lxp_exit_verify_balance_proof(
    const lxp_exit_claim *claim,
    const lxp_guarantor_key_record *guarantor_keys,
    size_t guarantor_key_count,
    lxp_arena *arena)
{
    uint8_t leaf_hash[32];
    uint8_t certificate_id[32];
    size_t valid_signatures = 0U;
    lxp_result status;
    if (claim == NULL || claim->checkpoint == NULL ||
        claim->certificate == NULL || guarantor_keys == NULL ||
        guarantor_key_count == 0U || arena == NULL ||
        !claim->checkpoint->finalized)
        return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
    status = exit_balance_leaf(&claim->balance_record, leaf_hash);
    if (status == LXP_OK)
        status = lxp_merkle_proof_verify(
            leaf_hash, &claim->balance_proof, claim->checkpoint->state_root);
    if (status != LXP_OK) return LXP_ERR_ROOT_MISMATCH;
    status = lxp_checkpoint_certificate_hash(
        &claim->certificate->checkpoint, arena, certificate_id);
    if (status != LXP_OK ||
        lxp_ct_memcmp(certificate_id,
                      claim->checkpoint->checkpoint_id, 32U) != 0)
        return LXP_ERR_DEPOSIT_PROOF_NOT_FINAL;
    status = lxp_guarantor_cert_verify(
        claim->certificate, guarantor_keys, guarantor_key_count,
        arena, &valid_signatures);
    if (status != LXP_OK ||
        valid_signatures < claim->certificate->threshold)
        return LXP_ERR_ATTESTATION_THRESHOLD;
    return LXP_OK;
}
