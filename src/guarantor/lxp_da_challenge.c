#include "layerx/lxp_guarantor.h"

#include "layerx/lxp_crypto.h"

#include <string.h>

static void put_u32(uint8_t out[4], uint32_t value)
{
    out[0] = (uint8_t)(value >> 24U);
    out[1] = (uint8_t)(value >> 16U);
    out[2] = (uint8_t)(value >> 8U);
    out[3] = (uint8_t)value;
}

static void put_u64(uint8_t out[8], uint64_t value)
{
    size_t i;
    for (i = 0U; i < 8U; ++i) out[7U - i] = (uint8_t)(value >> (i * 8U));
}

static uint64_t first_u64(const uint8_t digest[32])
{
    uint64_t value = 0U;
    size_t i;
    for (i = 0U; i < 8U; ++i) value = (value << 8U) | digest[i];
    return value;
}

lxp_result lxp_da_challenge_indices(
    const uint8_t checkpoint_hash[32], uint32_t chunk_count,
    size_t requested_count, uint32_t *indices)
{
    uint8_t material[36];
    uint8_t digest[32];
    uint32_t counter = 0U;
    size_t produced = 0U;
    lxp_result status;
    if (checkpoint_hash == NULL || indices == NULL || chunk_count == 0U ||
        requested_count == 0U ||
        requested_count > LXP_MAX_DA_CHALLENGE_INDICES ||
        requested_count > chunk_count)
        return LXP_ERR_NON_CANONICAL;
    (void)memcpy(material, checkpoint_hash, 32U);
    while (produced < requested_count) {
        uint32_t candidate;
        size_t i;
        bool duplicate = false;
        put_u32(material + 32U, counter++);
        status = lxp_hash_domain(LXP_DOMAIN_DA_CHALLENGE, material,
                                 sizeof(material), digest);
        if (status != LXP_OK) return status;
        candidate = (uint32_t)(first_u64(digest) % chunk_count);
        for (i = 0U; i < produced; ++i)
            if (indices[i] == candidate) duplicate = true;
        if (!duplicate) indices[produced++] = candidate;
    }
    lxp_secure_zero(digest, sizeof(digest));
    return LXP_OK;
}

lxp_result lxp_da_challenge_issue(
    const lxp_guarantor_attestation *attestation,
    const uint8_t checkpoint_hash[32], uint32_t chunk_index,
    uint32_t chunk_count, uint64_t issued_at_ms, uint64_t response_window_ms,
    lxp_da_challenge *challenge)
{
    uint8_t material[116];
    if (attestation == NULL || checkpoint_hash == NULL || challenge == NULL ||
        !attestation->da_possessed ||
        attestation->availability_class_mask !=
            LXP_GUARANTOR_AVAILABILITY_ALL ||
        chunk_count == 0U || chunk_index >= chunk_count || issued_at_ms == 0U ||
        response_window_ms == 0U || response_window_ms > UINT64_MAX - issued_at_ms)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(challenge, 0, sizeof(*challenge));
    (void)memcpy(challenge->checkpoint_hash, checkpoint_hash, 32U);
    challenge->signed_commitment = *attestation;
    challenge->batch_number = attestation->batch_number;
    challenge->chunk_index = chunk_index;
    challenge->chunk_count = chunk_count;
    challenge->issued_at_ms = issued_at_ms;
    challenge->deadline_ms = issued_at_ms + response_window_ms;
    (void)memcpy(material, checkpoint_hash, 32U);
    (void)memcpy(material + 32U, attestation->guarantor_id, 32U);
    (void)memcpy(material + 64U, attestation->data_availability_root, 32U);
    put_u64(material + 96U, challenge->batch_number);
    put_u32(material + 104U, chunk_index);
    put_u64(material + 108U, challenge->deadline_ms);
    return lxp_hash_domain(LXP_DOMAIN_DA_CHALLENGE, material,
                           sizeof(material), challenge->challenge_id);
}

