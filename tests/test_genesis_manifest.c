#include "layerx/lxp_genesis.h"

#include "layerx/lxp_hash.h"

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

static lxp_result checkpoint_id(
    uint32_t network_id, const uint8_t state_root[32], uint8_t output[32])
{
    uint8_t preimage[36];
    preimage[0] = (uint8_t)(network_id >> 24U);
    preimage[1] = (uint8_t)(network_id >> 16U);
    preimage[2] = (uint8_t)(network_id >> 8U);
    preimage[3] = (uint8_t)network_id;
    (void)memcpy(preimage + 4U, state_root, 32U);
    return lxp_hash_domain(
        LXP_DOMAIN_CHECKPOINT_CERTIFICATE,
        preimage, sizeof(preimage), output);
}

int main(void)
{
    static const uint8_t signer_private_key[32] = {7U};
    static uint8_t arena_bytes[1048576U];
    static uint8_t encoded_copy[LXP_GENESIS_MAX_ENCODED_BYTES];
    static lxp_genesis_manifest manifest;
    static lxp_genesis_manifest decoded;
    lxp_arena arena;
    lxp_byte_span preimage;
    lxp_byte_span encoded;
    lxp_byte_span reencoded;
    lxp_genesis_registration registration;
    bool enabled = false;
    size_t encoded_length;

    if (lxp_arena_init(&arena, arena_bytes, sizeof(arena_bytes)) != LXP_OK)
        return 1;
    (void)memset(&manifest, 0, sizeof(manifest));
    manifest.protocol_version = LXP_PROTOCOL_VERSION;
    manifest.network_id = 42U;
    manifest.genesis_timestamp_ms = 1700000000000U;
    manifest.parameter_count = 2U;
    manifest.parameters[0].module_id = 1U;
    manifest.parameters[0].key[0] = 1U;
    manifest.parameters[0].value[0] = 11U;
    manifest.parameters[1].module_id = 2U;
    manifest.parameters[1].key[0] = 1U;
    manifest.parameters[1].value[0] = 12U;
    manifest.guarantor_count = 2U;
    manifest.guarantors[0].guarantor_id[0] = 1U;
    manifest.guarantors[0].public_key[0] = 2U;
    manifest.guarantors[0].bond = (lxp_u128){0U, 100U};
    manifest.guarantors[1].guarantor_id[0] = 2U;
    manifest.guarantors[1].public_key[0] = 3U;
    manifest.guarantors[1].bond = (lxp_u128){0U, 100U};
    manifest.account_count = 2U;
    manifest.accounts[0].asset_id[0] = 1U;
    manifest.accounts[0].account_id[0] = 1U;
    manifest.accounts[0].balance = (lxp_u128){0U, 700U};
    manifest.accounts[1].asset_id[0] = 1U;
    manifest.accounts[1].account_id[0] = 2U;
    manifest.accounts[1].balance = (lxp_u128){0U, 300U};
    manifest.accounts[1].locked = true;
    manifest.accounts[1].subaccount_kind = 4U;
    manifest.accounts[1].parent_account_id[0] = 1U;
    manifest.module_value_count = 1U;
    manifest.module_values[0].module_id = 1U;
    manifest.module_values[0].key[0] = 1U;
    manifest.module_values[0].value[0] = 9U;
    manifest.module_values[0].value_length = 1U;
    if (public_key_for(
            signer_private_key, manifest.signer_public_key) != 0 ||
        lxp_genesis_state_root(
            &manifest, &arena, manifest.genesis_state_root) != LXP_OK ||
        checkpoint_id(
            manifest.network_id, manifest.genesis_state_root,
            manifest.paxeer_genesis_checkpoint_id) != LXP_OK ||
        lxp_genesis_encode(
            &manifest, false, &arena, &preimage) != LXP_OK ||
        sign_raw(signer_private_key, preimage.bytes, preimage.length,
                 manifest.signature) != 0 ||
        lxp_arena_reset(&arena, 0U) != LXP_OK ||
        lxp_genesis_encode(
            &manifest, true, &arena, &encoded) != LXP_OK ||
        encoded.length > sizeof(encoded_copy))
        return 1;
    encoded_length = encoded.length;
    (void)memcpy(encoded_copy, encoded.bytes, encoded_length);
    if (lxp_genesis_parse(
            encoded_copy, encoded_length,
            LXP_GENESIS_INPUT_MANIFEST, &decoded) != LXP_OK ||
        lxp_genesis_verify_signature(&decoded, &arena) != LXP_OK ||
        decoded.accounts[1].subaccount_kind != 4U ||
        lxp_arena_reset(&arena, 0U) != LXP_OK ||
        lxp_genesis_encode(
            &decoded, true, &arena, &reencoded) != LXP_OK ||
        reencoded.length != encoded_length ||
        memcmp(reencoded.bytes, encoded_copy, encoded_length) != 0 ||
        lxp_genesis_parse(
            encoded_copy, encoded_length,
            LXP_GENESIS_INPUT_DATABASE, &decoded) != LXP_ERR_NON_CANONICAL)
        return 1;
    (void)memset(&registration, 0, sizeof(registration));
    registration.network_id = manifest.network_id;
    (void)memcpy(registration.checkpoint_id,
                 manifest.paxeer_genesis_checkpoint_id, 32U);
    (void)memcpy(registration.state_root,
                 manifest.genesis_state_root, 32U);
    registration.finalised = true;
    if (lxp_genesis_accept(
            &manifest, &registration, false,
            &arena, &enabled) != LXP_ERR_ROOT_MISMATCH || enabled ||
        lxp_genesis_main(
            encoded_copy, encoded_length, &registration, true,
            &arena, &enabled) != LXP_OK || !enabled)
        return 1;
    encoded_copy[encoded_length - 1U] ^= 1U;
    return lxp_genesis_parse(
        encoded_copy, encoded_length, LXP_GENESIS_INPUT_MANIFEST,
        &decoded) == LXP_OK &&
        lxp_genesis_verify_signature(&decoded, &arena) == LXP_OK ? 1 : 0;
}
