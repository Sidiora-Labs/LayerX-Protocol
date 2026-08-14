#include "layerx/lxp_crypto.h"

#include <openssl/evp.h>
#include <stdio.h>
#include <string.h>

int lxp_fuzz_signature(const uint8_t public_key[32], const uint8_t signature[64],
                       const void *message, size_t message_length);

static int decode_hex(const char *hex, uint8_t *out, size_t length)
{
    size_t i;
    for (i = 0U; i < length; ++i) {
        unsigned value;
        if (sscanf(hex + i * 2U, "%2x", &value) != 1) return 1;
        out[i] = (uint8_t)value;
    }
    return 0;
}

int main(void)
{
    uint8_t seed[32], public_key[32], signature[64], digest[32];
    size_t public_length = sizeof(public_key), signature_length = sizeof(signature);
    EVP_PKEY *private_key;
    EVP_MD_CTX *context;
    bool valid[2];
    lxp_ed25519_verify_item items[2];
    static const uint8_t message[] = {0U,0U,0U,17U,'L','a','y','e','r','X'};

    if (decode_hex("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60", seed, 32U) != 0)
        return 1;
    private_key = EVP_PKEY_new_raw_private_key(EVP_PKEY_ED25519, NULL, seed, 32U);
    if (private_key == NULL ||
        EVP_PKEY_get_raw_public_key(private_key, public_key, &public_length) != 1 ||
        public_length != 32U ||
        lxp_hash_domain(LXP_DOMAIN_SIGNATURE_PREIMAGE, message, sizeof(message), digest) != LXP_OK)
        return 1;
    context = EVP_MD_CTX_new();
    if (context == NULL || EVP_DigestSignInit(context, NULL, NULL, NULL, private_key) != 1 ||
        EVP_DigestSign(context, signature, &signature_length, digest, sizeof(digest)) != 1 ||
        signature_length != 64U) return 1;
    EVP_MD_CTX_free(context);
    EVP_PKEY_free(private_key);
    if (lxp_ed25519_verify(public_key, signature, LXP_DOMAIN_SIGNATURE_PREIMAGE,
                           message, sizeof(message)) != LXP_OK ||
        lxp_ed25519_verify(public_key, signature, LXP_DOMAIN_ACTIVITY_ID,
                           message, sizeof(message)) != LXP_ERR_BAD_SIGNATURE ||
        lxp_fuzz_signature(public_key, signature, digest, sizeof(digest)) != 0)
        return 1;
    items[0] = (lxp_ed25519_verify_item){public_key, signature,
        LXP_DOMAIN_SIGNATURE_PREIMAGE, message, sizeof(message)};
    items[1] = items[0];
    items[1].domain = LXP_DOMAIN_ACTIVITY_ID;
    if (lxp_ed25519_verify_batch(items, 2U, valid) != LXP_ERR_BAD_SIGNATURE ||
        !valid[0] || valid[1]) return 1;
    (void)memset(signature + 32U, 0xff, 32U);
    return lxp_ed25519_verify(public_key, signature,
        LXP_DOMAIN_SIGNATURE_PREIMAGE, message, sizeof(message)) ==
        LXP_ERR_BAD_SIGNATURE ? 0 : 1;
}
