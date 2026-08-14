#include "layerx/lx_batch.h"

#include "layerx/lxp_merkle.h"

#include <string.h>

static void put_u64(uint8_t bytes[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i)
        bytes[i] = (uint8_t)(value >> ((7U - i) * 8U));
}

lxp_result lx_oracle_leaf_encode(const lx_oracle_accepted *accepted,
                                 uint8_t *bytes, size_t capacity,
                                 size_t *length)
{
    if (accepted == NULL || bytes == NULL || length == NULL ||
        capacity < LX_ORACLE_LEAF_BYTES ||
        accepted->payload_length != LX_ORACLE_OBSERVATION_BYTES)
        return LXP_ERR_NON_CANONICAL;
    (void)memcpy(bytes, accepted->payload, LX_ORACLE_OBSERVATION_BYTES);
    (void)memcpy(bytes + LX_ORACLE_OBSERVATION_BYTES,
                 accepted->observation.oracle_public_key, 32U);
    (void)memcpy(bytes + LX_ORACLE_OBSERVATION_BYTES + 32U,
                 accepted->observation.signature, 64U);
    put_u64(bytes + LX_ORACLE_OBSERVATION_BYTES + 96U,
            accepted->global_sequence);
    *length = LX_ORACLE_LEAF_BYTES;
    return LXP_OK;
}

static void leaves_sort(
    uint8_t leaves[LX_ORACLE_STORE_CAPACITY][LX_ORACLE_LEAF_BYTES],
    size_t count)
{
    size_t i;
    for (i = 1U; i < count; ++i) {
        uint8_t current[LX_ORACLE_LEAF_BYTES];
        size_t position = i;
        (void)memcpy(current, leaves[i], sizeof(current));
        while (position != 0U &&
               memcmp(current, leaves[position - 1U], sizeof(current)) < 0) {
            (void)memcpy(leaves[position], leaves[position - 1U],
                         sizeof(current));
            --position;
        }
        (void)memcpy(leaves[position], current, sizeof(current));
    }
}

lxp_result lx_oracle_availability_bundle_build(
    const lx_oracle_store *store, lx_oracle_availability_bundle *bundle)
{
    size_t i;
    if (store == NULL || bundle == NULL ||
        store->count > LX_ORACLE_STORE_CAPACITY)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(bundle, 0, sizeof(*bundle));
    for (i = 0U; i < store->count; ++i) {
        size_t length;
        lxp_result status = lx_oracle_leaf_encode(
            &store->accepted[i], bundle->leaves[i],
            LX_ORACLE_LEAF_BYTES, &length);
        if (status != LXP_OK || length != LX_ORACLE_LEAF_BYTES)
            return status != LXP_OK ? status : LXP_FATAL_INVARIANT;
    }
    bundle->count = store->count;
    leaves_sort(bundle->leaves, bundle->count);
    return LXP_OK;
}

lxp_result lx_oracle_root_from_availability(
    const lx_oracle_availability_bundle *bundle, lxp_arena *arena,
    uint8_t root[32])
{
    uint8_t hashes[LX_ORACLE_STORE_CAPACITY][32];
    size_t i;
    lxp_result status;
    if (bundle == NULL || arena == NULL || root == NULL ||
        bundle->count == 0U || bundle->count > LX_ORACLE_STORE_CAPACITY)
        return LXP_ERR_NON_CANONICAL;
    for (i = 0U; i < bundle->count; ++i) {
        status = lxp_merkle_leaf_hash(bundle->leaves[i],
                                      LX_ORACLE_LEAF_BYTES, hashes[i]);
        if (status != LXP_OK) return status;
    }
    return lxp_merkle_build((const uint8_t (*)[32])hashes,
                            bundle->count, arena, root);
}

lxp_result lx_oracle_root_compute(const lx_oracle_store *store,
                                  lxp_arena *arena, uint8_t root[32])
{
    lx_oracle_availability_bundle bundle;
    lxp_result status = lx_oracle_availability_bundle_build(store, &bundle);
    if (status != LXP_OK) return status;
    return lx_oracle_root_from_availability(&bundle, arena, root);
}

lxp_result lx_batch_header_set_oracle_root(lx_batch_header *header,
                                           const lx_oracle_store *store,
                                           lxp_arena *arena)
{
    uint8_t root[32];
    lxp_result status;
    if (header == NULL) return LXP_ERR_NON_CANONICAL;
    status = lx_oracle_root_compute(store, arena, root);
    if (status == LXP_OK) (void)memcpy(header->oracle_root, root, 32U);
    return status;
}
