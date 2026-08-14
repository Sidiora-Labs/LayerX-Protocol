#include "layerx/lxp_merkle.h"

#include <string.h>

int main(void)
{
    enum { LEAVES = 1000 };
    uint8_t arena_memory[131072];
    uint8_t hashes[LEAVES][32];
    uint8_t root[32];
    lxp_arena arena;
    lxp_merkle_proof proof;
    size_t i;
    for (i = 0U; i < LEAVES; ++i) {
        uint8_t value[8];
        size_t j;
        for (j = 0U; j < sizeof(value); ++j)
            value[sizeof(value)-1U-j]=(uint8_t)((uint64_t)i>>(j*8U));
        if (lxp_merkle_leaf_hash(value,sizeof(value),hashes[i])!=LXP_OK) return 1;
    }
    if (lxp_arena_init(&arena,arena_memory,sizeof(arena_memory))!=LXP_OK) return 1;
    for (i = 0U; i < LEAVES; ++i) {
        if (lxp_merkle_proof_generate((const uint8_t (*)[32])hashes,LEAVES,i,&arena,&proof,root)!=LXP_OK ||
            lxp_merkle_proof_verify(hashes[i],&proof,root)!=LXP_OK) return 1;
    }
    if (lxp_merkle_proof_generate((const uint8_t (*)[32])hashes,LEAVES,517U,&arena,&proof,root)!=LXP_OK)
        return 1;
    for (i = 0U; i < (size_t)proof.depth*32U*8U; ++i) {
        uint8_t *bytes=(uint8_t *)proof.siblings;
        bytes[i/8U]^=(uint8_t)(1U<<(i&7U));
        if (lxp_merkle_proof_verify(hashes[517],&proof,root)==LXP_OK) return 1;
        bytes[i/8U]^=(uint8_t)(1U<<(i&7U));
    }
    proof.depth=(uint8_t)(proof.depth+1U);
    return lxp_merkle_proof_verify(hashes[517],&proof,root)==LXP_ERR_NON_CANONICAL ? 0 : 1;
}
