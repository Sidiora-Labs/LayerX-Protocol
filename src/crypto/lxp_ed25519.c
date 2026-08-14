#include "layerx/lxp_crypto.h"

#include <openssl/evp.h>
#include <string.h>

static int little_endian_less(const uint8_t *left, const uint8_t *right,
                              size_t length)
{
    size_t i = length;
    while (i-- != 0U) {
        if (left[i] < right[i]) return 1;
        if (left[i] > right[i]) return 0;
    }
    return 0;
}

static int scalar_is_canonical(const uint8_t scalar[32])
{
    static const uint8_t order[32] = {
        0xedU,0xd3U,0xf5U,0x5cU,0x1aU,0x63U,0x12U,0x58U,
        0xd6U,0x9cU,0xf7U,0xa2U,0xdeU,0xf9U,0xdeU,0x14U,
        0U,0U,0U,0U,0U,0U,0U,0U,0U,0U,0U,0U,0U,0U,0U,0x10U
    };
    return little_endian_less(scalar, order, sizeof(order));
}

bool lxp_ed25519_pubkey_is_canonical(const uint8_t public_key[32])
{
    static const uint8_t field_prime[32] = {
        0xedU,0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,
        0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,
        0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,
        0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,0xffU,0x7fU
    };
    uint8_t y[32];
    uint8_t aggregate = 0U;
    size_t i;
    if (public_key == NULL) return false;
    (void)memcpy(y, public_key, sizeof(y));
    y[31] &= 0x7fU;
    for (i = 0U; i < sizeof(y); ++i) aggregate |= y[i];
    if (aggregate == 0U || (y[0] == 1U && lxp_ct_is_zero(y + 1U, 31U)))
        return false;
    return little_endian_less(y, field_prime, sizeof(y)) != 0;
}

lxp_result lxp_ed25519_verify_raw(const uint8_t public_key[32],
                                  const uint8_t signature[64],
                                  const void *message, size_t message_length)
{
    EVP_PKEY *key;
    EVP_MD_CTX *context;
    int verified;
    if (public_key == NULL || signature == NULL ||
        (message == NULL && message_length != 0U) ||
        !lxp_ed25519_pubkey_is_canonical(public_key) ||
        !scalar_is_canonical(signature + 32U)) return LXP_ERR_BAD_SIGNATURE;
    key = EVP_PKEY_new_raw_public_key(EVP_PKEY_ED25519, NULL, public_key, 32U);
    if (key == NULL) return LXP_ERR_BAD_SIGNATURE;
    context = EVP_MD_CTX_new();
    if (context == NULL) { EVP_PKEY_free(key); return LXP_ERR_BAD_SIGNATURE; }
    verified = EVP_DigestVerifyInit(context, NULL, NULL, NULL, key) == 1 &&
               EVP_DigestVerify(context, signature, 64U,
                                (const uint8_t *)message, message_length) == 1;
    EVP_MD_CTX_free(context);
    EVP_PKEY_free(key);
    return verified ? LXP_OK : LXP_ERR_BAD_SIGNATURE;
}

lxp_result lxp_ed25519_verify(const uint8_t public_key[32],
                              const uint8_t signature[64],
                              lxp_domain_tag_id domain, const void *message,
                              size_t message_length)
{
    uint8_t preimage[32];
    lxp_result result = lxp_hash_domain(domain, message, message_length, preimage);
    if (result == LXP_OK)
        result = lxp_ed25519_verify_raw(public_key, signature, preimage,
                                        sizeof(preimage));
    lxp_secure_zero(preimage, sizeof(preimage));
    return result;
}

lxp_result lxp_ed25519_verify_batch(const lxp_ed25519_verify_item *items,
                                    size_t count, bool *valid)
{
    size_t i;
    bool all_valid = true;
    if ((items == NULL || valid == NULL) && count != 0U)
        return LXP_ERR_NON_CANONICAL;
    for (i = 0U; i < count; ++i) {
        valid[i] = lxp_ed25519_verify(items[i].public_key, items[i].signature,
                                      items[i].domain, items[i].message,
                                      items[i].message_length) == LXP_OK;
        all_valid = all_valid && valid[i];
    }
    return all_valid ? LXP_OK : LXP_ERR_BAD_SIGNATURE;
}
