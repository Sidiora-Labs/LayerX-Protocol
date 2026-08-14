#include "layerx/lxp_tools.h"

#include "layerx/lxp_crypto.h"

#include <string.h>

static void put_u64(uint8_t out[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i)
        out[7U - i] = (uint8_t)(value >> (i * 8U));
}

lxp_result lxp_verify_main(
    const lxp_verify_run *run,
    uint8_t output[LXP_VERIFY_OUTPUT_BYTES])
{
    lxp_replay_batch_result replayed;
    uint8_t published_header_hash[32];
    uint8_t certified_header_hash[32];
    uint8_t checkpoint_id[32];
    size_t valid_signatures = 0U;
    lxp_result status;
    if (run == NULL || run->bundle == NULL || run->header == NULL ||
        run->certificate == NULL || run->guarantor_keys == NULL ||
        run->guarantor_key_count == 0U || run->engine == NULL ||
        run->starting_state_root == NULL || run->arena == NULL ||
        output == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = lxp_batch_header_hash(
        run->header, run->arena, published_header_hash);
    if (status == LXP_OK)
        status = lxp_batch_header_hash(
            &run->certificate->checkpoint.header,
            run->arena, certified_header_hash);
    if (status == LXP_OK && lxp_ct_memcmp(
            published_header_hash, certified_header_hash, 32U) != 0)
        status = LXP_ERR_ROOT_MISMATCH;
    if (status == LXP_OK)
        status = lxp_guarantor_cert_verify(
            run->certificate, run->guarantor_keys,
            run->guarantor_key_count, run->arena,
            &valid_signatures);
    if (status == LXP_OK)
        status = lxp_checkpoint_certificate_hash(
            &run->certificate->checkpoint, run->arena, checkpoint_id);
    if (status == LXP_OK &&
        (valid_signatures < run->certificate->threshold ||
         lxp_ct_memcmp(
             checkpoint_id,
             run->certificate->attestations[0].checkpoint_id,
             32U) != 0))
        status = LXP_ERR_ATTESTATION_THRESHOLD;
    if (status == LXP_OK)
        status = lxp_da_verify_served_bytes(
            run->bundle, run->header, run->engine,
            run->starting_state_root, run->arena, &replayed);
    if (status != LXP_OK) return status;
    (void)memcpy(output, "LXVF", 4U);
    output[4] = 1U;
    put_u64(output + 5U, run->header->batch_number);
    put_u64(output + 13U, run->header->first_sequence);
    put_u64(output + 21U, run->header->last_sequence);
    (void)memcpy(output + 29U, replayed.resulting_state_root, 32U);
    (void)memcpy(output + 61U,
                 replayed.roots.activity_merkle_root, 32U);
    (void)memcpy(output + 93U,
                 replayed.roots.receipt_merkle_root, 32U);
    (void)memcpy(output + 125U,
                 replayed.roots.event_merkle_root, 32U);
    (void)memcpy(output + 157U, replayed.roots.oracle_root, 32U);
    (void)memcpy(output + 189U,
                 run->header->data_availability_root, 32U);
    (void)memcpy(output + 221U, checkpoint_id, 32U);
    return LXP_OK;
}
