#include "layerx/lxp_genesis.h"

#include <openssl/evp.h>
#include <string.h>

static int public_key_for(
    const uint8_t private_key[32], uint8_t public_key[32])
{
    EVP_PKEY *key = EVP_PKEY_new_raw_private_key(
        EVP_PKEY_ED25519, NULL, private_key, 32U);
    size_t length = 32U;
    int ok = key != NULL && EVP_PKEY_get_raw_public_key(
        key, public_key, &length) == 1 && length == 32U;
    EVP_PKEY_free(key);
    return ok ? 0 : 1;
}

static int sign_raw(
    const uint8_t private_key[32], const uint8_t *message,
    size_t message_length, uint8_t signature[64])
{
    EVP_PKEY *key = EVP_PKEY_new_raw_private_key(
        EVP_PKEY_ED25519, NULL, private_key, 32U);
    EVP_MD_CTX *context = EVP_MD_CTX_new();
    size_t signature_length = 64U;
    int ok = key != NULL && context != NULL &&
        EVP_DigestSignInit(context, NULL, NULL, NULL, key) == 1 &&
        EVP_DigestSign(context, signature, &signature_length,
                       message, message_length) == 1 &&
        signature_length == 64U;
    EVP_MD_CTX_free(context);
    EVP_PKEY_free(key);
    return ok ? 0 : 1;
}

static int sign_attestation(
    const uint8_t private_key[32], lxp_custody_attestation *attestation)
{
    uint8_t message[120];
    size_t cursor = 0U;
    size_t i;
    message[cursor++] = (uint8_t)(attestation->network_id >> 24U);
    message[cursor++] = (uint8_t)(attestation->network_id >> 16U);
    message[cursor++] = (uint8_t)(attestation->network_id >> 8U);
    message[cursor++] = (uint8_t)attestation->network_id;
    (void)memcpy(message + cursor, attestation->checkpoint_id, 32U);
    cursor += 32U;
    (void)memcpy(message + cursor, attestation->custody_state_root, 32U);
    cursor += 32U;
    message[cursor++] = 0U;
    message[cursor++] = 0U;
    message[cursor++] = 0U;
    message[cursor++] = 1U;
    (void)memcpy(message + cursor, attestation->assets[0].asset_id, 32U);
    cursor += 32U;
    if (lxp_u128_to_be(attestation->assets[0].amount,
                       message + cursor) != LXP_OK)
        return 1;
    cursor += 16U;
    for (i = cursor; i < sizeof(message); ++i) message[i] = 0U;
    return sign_raw(
        private_key, message, cursor, attestation->signature);
}

int main(void)
{
    static const uint8_t paxeer_private_key[32] = {9U};
    static lxp_genesis_manifest candidate;
    static lxp_genesis_manifest accepted;
    lxp_custody_attestation attestation;
    lxp_genesis_registration registration;
    lxp_genesis_reconcile_report report;

    (void)memset(&candidate, 0, sizeof(candidate));
    candidate.account_count = 2U;
    candidate.accounts[0].asset_id[0] = 1U;
    candidate.accounts[0].account_id[0] = 1U;
    candidate.accounts[0].balance = (lxp_u128){0U, 60U};
    candidate.accounts[1].asset_id[0] = 1U;
    candidate.accounts[1].account_id[0] = 2U;
    candidate.accounts[1].balance = (lxp_u128){0U, 39U};
    candidate.accounts[1].locked = true;
    candidate.accounts[1].subaccount_kind =
        (uint16_t)LXP_IMPORT_PERPS_POSITIONS;
    candidate.accounts[1].parent_account_id[0] = 1U;
    (void)memset(&attestation, 0, sizeof(attestation));
    attestation.network_id = 42U;
    attestation.checkpoint_id[0] = 2U;
    attestation.custody_state_root[0] = 3U;
    attestation.asset_count = 1U;
    attestation.assets[0].asset_id[0] = 1U;
    attestation.assets[0].amount = (lxp_u128){0U, 100U};
    if (public_key_for(
            paxeer_private_key, attestation.paxeer_public_key) != 0 ||
        sign_attestation(paxeer_private_key, &attestation) != 0)
        return 1;
    (void)memset(&registration, 0, sizeof(registration));
    registration.network_id = 42U;
    registration.finalised = true;
    (void)memcpy(registration.checkpoint_id,
                 attestation.checkpoint_id, 32U);
    (void)memcpy(registration.state_root,
                 attestation.custody_state_root, 32U);
    if (lxp_genesis_reconcile(
            &candidate, &attestation, &registration,
            attestation.paxeer_public_key, &accepted,
            &report) != LXP_FATAL_SUPPLY_MISMATCH ||
        report.matched || report.attested_amount.lo != 100U ||
        report.computed_amount.lo != 99U || report.difference.lo != 1U ||
        report.computed_exceeds_attested || accepted.account_count != 0U)
        return 1;
    candidate.accounts[1].balance.lo = 40U;
    if (lxp_genesis_reconcile(
            &candidate, &attestation, &registration,
            attestation.paxeer_public_key, &accepted,
            &report) != LXP_OK || !report.matched ||
        accepted.account_count != 2U)
        return 1;
    attestation.signature[0] ^= 1U;
    return lxp_genesis_reconcile(
        &candidate, &attestation, &registration,
        attestation.paxeer_public_key, &accepted,
        &report) == LXP_ERR_BAD_SIGNATURE && accepted.account_count == 0U ?
        0 : 1;
}
