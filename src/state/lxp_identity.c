#include "layerx/lxp_identity.h"

#include "layerx/lxp_hash.h"
#include "layerx/lxp_crypto.h"
#include "layerx/lxp_protocol.h"

#include <string.h>

static int span_equal(const uint8_t *bytes, size_t length, const char *text)
{
    size_t text_length = strlen(text);
    return length == text_length && memcmp(bytes, text, length) == 0;
}

static int contains_marker(const uint8_t *bytes, size_t length,
                           const char *marker)
{
    size_t marker_length = strlen(marker);
    size_t i;
    if (length <= marker_length) return 0;
    for (i = 0U; i + marker_length < length; ++i)
        if (memcmp(bytes + i, marker, marker_length) == 0 &&
            i != 0U && i + marker_length < length) return 1;
    return 0;
}

static int account_namespace_valid(const uint8_t *name, size_t length)
{
    size_t i;
    if (name == NULL || length == 0U || length > 1024U) return 0;
    for (i = 0U; i < length; ++i)
        if (name[i] < 0x21U || name[i] > 0x7eU) return 0;
    if (span_equal(name, length, "system:insurance") ||
        span_equal(name, length, "system:fees") ||
        span_equal(name, length, "system:paxeer-reserve") ||
        span_equal(name, length, "system:paxeer-withdrawals")) return 1;
    if (length > 17U && memcmp(name, "system:liquidity:", 17U) == 0) return 1;
    if (length <= 6U || memcmp(name, "agent:", 6U) != 0) return 0;
    if (length > 5U && memcmp(name + length - 5U, ":main", 5U) == 0 &&
        length > 11U) return 1;
    return contains_marker(name + 6U, length - 6U, ":budget:") ||
           contains_marker(name + 6U, length - 6U, ":escrow:") ||
           contains_marker(name + 6U, length - 6U, ":margin:");
}

lxp_result lxp_did_id_derive(const uint8_t *did, size_t did_length,
                             uint8_t did_id[32])
{
    uint8_t canonical[2U + LXP_MAX_DID_LENGTH];
    if (did == NULL || did_id == NULL || did_length == 0U ||
        did_length > LXP_MAX_DID_LENGTH) return LXP_ERR_UNKNOWN_DID;
    canonical[0] = (uint8_t)(did_length >> 8U);
    canonical[1] = (uint8_t)did_length;
    (void)memcpy(canonical + 2U, did, did_length);
    return lxp_hash_domain(LXP_DOMAIN_DID_ID, canonical, did_length + 2U, did_id);
}

lxp_result lxp_account_id_derive(const uint8_t *account_name,
                                 size_t account_name_length,
                                 uint8_t account_id[32])
{
    if (account_id == NULL ||
        !account_namespace_valid(account_name, account_name_length))
        return LXP_ERR_UNKNOWN_ACCOUNT_NAMESPACE;
    return lxp_hash_account_id(account_name, account_name_length, account_id);
}

