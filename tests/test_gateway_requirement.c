#include "layerx/lxp_gateway.h"
#include "layerx/lxp_hash.h"

#include <openssl/evp.h>
#include <stdio.h>
#include <string.h>

static void hex_encode(const uint8_t *bytes, size_t length, char *hex)
{
    static const char alphabet[] = "0123456789abcdef";
    size_t i;
    for (i = 0U; i < length; ++i) {
        hex[i * 2U] = alphabet[bytes[i] >> 4U];
        hex[i * 2U + 1U] = alphabet[bytes[i] & 0x0fU];
    }
    hex[length * 2U] = '\0';
}

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

static int sign_requirement(
    const uint8_t private_key[32], lxp_payment_requirement *requirement)
{
    uint8_t preimage[LXP_PAYMENT_REQUIREMENT_PREIMAGE_SIZE];
    size_t preimage_length = 0U;
    size_t signature_length = 64U;
    EVP_PKEY *key = EVP_PKEY_new_raw_private_key(
        EVP_PKEY_ED25519, NULL, private_key, 32U);
    EVP_MD_CTX *context = EVP_MD_CTX_new();
    int ok = key != NULL && context != NULL &&
        lxp_payment_requirement_encode(
            requirement, false, preimage, sizeof(preimage),
            &preimage_length) == LXP_OK &&
        preimage_length == sizeof(preimage) &&
        EVP_DigestSignInit(context, NULL, NULL, NULL, key) == 1 &&
        EVP_DigestSign(context, requirement->service_signature,
                       &signature_length, preimage, sizeof(preimage)) == 1 &&
        signature_length == 64U;
    EVP_MD_CTX_free(context);
    EVP_PKEY_free(key);
    return ok ? 0 : 1;
}

int main(void)
{
    uint8_t private_key[32] = {7U};
    uint8_t public_key[32];
    lxp_payment_requirement issued;
    lxp_payment_requirement translated;
    uint8_t direct[LXP_PAYMENT_REQUIREMENT_ENCODED_SIZE];
    uint8_t canonical[LXP_PAYMENT_REQUIREMENT_ENCODED_SIZE];
    uint8_t direct_root[32];
    uint8_t gateway_root[32];
    char recipient_hex[65];
    char asset_hex[65];
    char invoice_hex[65];
    char purpose_hex[65];
    char signature_hex[129];
    char json[768];
    size_t direct_length = 0U;
    size_t canonical_length = 0U;
    size_t json_length;
    size_t i;

    (void)memset(&issued, 0, sizeof(issued));
    issued.network_id = 42U;
    issued.recipient[0] = 0xd4U;
    issued.asset[0] = 0xa1U;
    issued.amount = (lxp_u128){0U, 700000000U};
    issued.invoice_id[0] = 0xb2U;
    issued.purpose_hash[0] = 0xc3U;
    issued.expiry = 2000000000U;
    issued.acceptable_conditions = 5U;
    if (public_key_for(private_key, public_key) != 0 ||
        sign_requirement(private_key, &issued) != 0 ||
        lxp_payment_requirement_verify(
            &issued, 42U, public_key) != LXP_OK ||
        lxp_payment_requirement_encode(
            &issued, true, direct, sizeof(direct), &direct_length) != LXP_OK ||
        direct_length != sizeof(direct)) return 1;
    hex_encode(issued.recipient, 32U, recipient_hex);
    hex_encode(issued.asset, 32U, asset_hex);
    hex_encode(issued.invoice_id, 32U, invoice_hex);
    hex_encode(issued.purpose_hash, 32U, purpose_hex);
    hex_encode(issued.service_signature, 64U, signature_hex);
    if (snprintf(
            json, sizeof(json),
            "{\"network_id\":42,\"recipient\":\"%s\",\"asset\":\"%s\","
            "\"amount\":\"700000000\",\"invoice_id\":\"%s\","
            "\"purpose_hash\":\"%s\",\"expiry\":2000000000,"
            "\"acceptable_conditions\":5,\"service_signature\":\"%s\"}",
            recipient_hex, asset_hex, invoice_hex, purpose_hex,
            signature_hex) < 0) return 1;
    json_length = strlen(json);
    if (lxp_gateway_translate(
            (const uint8_t *)json, json_length, &translated,
            canonical, &canonical_length) != LXP_OK ||
        canonical_length != sizeof(canonical) ||
        memcmp(canonical, direct, sizeof(direct)) != 0 ||
        lxp_payment_requirement_verify(
            &translated, 42U, public_key) != LXP_OK ||
        lxp_hash_sha256(direct, sizeof(direct), direct_root) != LXP_OK ||
        lxp_hash_sha256(canonical, sizeof(canonical), gateway_root) != LXP_OK ||
        memcmp(direct_root, gateway_root, 32U) != 0)
        return 1;

    json[1] = ' ';
    if (lxp_gateway_translate(
            (const uint8_t *)json, json_length, &translated,
            canonical, &canonical_length) != LXP_ERR_NON_CANONICAL)
        return 1;
    json[1] = '"';
    for (i = 0U; i < json_length; ++i) {
        lxp_result status = lxp_gateway_translate(
            (const uint8_t *)json, i, &translated,
            canonical, &canonical_length);
        if (status == LXP_OK) return 1;
    }
    issued.service_signature[0] ^= 1U;
    if (lxp_payment_requirement_verify(
            &issued, 42U, public_key) != LXP_ERR_BAD_SIGNATURE)
        return 1;
    issued.service_signature[0] ^= 1U;
    if (lxp_payment_requirement_verify(
            &issued, 7U, public_key) != LXP_ERR_NON_CANONICAL)
        return 1;
    issued.invoice_id[0] = 0U;
    if (lxp_payment_requirement_verify(
            &issued, 42U, public_key) != LXP_ERR_NON_CANONICAL)
        return 1;
    return 0;
}
