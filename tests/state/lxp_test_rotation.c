#define OPENSSL_API_COMPAT 0x10100000L

#include "layerx/lxp_identity.h"

#include "layerx/lxp_crypto.h"

#include <openssl/bn.h>
#include <openssl/ec.h>
#include <openssl/ecdsa.h>
#include <openssl/obj_mac.h>
#include <stdint.h>
#include <string.h>

static int compact_signature(const ECDSA_SIG *signature, uint8_t out[64])
{
    const BIGNUM *r;
    const BIGNUM *s;
    ECDSA_SIG_get0(signature, &r, &s);
    return BN_bn2binpad(r, out, 32) == 32 &&
           BN_bn2binpad(s, out + 32U, 32) == 32 ? 0 : 1;
}

static int sign_binding(const uint8_t digest[32], uint8_t signature[64],
                        uint8_t *recovery_id)
{
    EC_KEY *key = EC_KEY_new_by_curve_name(NID_secp256k1);
    BIGNUM *private_value = BN_new();
    const EC_GROUP *group;
    EC_POINT *public_point;
    ECDSA_SIG *signed_digest;
    uint8_t expected[20];
    uint8_t candidate[20];
    uint8_t uncompressed[65];
    size_t public_length;
    uint8_t recovery;
    if (key == NULL || private_value == NULL ||
        BN_set_word(private_value, 1U) != 1 ||
        EC_KEY_set_private_key(key, private_value) != 1) return 0;
    group = EC_KEY_get0_group(key);
    public_point = EC_POINT_new(group);
    if (public_point == NULL ||
        EC_POINT_mul(group, public_point, private_value, NULL, NULL, NULL) != 1 ||
        EC_KEY_set_public_key(key, public_point) != 1) return 0;
    public_length = EC_POINT_point2oct(group, public_point,
        POINT_CONVERSION_UNCOMPRESSED, uncompressed, sizeof(uncompressed), NULL);
    if (public_length != 65U) return 0;
    /* Recovery itself derives the address; find the recovery id that succeeds. */
    signed_digest = ECDSA_do_sign(digest, 32, key);
    if (signed_digest == NULL || compact_signature(signed_digest, signature) != 0)
        return 0;
    if (!lxp_secp256k1_sig_is_low_s(signature)) {
        const BIGNUM *r;
        const BIGNUM *s;
        BIGNUM *order = BN_new();
        BIGNUM *low = BN_new();
        ECDSA_SIG_get0(signed_digest, &r, &s);
        if (order == NULL || low == NULL ||
            EC_GROUP_get_order(group, order, NULL) != 1 ||
            BN_sub(low, order, s) != 1 ||
            BN_bn2binpad(low, signature + 32U, 32) != 32) return 0;
        BN_free(low);
        BN_free(order);
    }
    if (lxp_secp256k1_address(uncompressed, public_length, expected) != LXP_OK)
        return 0;
    for (recovery = 0U; recovery < 4U; ++recovery) {
        if (lxp_secp256k1_recover_address(signature, recovery, digest,
                                          candidate) == LXP_OK &&
            memcmp(candidate, expected, sizeof(expected)) == 0) {
            *recovery_id = recovery;
            ECDSA_SIG_free(signed_digest);
            EC_POINT_free(public_point);
            BN_free(private_value);
            EC_KEY_free(key);
            return 1;
        }
    }
    return 0;
}

int main(void)
{
    lxp_identity identity;
    lxp_identity lapsed;
    uint8_t old_key[32] = { 1U };
    uint8_t new_key[32] = { 2U };
    uint8_t recovered_key[32] = { 3U };
    uint8_t signature[64];
    uint8_t digest[32];
    uint8_t recovery_id;
    (void)memset(&identity, 0, sizeof(identity));
    identity.status = LXP_IDENTITY_ACTIVE;
    identity.did_id[0] = 9U;
    (void)memcpy(identity.primary_key, old_key, 32U);
    if (lxp_identity_rotate_announce(&identity, new_key, 100U, 10U, 20U) !=
            LXP_OK ||
        lxp_identity_rotate_announce(&identity, recovered_key, 101U, 10U, 21U) !=
            LXP_ERR_AUTH_SCOPE ||
        !lxp_identity_key_valid(&identity, old_key, 105U, 19U) ||
        !lxp_identity_key_valid(&identity, new_key, 105U, 19U) ||
        lxp_identity_rotate_commit(&identity, 109U) != LXP_ERR_NOT_YET_VALID ||
        lxp_identity_rotate_commit(&identity, 110U) != LXP_OK ||
        !lxp_identity_key_valid(&identity, old_key, 110U, 19U) ||
        lxp_identity_key_valid(&identity, old_key, 110U, 20U) ||
        !lxp_identity_key_valid(&identity, new_key, 110U, 20U)) return 1;
    lapsed = identity;
    lapsed.has_pending_key = false;
    if (lxp_identity_rotate_announce(&lapsed, old_key, 300U, 10U, 40U) !=
            LXP_OK || lxp_identity_rotate_commit(&lapsed, 321U) != LXP_OK ||
        lapsed.has_pending_key || memcmp(lapsed.primary_key, new_key, 32U) != 0)
        return 1;
    identity.recovery_root[0] = 1U;
    identity.recovery_threshold = 2U;
    if (lxp_identity_recover_begin(&identity, recovered_key, 1U, 200U, 10U) !=
            LXP_ERR_AUTH_SCOPE ||
        lxp_identity_recover_begin(&identity, recovered_key, 2U, 200U, 10U) !=
            LXP_OK ||
        lxp_identity_recover_commit(&identity, 209U) != LXP_ERR_NOT_YET_VALID ||
        lxp_identity_recover_commit(&identity, 210U) != LXP_OK ||
        memcmp(identity.primary_key, recovered_key, 32U) != 0) return 1;
    if (lxp_identity_evm_binding_digest(&identity, 42U, digest) != LXP_OK ||
        !sign_binding(digest, signature, &recovery_id) ||
        lxp_identity_bind_evm_payout(&identity, 42U, signature, recovery_id) !=
            LXP_OK || !identity.has_evm_payout_binding) return 1;
    if (lxp_identity_retire(&identity, false, false) != LXP_ERR_ACCOUNT_NOT_EMPTY ||
        lxp_identity_retire(&identity, true, true) != LXP_ERR_ACCOUNT_NOT_EMPTY ||
        lxp_identity_retire(&identity, true, false) != LXP_OK ||
        identity.status != LXP_IDENTITY_RETIRED) return 1;
    return 0;
}