lxp_result lxp_identity_register(lxp_identity_store *store,
                                 const uint8_t *did, size_t did_length,
                                 const uint8_t primary_key[32],
                                 lxp_identity **identity)
{
    uint8_t identifier[32];
    size_t i;
    lxp_result status;
    if (store == NULL || primary_key == NULL || identity == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = lxp_did_id_derive(did, did_length, identifier);
    if (status != LXP_OK) return status;
    for (i = 0U; i < store->count; ++i)
        if (memcmp(store->identities[i].did_id, identifier, 32U) == 0)
            return LXP_ERR_SEQUENCE_REUSED;
    if (store->count == LXP_IDENTITY_STORE_CAPACITY)
        return LXP_ERR_ARENA_EXHAUSTED;
    *identity = &store->identities[store->count++];
    (void)memset(*identity, 0, sizeof(**identity));
    (void)memcpy((*identity)->did_id, identifier, 32U);
    (void)memcpy((*identity)->primary_key, primary_key, 32U);
    (*identity)->status = LXP_IDENTITY_ACTIVE;
    return LXP_OK;
}

lxp_result lxp_identity_resolve(lxp_identity_store *store,
                                const uint8_t *did, size_t did_length,
                                lxp_identity **identity)
{
    uint8_t identifier[32];
    size_t i;
    lxp_result status;
    if (store == NULL || identity == NULL) return LXP_ERR_NON_CANONICAL;
    status = lxp_did_id_derive(did, did_length, identifier);
    if (status != LXP_OK) return status;
    for (i = 0U; i < store->count; ++i) {
        if (memcmp(store->identities[i].did_id, identifier, 32U) == 0) {
            if (store->identities[i].status == LXP_IDENTITY_FROZEN)
                return LXP_ERR_IDENTITY_FROZEN;
            if (store->identities[i].status != LXP_IDENTITY_ACTIVE &&
                store->identities[i].status != LXP_IDENTITY_RECOVERING)
                return LXP_ERR_UNKNOWN_DID;
            *identity = &store->identities[i];
            return LXP_OK;
        }
    }
    return LXP_ERR_UNKNOWN_DID;
}

lxp_result lxp_identity_consume_sequence(lxp_identity *identity,
                                         uint64_t account_sequence)
{
    if (identity == NULL) return LXP_ERR_UNKNOWN_DID;
    if (identity->status == LXP_IDENTITY_FROZEN)
        return LXP_ERR_IDENTITY_FROZEN;
    if (identity->status != LXP_IDENTITY_ACTIVE &&
        identity->status != LXP_IDENTITY_RECOVERING) return LXP_ERR_UNKNOWN_DID;
    if (account_sequence < identity->next_sequence)
        return LXP_ERR_SEQUENCE_REUSED;
    if (account_sequence > identity->next_sequence) return LXP_ERR_SEQUENCE_GAP;
    if (identity->next_sequence == UINT64_MAX) return LXP_ERR_OVERFLOW;
    ++identity->next_sequence;
    return LXP_OK;
}

static lxp_result delayed_window(uint64_t now, uint64_t delay,
                                 uint64_t *effective, uint64_t *lapse)
{
    if (delay == 0U || now > UINT64_MAX - delay) return LXP_ERR_OVERFLOW;
    *effective = now + delay;
    if (*effective > UINT64_MAX - delay) return LXP_ERR_OVERFLOW;
    *lapse = *effective + delay;
    return LXP_OK;
}

lxp_result lxp_identity_rotate_announce(lxp_identity *identity,
                                        const uint8_t pending_key[32],
                                        uint64_t batch_timestamp,
                                        uint64_t challenge_delay,
                                        uint64_t effective_sequence)
{
    uint64_t effective;
    uint64_t lapse;
    lxp_result status;
    if (identity == NULL || pending_key == NULL ||
        lxp_ct_is_zero(pending_key, 32U)) return LXP_ERR_BAD_SIGNATURE;
    if (identity->has_pending_key) return LXP_ERR_AUTH_SCOPE;
    status = delayed_window(batch_timestamp, challenge_delay, &effective, &lapse);
    if (status != LXP_OK) return status;
    (void)memcpy(identity->pending_key, pending_key, 32U);
    identity->has_pending_key = true;
    identity->rotation_announced_at = batch_timestamp;
    identity->rotation_effective_at = effective;
    identity->rotation_lapse_at = lapse;
    identity->rotation_effective_sequence = effective_sequence;
    return LXP_OK;
}

lxp_result lxp_identity_rotate_commit(lxp_identity *identity,
                                      uint64_t batch_timestamp)
{
    if (identity == NULL || !identity->has_pending_key)
        return LXP_ERR_NON_CANONICAL;
    if (batch_timestamp < identity->rotation_effective_at)
        return LXP_ERR_NOT_YET_VALID;
    if (batch_timestamp > identity->rotation_lapse_at) {
        lxp_secure_zero(identity->pending_key, 32U);
        identity->has_pending_key = false;
        return LXP_OK;
    }
    (void)memcpy(identity->superseded_key, identity->primary_key, 32U);
    identity->has_superseded_key = true;
    (void)memcpy(identity->primary_key, identity->pending_key, 32U);
    lxp_secure_zero(identity->pending_key, 32U);
    identity->has_pending_key = false;
    return LXP_OK;
}

bool lxp_identity_key_valid(const lxp_identity *identity,
                            const uint8_t key[32], uint64_t batch_timestamp,
                            uint64_t global_sequence)
{
    if (identity == NULL || key == NULL) return false;
    if (lxp_ct_memcmp(identity->primary_key, key, 32U) == 0) return true;
    if (identity->has_pending_key &&
        batch_timestamp <= identity->rotation_lapse_at &&
        lxp_ct_memcmp(identity->pending_key, key, 32U) == 0) return true;
    return identity->has_superseded_key &&
           global_sequence < identity->rotation_effective_sequence &&
           lxp_ct_memcmp(identity->superseded_key, key, 32U) == 0;
}

lxp_result lxp_identity_recover_begin(lxp_identity *identity,
                                      const uint8_t recovered_key[32],
                                      uint16_t approvals,
                                      uint64_t batch_timestamp,
                                      uint64_t challenge_delay)
{
    lxp_result status;
    if (identity == NULL || recovered_key == NULL ||
        identity->recovery_threshold == 0U ||
        approvals < identity->recovery_threshold ||
        lxp_ct_is_zero(identity->recovery_root, 32U) ||
        lxp_ct_is_zero(recovered_key, 32U)) return LXP_ERR_AUTH_SCOPE;
    if (identity->status == LXP_IDENTITY_RECOVERING)
        return LXP_ERR_AUTH_SCOPE;
    status = delayed_window(batch_timestamp, challenge_delay,
                            &identity->recovery_effective_at,
                            &identity->recovery_lapse_at);
    if (status != LXP_OK) return status;
    (void)memcpy(identity->recovery_pending_key, recovered_key, 32U);
    identity->recovery_approvals = approvals;
    identity->recovery_vetoed = false;
    identity->status = LXP_IDENTITY_RECOVERING;
    return LXP_OK;
}

lxp_result lxp_identity_recover_veto(lxp_identity *identity)
{
    if (identity == NULL || identity->status != LXP_IDENTITY_RECOVERING)
        return LXP_ERR_NON_CANONICAL;
    identity->recovery_vetoed = true;
    identity->status = LXP_IDENTITY_ACTIVE;
    lxp_secure_zero(identity->recovery_pending_key, 32U);
    return LXP_OK;
}

lxp_result lxp_identity_recover_commit(lxp_identity *identity,
                                       uint64_t batch_timestamp)
{
    if (identity == NULL || identity->status != LXP_IDENTITY_RECOVERING ||
        identity->recovery_vetoed) return LXP_ERR_AUTH_REVOKED;
    if (batch_timestamp < identity->recovery_effective_at)
        return LXP_ERR_NOT_YET_VALID;
    if (batch_timestamp > identity->recovery_lapse_at) {
        identity->status = LXP_IDENTITY_ACTIVE;
        lxp_secure_zero(identity->recovery_pending_key, 32U);
        return LXP_OK;
    }
    (void)memcpy(identity->superseded_key, identity->primary_key, 32U);
    identity->has_superseded_key = true;
    (void)memcpy(identity->primary_key, identity->recovery_pending_key, 32U);
    lxp_secure_zero(identity->recovery_pending_key, 32U);
    identity->status = LXP_IDENTITY_ACTIVE;
    return LXP_OK;
}

lxp_result lxp_identity_evm_binding_digest(const lxp_identity *identity,
                                           uint32_t network_id,
                                           uint8_t digest[32])
{
    uint8_t statement[36];
    if (identity == NULL || digest == NULL || network_id == 0U)
        return LXP_ERR_NON_CANONICAL;
    (void)memcpy(statement, identity->did_id, 32U);
    statement[32] = (uint8_t)(network_id >> 24U);
    statement[33] = (uint8_t)(network_id >> 16U);
    statement[34] = (uint8_t)(network_id >> 8U);
    statement[35] = (uint8_t)network_id;
    return lxp_hash_domain(LXP_DOMAIN_EVM_PAYOUT_BINDING, statement,
                           sizeof(statement), digest);
}

lxp_result lxp_identity_bind_evm_payout(lxp_identity *identity,
                                        uint32_t network_id,
                                        const uint8_t signature[64],
                                        uint8_t recovery_id)
{
    uint8_t digest[32];
    uint8_t address[20];
    lxp_result status;
    if (identity == NULL || signature == NULL) return LXP_ERR_BAD_SIGNATURE;
    status = lxp_identity_evm_binding_digest(identity, network_id, digest);
    if (status == LXP_OK)
        status = lxp_secp256k1_recover_address(signature, recovery_id, digest,
                                               address);
    if (status != LXP_OK) return status;
    (void)memcpy(identity->evm_payout_address, address, 20U);
    identity->has_evm_payout_binding = true;
    return LXP_OK;
}

lxp_result lxp_identity_retire(lxp_identity *identity,
                               bool every_balance_zero,
                               bool has_open_reference)
{
    if (identity == NULL) return LXP_ERR_UNKNOWN_DID;
    if (!every_balance_zero || has_open_reference)
        return LXP_ERR_ACCOUNT_NOT_EMPTY;
    identity->status = LXP_IDENTITY_RETIRED;
    return LXP_OK;
}
