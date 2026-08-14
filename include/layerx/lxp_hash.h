#ifndef LAYERX_LXP_HASH_H
#define LAYERX_LXP_HASH_H

#include "layerx/lxp_protocol.h"
#include "layerx/lxp_result.h"

#include <stddef.h>
#include <stdint.h>

enum { LXP_HASH_SIZE = 32, LXP_HASH_BLOCK_SIZE = 64 };

typedef struct lxp_hash_context {
    uint32_t state[8];
    uint64_t total_length;
    uint8_t block[LXP_HASH_BLOCK_SIZE];
    size_t block_length;
} lxp_hash_context;

void lxp_hash_init(lxp_hash_context *context);
lxp_result lxp_hash_update(lxp_hash_context *context, const void *data,
                           size_t length);
lxp_result lxp_hash_final(lxp_hash_context *context,
                          uint8_t digest[LXP_HASH_SIZE]);
lxp_result lxp_hash_sha256(const void *data, size_t length,
                           uint8_t digest[LXP_HASH_SIZE]);
lxp_result lxp_hash_domain(lxp_domain_tag_id domain, const void *data,
                           size_t length, uint8_t digest[LXP_HASH_SIZE]);
lxp_result lxp_hash_activity_id(const void *data, size_t length, uint8_t out[32]);
lxp_result lxp_hash_payload(const void *data, size_t length, uint8_t out[32]);
lxp_result lxp_hash_signature_preimage(const void *data, size_t length,
                                       uint8_t out[32]);
lxp_result lxp_hash_authority(const void *data, size_t length, uint8_t out[32]);
lxp_result lxp_hash_context_value(const void *data, size_t length,
                                  uint8_t out[32]);
lxp_result lxp_hash_account_id(const void *data, size_t length, uint8_t out[32]);

#endif
