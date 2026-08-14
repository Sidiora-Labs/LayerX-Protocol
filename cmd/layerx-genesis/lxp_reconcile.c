#include "layerx/lxp_genesis.h"

#include "layerx/lxp_crypto.h"

#include <string.h>

static lxp_result attestation_message(
    const lxp_custody_attestation *attestation,
    uint8_t *message, size_t capacity, size_t *length)
{
    size_t cursor = 0U;
    size_t i;
    if (attestation == NULL || message == NULL || length == NULL ||
        attestation->network_id == 0U || attestation->asset_count == 0U ||
        attestation->asset_count > LXP_IMPORT_MAX_ASSET_TOTALS ||
        capacity < 72U + attestation->asset_count * 48U)
        return LXP_ERR_NON_CANONICAL;
    message[cursor++] = (uint8_t)(attestation->network_id >> 24U);
    message[cursor++] = (uint8_t)(attestation->network_id >> 16U);
    message[cursor++] = (uint8_t)(attestation->network_id >> 8U);
    message[cursor++] = (uint8_t)attestation->network_id;
    (void)memcpy(message + cursor, attestation->checkpoint_id, 32U);
    cursor += 32U;
    (void)memcpy(message + cursor, attestation->custody_state_root, 32U);
    cursor += 32U;
    message[cursor++] = (uint8_t)(attestation->asset_count >> 24U);
    message[cursor++] = (uint8_t)(attestation->asset_count >> 16U);
    message[cursor++] = (uint8_t)(attestation->asset_count >> 8U);
    message[cursor++] = (uint8_t)attestation->asset_count;
    for (i = 0U; i < attestation->asset_count; ++i) {
        if (lxp_ct_is_zero(attestation->assets[i].asset_id, 32U) ||
            (i != 0U && memcmp(attestation->assets[i - 1U].asset_id,
                               attestation->assets[i].asset_id, 32U) >= 0))
            return LXP_ERR_UNSORTED_SEQUENCE;
        (void)memcpy(message + cursor,
                     attestation->assets[i].asset_id, 32U);
        cursor += 32U;
        if (lxp_u128_to_be(
                attestation->assets[i].amount,
                message + cursor) != LXP_OK)
            return LXP_ERR_OVERFLOW;
        cursor += 16U;
    }
    *length = cursor;
    return LXP_OK;
}

lxp_result lxp_custody_attestation_verify(
    const lxp_custody_attestation *attestation,
    const lxp_genesis_registration *finalised_state,
    const uint8_t expected_paxeer_public_key[32])
{
    uint8_t message[72U + LXP_IMPORT_MAX_ASSET_TOTALS * 48U];
    size_t length = 0U;
    lxp_result status;
    if (attestation == NULL || finalised_state == NULL ||
        expected_paxeer_public_key == NULL || !finalised_state->finalised ||
        finalised_state->network_id != attestation->network_id ||
        lxp_ct_memcmp(finalised_state->checkpoint_id,
                      attestation->checkpoint_id, 32U) != 0 ||
        lxp_ct_memcmp(finalised_state->state_root,
                      attestation->custody_state_root, 32U) != 0 ||
        lxp_ct_memcmp(expected_paxeer_public_key,
                      attestation->paxeer_public_key, 32U) != 0)
        return LXP_ERR_ROOT_MISMATCH;
    status = attestation_message(
        attestation, message, sizeof(message), &length);
    if (status == LXP_OK)
        status = lxp_ed25519_verify_raw(
            expected_paxeer_public_key, attestation->signature,
            message, length);
    return status;
}

