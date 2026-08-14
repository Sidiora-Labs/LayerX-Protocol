#ifndef LAYERX_LXP_MERKLE_H
#define LAYERX_LXP_MERKLE_H

#include "layerx/lxp_codec.h"
#include "layerx/lxp_hash.h"

typedef struct lxp_merkle_leaf {
    lxp_byte_span key;
    lxp_byte_span value;
} lxp_merkle_leaf;

enum { LXP_MERKLE_MAX_DEPTH = 32 };
typedef struct lxp_merkle_proof {
    uint32_t leaf_index;
    uint32_t leaf_count;
    uint8_t depth;
    uint8_t siblings[LXP_MERKLE_MAX_DEPTH][32];
} lxp_merkle_proof;
#define lxp_merkle_proof lxp_merkle_proof

lxp_result lxp_merkle_leaf_hash(const void *data, size_t length, uint8_t out[32]);
lxp_result lxp_merkle_node_hash(const uint8_t left[32], const uint8_t right[32],
                                uint8_t out[32]);
lxp_result lxp_merkle_build(const uint8_t (*leaf_hashes)[32], size_t count,
                            lxp_arena *arena, uint8_t root[32]);
lxp_result lxp_merkle_root(const lxp_merkle_leaf *leaves, size_t count,
                           lxp_arena *arena, uint8_t root[32]);
lxp_result lxp_merkle_proof_generate(const uint8_t (*leaf_hashes)[32],
                                     size_t count, size_t leaf_index,
                                     lxp_arena *arena,
                                     lxp_merkle_proof *proof,
                                     uint8_t root[32]);
lxp_result lxp_merkle_proof_verify(const uint8_t leaf_hash[32],
                                   const lxp_merkle_proof *proof,
                                   const uint8_t expected_root[32]);
lxp_result lxp_merkle_proof_encode(lxp_codec_writer *writer,
                                   const lxp_merkle_proof *proof);

#endif
