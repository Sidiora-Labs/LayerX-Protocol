#ifndef LAYERX_LXP_IDENTITY_H
#define LAYERX_LXP_IDENTITY_H

#include "layerx/lxp_result.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef enum lxp_identity_status {
    LXP_IDENTITY_ACTIVE = 1,
    LXP_IDENTITY_RECOVERING = 2,
    LXP_IDENTITY_FROZEN = 3,
    LXP_IDENTITY_RETIRED = 4
} lxp_identity_status;

typedef struct lxp_identity {
    uint8_t did_id[32];
    lxp_identity_status status;
    uint8_t primary_key[32];
    uint8_t pending_key[32];
    bool has_pending_key;
    uint64_t rotation_announced_at;
    uint64_t rotation_effective_at;
    uint64_t rotation_lapse_at;
    uint64_t rotation_effective_sequence;
    uint8_t superseded_key[32];
    bool has_superseded_key;
    uint64_t next_sequence;
    uint64_t revocation_sequence;
    uint8_t recovery_root[32];
    uint16_t recovery_threshold;
    uint8_t recovery_pending_key[32];
    uint16_t recovery_approvals;
    uint64_t recovery_effective_at;
    uint64_t recovery_lapse_at;
    bool recovery_vetoed;
    uint8_t evm_payout_address[20];
    bool has_evm_payout_binding;
} lxp_identity;
#define lxp_identity lxp_identity

enum { LXP_IDENTITY_STORE_CAPACITY = 256 };
typedef struct lxp_identity_store {
    lxp_identity identities[LXP_IDENTITY_STORE_CAPACITY];
    size_t count;
} lxp_identity_store;

lxp_result lxp_did_id_derive(const uint8_t *did, size_t did_length,
                             uint8_t did_id[32]);
lxp_result lxp_account_id_derive(const uint8_t *account_name,
                                 size_t account_name_length,
                                 uint8_t account_id[32]);
lxp_result lxp_identity_register(lxp_identity_store *store,
                                 const uint8_t *did, size_t did_length,
                                 const uint8_t primary_key[32],
                                 lxp_identity **identity);
lxp_result lxp_identity_resolve(lxp_identity_store *store,
                                const uint8_t *did, size_t did_length,
                                lxp_identity **identity);
lxp_result lxp_identity_consume_sequence(lxp_identity *identity,
                                         uint64_t account_sequence);
lxp_result lxp_identity_rotate_announce(lxp_identity *identity,
                                        const uint8_t pending_key[32],
                                        uint64_t batch_timestamp,
                                        uint64_t challenge_delay,
                                        uint64_t effective_sequence);
lxp_result lxp_identity_rotate_commit(lxp_identity *identity,
                                      uint64_t batch_timestamp);
bool lxp_identity_key_valid(const lxp_identity *identity,
                            const uint8_t key[32], uint64_t batch_timestamp,
                            uint64_t global_sequence);
lxp_result lxp_identity_recover_begin(lxp_identity *identity,
                                      const uint8_t recovered_key[32],
                                      uint16_t approvals,
                                      uint64_t batch_timestamp,
                                      uint64_t challenge_delay);
lxp_result lxp_identity_recover_veto(lxp_identity *identity);
lxp_result lxp_identity_recover_commit(lxp_identity *identity,
                                       uint64_t batch_timestamp);
lxp_result lxp_identity_evm_binding_digest(const lxp_identity *identity,
                                           uint32_t network_id,
                                           uint8_t digest[32]);
lxp_result lxp_identity_bind_evm_payout(lxp_identity *identity,
                                        uint32_t network_id,
                                        const uint8_t signature[64],
                                        uint8_t recovery_id);
lxp_result lxp_identity_retire(lxp_identity *identity,
                               bool every_balance_zero,
                               bool has_open_reference);

#endif