lxp_result lxp_genesis_reject(
    lxp_genesis_manifest *accepted_manifest,
    lxp_genesis_reconcile_report *report,
    const uint8_t asset_id[32], lxp_u128 attested,
    lxp_u128 computed)
{
    if (accepted_manifest == NULL || report == NULL || asset_id == NULL)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(accepted_manifest, 0, sizeof(*accepted_manifest));
    (void)memset(report, 0, sizeof(*report));
    (void)memcpy(report->mismatch_asset_id, asset_id, 32U);
    report->attested_amount = attested;
    report->computed_amount = computed;
    report->computed_exceeds_attested = lxp_u128_cmp(computed, attested) > 0;
    if (report->computed_exceeds_attested) {
        if (lxp_u128_sub(computed, attested, &report->difference) != LXP_OK)
            return LXP_FATAL_SUPPLY_MISMATCH;
    } else if (lxp_u128_sub(
                   attested, computed, &report->difference) != LXP_OK) {
        return LXP_FATAL_SUPPLY_MISMATCH;
    }
    return LXP_FATAL_SUPPLY_MISMATCH;
}

static const lxp_custody_attested_asset *attested_for(
    const lxp_custody_attestation *attestation,
    const uint8_t asset_id[32])
{
    size_t i;
    for (i = 0U; i < attestation->asset_count; ++i)
        if (memcmp(attestation->assets[i].asset_id, asset_id, 32U) == 0)
            return &attestation->assets[i];
    return NULL;
}

lxp_result lxp_genesis_reconcile(
    const lxp_genesis_manifest *candidate,
    const lxp_custody_attestation *attestation,
    const lxp_genesis_registration *finalised_state,
    const uint8_t expected_paxeer_public_key[32],
    lxp_genesis_manifest *accepted_manifest,
    lxp_genesis_reconcile_report *report)
{
    lxp_import_asset_total totals[LXP_IMPORT_MAX_ASSET_TOTALS];
    size_t total_count = 0U;
    size_t i;
    lxp_result status;
    if (candidate == NULL || accepted_manifest == NULL || report == NULL) {
        return LXP_ERR_NON_CANONICAL;
    }
    (void)memset(accepted_manifest, 0, sizeof(*accepted_manifest));
    (void)memset(report, 0, sizeof(*report));
    status = lxp_custody_attestation_verify(
        attestation, finalised_state, expected_paxeer_public_key);
    if (status != LXP_OK) return status;
    (void)memset(totals, 0, sizeof(totals));
    for (i = 0U; i < candidate->account_count; ++i) {
        size_t j;
        for (j = 0U; j < total_count; ++j)
            if (memcmp(totals[j].asset_id,
                       candidate->accounts[i].asset_id, 32U) == 0)
                break;
        if (j == total_count) {
            if (total_count == LXP_IMPORT_MAX_ASSET_TOTALS)
                return LXP_ERR_LENGTH_LIMIT;
            (void)memcpy(totals[j].asset_id,
                         candidate->accounts[i].asset_id, 32U);
            ++total_count;
        }
        status = lxp_u128_add(
            totals[j].amount, candidate->accounts[i].balance,
            &totals[j].amount);
        if (status != LXP_OK) return LXP_FATAL_SUPPLY_MISMATCH;
    }
    for (i = 0U; i < total_count; ++i) {
        const lxp_custody_attested_asset *asset = attested_for(
            attestation, totals[i].asset_id);
        if (asset == NULL)
            return lxp_genesis_reject(
                accepted_manifest, report, totals[i].asset_id,
                (lxp_u128){0U, 0U}, totals[i].amount);
        if (lxp_u128_cmp(asset->amount, totals[i].amount) != 0)
            return lxp_genesis_reject(
                accepted_manifest, report, totals[i].asset_id,
                asset->amount, totals[i].amount);
    }
    for (i = 0U; i < attestation->asset_count; ++i) {
        size_t j;
        for (j = 0U; j < total_count; ++j)
            if (memcmp(attestation->assets[i].asset_id,
                       totals[j].asset_id, 32U) == 0) break;
        if (j == total_count)
            return lxp_genesis_reject(
                accepted_manifest, report,
                attestation->assets[i].asset_id,
                attestation->assets[i].amount,
                (lxp_u128){0U, 0U});
    }
    *accepted_manifest = *candidate;
    report->matched = true;
    return LXP_OK;
}
