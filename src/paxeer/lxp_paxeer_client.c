#include "layerx/lxp_paxeer.h"

#include "layerx/lxp_crypto.h"

#include <string.h>

lxp_result lxp_paxeer_custody_abi_init(lxp_paxeer_custody_abi *abi)
{
    static const lxp_paxeer_custody_input_kind inputs[] = {
        LXP_PAXEER_INPUT_FINALISED_CHECKPOINT_CERTIFICATE,
        LXP_PAXEER_INPUT_STATE_PROOF,
        LXP_PAXEER_INPUT_WITHDRAWAL_NULLIFIER,
        LXP_PAXEER_INPUT_GUARANTOR_SIGNATURES,
        LXP_PAXEER_INPUT_CHALLENGE_WINDOW_STATE,
        LXP_PAXEER_INPUT_EMERGENCY_EXIT_ELIGIBILITY
    };
    if (abi == NULL) return LXP_ERR_NON_CANONICAL;
    (void)memset(abi, 0, sizeof(*abi));
    (void)memcpy(abi->inputs, inputs, sizeof(inputs));
    abi->input_count = LXP_PAXEER_CUSTODY_INPUT_COUNT;
    return LXP_OK;
}

lxp_result lxp_paxeer_verify_cert(
    lxp_checkpoint_registry_state *state,
    const lxp_guarantor_cert *certificate,
    const lxp_guarantor_set *guarantor_set,
    const lxp_finalisation_requirements *requirements,
    lxp_arena *arena, bool *finalised)
{
    lxp_finalisation_state candidate;
    lxp_result status;
    if (state == NULL || certificate == NULL || guarantor_set == NULL ||
        requirements == NULL || arena == NULL || finalised == NULL)
        return LXP_ERR_NON_CANONICAL;
    *finalised = false;
    candidate = state->finalisation;
    status = lxp_checkpoint_finalisable(&candidate, certificate,
                                        guarantor_set, requirements, arena,
                                        finalised);
    if (status != LXP_OK) return status;
    if (!*finalised) return LXP_ERR_ATTESTATION_THRESHOLD;
    state->finalisation = candidate;
    return LXP_OK;
}

lxp_result lxp_checkpoint_register(
    lxp_checkpoint_registry_state *state,
    const lxp_guarantor_cert *certificate,
    const lxp_guarantor_set *guarantor_set,
    const lxp_finalisation_requirements *requirements,
    lxp_arena *arena, lxp_checkpoint_registration *registration)
{
    lxp_checkpoint_registry_state candidate;
    lxp_byte_span header;
    uint8_t checkpoint_id[32];
    uint8_t header_hash[32];
    bool finalised = false;
    lxp_result status;
    if (state == NULL || certificate == NULL || guarantor_set == NULL ||
        requirements == NULL || arena == NULL || registration == NULL)
        return LXP_ERR_NON_CANONICAL;
    candidate = *state;
    status = lxp_paxeer_verify_cert(&candidate, certificate, guarantor_set,
                                     requirements, arena, &finalised);
    if (status != LXP_OK || !finalised) return status;
    status = lxp_batch_header_encode(&certificate->checkpoint.header, arena,
                                     &header);
    if (status != LXP_OK) return status;
    if (header.length != LXP_BATCH_HEADER_ENCODED_SIZE)
        return LXP_ERR_NON_CANONICAL;
    status = lxp_checkpoint_certificate_hash(&certificate->checkpoint, arena,
                                              checkpoint_id);
    if (status == LXP_OK)
        status = lxp_hash_domain(LXP_DOMAIN_BATCH_HEADER, header.bytes,
                                 header.length, header_hash);
    if (status != LXP_OK) return status;
    (void)memcpy(candidate.checkpoint_id, checkpoint_id,
                 sizeof(candidate.checkpoint_id));
    (void)memcpy(candidate.last_header_hash, header_hash,
                 sizeof(candidate.last_header_hash));
    candidate.registered_header_length = header.length;
    if (candidate.registration_count == UINT64_MAX) return LXP_ERR_OVERFLOW;
    ++candidate.registration_count;
    (void)memset(registration, 0, sizeof(*registration));
    registration->header_commitments = header;
    (void)memcpy(registration->checkpoint_id, checkpoint_id,
                 sizeof(registration->checkpoint_id));
    (void)memcpy(registration->resulting_state_root,
                 certificate->checkpoint.header.resulting_state_root,
                 sizeof(registration->resulting_state_root));
    registration->batch_number = certificate->checkpoint.header.batch_number;
    *state = candidate;
    return LXP_OK;
}
