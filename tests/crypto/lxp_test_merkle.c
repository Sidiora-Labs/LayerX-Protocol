#include "layerx/lxp_merkle.h"

#include <string.h>

int main(void)
{
    uint8_t memory[4096];
    lxp_arena arena;
    uint8_t empty[32], single[32], odd[32], repeated[32], leaf_hash[32];
    static const uint8_t k1[]={1U},k2[]={2U},k3[]={3U};
    static const uint8_t v1[]={'a'},v2[]={'b'},v3[]={'c'};
    lxp_merkle_leaf leaves[3]={{{k1,1U},{v1,1U}},{{k2,1U},{v2,1U}},{{k3,1U},{v3,1U}}};
    if(lxp_arena_init(&arena,memory,sizeof(memory))!=LXP_OK ||
       lxp_merkle_root(NULL,0U,&arena,empty)!=LXP_OK ||
       lxp_merkle_root(leaves,1U,&arena,single)!=LXP_OK ||
       lxp_merkle_leaf_hash(v1,1U,leaf_hash)!=LXP_OK || memcmp(single,leaf_hash,32U)!=0 ||
       lxp_merkle_root(leaves,3U,&arena,odd)!=LXP_OK ||
       lxp_merkle_root(leaves,3U,&arena,repeated)!=LXP_OK || memcmp(odd,repeated,32U)!=0)
        return 1;
    leaves[1].key=leaves[0].key;
    return lxp_merkle_root(leaves,3U,&arena,repeated)==LXP_ERR_UNSORTED_SEQUENCE ? 0 : 1;
}
