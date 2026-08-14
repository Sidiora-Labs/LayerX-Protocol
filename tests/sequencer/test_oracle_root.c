#include "layerx/lx_batch.h"

#include <string.h>

static int add(lx_oracle_store *store, uint8_t market, uint64_t sequence,
               uint64_t price, uint64_t global_sequence, uint8_t seed_marker)
{
    uint8_t seed[32] = { 0U };
    uint8_t payload[LX_ORACLE_OBSERVATION_BYTES];
    lx_oracle_observation observation;
    size_t payload_length;
    seed[0] = seed_marker;
    (void)memset(&observation, 0, sizeof(observation));
    observation.market_id[0] = market;
    observation.observation_sequence = sequence;
    observation.price = (lxp_u128){ 0U, price };
    observation.observed_at = 1000U + sequence;
    observation.source_identifier = 42U;
    if (lx_oracle_observation_sign(&observation, seed) != LXP_OK ||
        lx_oracle_observation_encode(&observation, payload, sizeof(payload),
                                     &payload_length) != LXP_OK ||
        lx_oracle_store_put(store, &observation, payload, payload_length,
                            global_sequence) != LXP_OK)
        return 1;
    return 0;
}

int main(void)
{
    lx_oracle_store store;
    lx_oracle_availability_bundle bundle;
    lx_batch_header header;
    lxp_arena first_arena;
    lxp_arena second_arena;
    uint8_t first_bytes[4096];
    uint8_t second_bytes[4096];
    uint8_t recomputed[32];

    (void)memset(&store, 0, sizeof(store));
    (void)memset(&header, 0, sizeof(header));
    if (add(&store, 3U, 1U, 100U, 3U, 1U) != 0 ||
        add(&store, 1U, 1U, 200U, 1U, 2U) != 0 ||
        add(&store, 2U, 1U, 300U, 2U, 3U) != 0 ||
        lxp_arena_init(&first_arena, first_bytes, sizeof(first_bytes)) != LXP_OK ||
        lx_batch_header_set_oracle_root(&header, &store,
                                        &first_arena) != LXP_OK ||
        lx_oracle_availability_bundle_build(&store, &bundle) != LXP_OK ||
        bundle.count != 3U || bundle.leaves[0][0] != 1U ||
        bundle.leaves[1][0] != 2U || bundle.leaves[2][0] != 3U ||
        lxp_arena_init(&second_arena, second_bytes,
                       sizeof(second_bytes)) != LXP_OK ||
        lx_oracle_root_from_availability(&bundle, &second_arena,
                                         recomputed) != LXP_OK ||
        memcmp(header.oracle_root, recomputed, 32U) != 0)
        return 1;
    bundle.leaves[1][10] ^= 1U;
    if (lxp_arena_init(&second_arena, second_bytes,
                       sizeof(second_bytes)) != LXP_OK ||
        lx_oracle_root_from_availability(&bundle, &second_arena,
                                         recomputed) != LXP_OK ||
        memcmp(header.oracle_root, recomputed, 32U) == 0)
        return 1;
    return 0;
}