lxp_result lxp_da_challenge_respond(
    const lxp_da_store *store, const lxp_da_challenge *challenge,
    uint64_t responded_at_ms, lxp_arena *arena,
    lxp_da_challenge_response *response)
{
    lxp_da_bundle bundle;
    uint8_t (*hashes)[32];
    uint8_t root[32];
    uint8_t proof_root[32];
    void *memory;
    size_t i;
    lxp_result status;
    if (store == NULL || challenge == NULL || arena == NULL ||
        response == NULL || responded_at_ms == 0U)
        return LXP_ERR_NON_CANONICAL;
    status = lxp_da_store_read_bundle(store, challenge->batch_number, arena,
                                      &bundle, root);
    if (status != LXP_OK) return status;
    if (bundle.chunk_count != challenge->chunk_count ||
        challenge->chunk_index >= bundle.chunk_count ||
        lxp_ct_memcmp(root,
            challenge->signed_commitment.data_availability_root, 32U) != 0)
        return LXP_ERR_DA_MISSING;
    status = lxp_arena_alloc(arena, bundle.chunk_count * 32U,
                             _Alignof(uint64_t), &memory);
    if (status != LXP_OK) return status;
    hashes = (uint8_t (*)[32])memory;
    for (i = 0U; i < bundle.chunk_count; ++i)
        (void)memcpy(hashes[i], bundle.chunks[i].chunk_hash, 32U);
    (void)memset(response, 0, sizeof(*response));
    (void)memcpy(response->challenge_id, challenge->challenge_id, 32U);
    response->chunk = bundle.chunks[challenge->chunk_index];
    response->responded_at_ms = responded_at_ms;
    status = lxp_merkle_proof_generate(
        (const uint8_t (*)[32])hashes, bundle.chunk_count,
        challenge->chunk_index, arena, &response->inclusion_proof, proof_root);
    if (status == LXP_OK && lxp_ct_memcmp(proof_root, root, 32U) != 0)
        status = LXP_ERR_DA_MISSING;
    return status;
}

static lxp_result failure(const lxp_da_challenge *challenge,
                          const lxp_da_challenge_response *response,
                          lxp_result code, bool *satisfied,
                          lxp_da_failure_evidence *evidence)
{
    *satisfied = false;
    (void)memset(evidence, 0, sizeof(*evidence));
    evidence->challenge = *challenge;
    evidence->failure_code = code;
    if (response != NULL) {
        evidence->served_bytes = response->chunk.bytes;
        (void)memcpy(evidence->served_chunk_hash,
                     response->chunk.chunk_hash, 32U);
    }
    return LXP_ERR_DA_MISSING;
}

lxp_result lxp_da_challenge_judge(
    const lxp_da_challenge *challenge,
    const lxp_da_challenge_response *response, uint64_t judged_at_ms,
    bool *satisfied, lxp_da_failure_evidence *evidence)
{
    lxp_da_chunk hashed;
    lxp_result status;
    if (challenge == NULL || satisfied == NULL || evidence == NULL ||
        judged_at_ms == 0U)
        return LXP_ERR_NON_CANONICAL;
    *satisfied = false;
    if (response == NULL) {
        if (judged_at_ms <= challenge->deadline_ms)
            return LXP_ERR_NOT_YET_VALID;
        return failure(challenge, NULL, LXP_ERR_DA_MISSING,
                       satisfied, evidence);
    }
    if (response->responded_at_ms < challenge->issued_at_ms ||
        response->responded_at_ms > challenge->deadline_ms ||
        judged_at_ms < response->responded_at_ms)
        return failure(challenge, response, LXP_ERR_NOT_YET_VALID,
                       satisfied, evidence);
    if (lxp_ct_memcmp(response->challenge_id,
                      challenge->challenge_id, 32U) != 0 ||
        response->chunk.batch_number != challenge->batch_number ||
        response->chunk.chunk_index != challenge->chunk_index ||
        response->inclusion_proof.leaf_index != challenge->chunk_index ||
        response->inclusion_proof.leaf_count != challenge->chunk_count)
        return failure(challenge, response, LXP_ERR_NON_CANONICAL,
                       satisfied, evidence);
    hashed = response->chunk;
    status = lxp_da_chunk_hash(&hashed);
    if (status != LXP_OK ||
        lxp_ct_memcmp(hashed.chunk_hash,
                      response->chunk.chunk_hash, 32U) != 0)
        return failure(challenge, response, LXP_ERR_ROOT_MISMATCH,
                       satisfied, evidence);
    status = lxp_merkle_proof_verify(
        response->chunk.chunk_hash, &response->inclusion_proof,
        challenge->signed_commitment.data_availability_root);
    if (status != LXP_OK)
        return failure(challenge, response, status, satisfied, evidence);
    (void)memset(evidence, 0, sizeof(*evidence));
    *satisfied = true;
    return LXP_OK;
}
