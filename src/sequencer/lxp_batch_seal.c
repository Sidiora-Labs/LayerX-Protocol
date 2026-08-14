#include "layerx/lxp_batch.h"
#include "layerx/lxp_crypto.h"
#include "layerx/lxp_protocol.h"

#include <openssl/evp.h>
#include <string.h>

static int authorization_matches(
    const lxp_batch_header *header,
    const lxp_sequencer_authorization *authorization)
{
    return authorization != NULL && authorization->authorized == 1U &&
           header->batch_number >= authorization->first_batch_number &&
           header->batch_number <= authorization->last_batch_number &&
           lxp_ct_memcmp(header->sequencer_id,
                         authorization->sequencer_id, 32U) == 0;
}

lxp_result lxp_batch_seal(lxp_batch_header *header,
                          const lxp_batch_seal_input *input,
                          const lxp_batch_roots *roots, lxp_log *log,
                          lxp_arena *arena)
{
    lxp_byte_span encoded;
    size_t mark;
    lxp_result status;
    if (header == NULL || input == NULL || roots == NULL || log == NULL ||
        arena == NULL || !lxp_protocol_version_supported(
            input->protocol_version) || input->network_id == 0U ||
        input->last_sequence < input->first_sequence ||
        input->timestamp_ms == 0U) return LXP_ERR_NON_CANONICAL;
    (void)memset(header, 0, sizeof(*header));
    header->protocol_version = input->protocol_version;
    header->network_id = input->network_id;
    header->epoch = input->epoch;
    header->batch_number = input->batch_number;
    header->first_sequence = input->first_sequence;
    header->last_sequence = input->last_sequence;
    (void)memcpy(header->previous_state_root, input->previous_state_root, 32U);
    (void)memcpy(header->resulting_state_root, input->resulting_state_root, 32U);
    (void)memcpy(header->activity_merkle_root,
                 roots->activity_merkle_root, 32U);
    (void)memcpy(header->receipt_merkle_root,
                 roots->receipt_merkle_root, 32U);
    (void)memcpy(header->event_merkle_root, roots->event_merkle_root, 32U);
    (void)memcpy(header->data_availability_root,
                 roots->data_availability_root, 32U);
    (void)memcpy(header->oracle_root, roots->oracle_root, 32U);
    header->timestamp_ms = input->timestamp_ms;
    (void)memcpy(header->sequencer_id, input->sequencer_id, 32U);
    mark = lxp_arena_mark(arena);
    status = lxp_batch_header_encode(header, arena, &encoded);
    if (status == LXP_OK)
        status = lxp_log_append(log, LXP_LOG_BATCH_HEADER,
                                header->last_sequence, encoded.bytes,
                                (uint32_t)encoded.length, NULL);
    if (status == LXP_OK) status = lxp_log_write_boundary(log);
    (void)lxp_arena_reset(arena, mark);
    return status;
}

lxp_result lxp_batch_sign(const lxp_batch_header *header,
                          const uint8_t private_key[32],
                          const lxp_sequencer_authorization *authorization,
                          uint8_t signature[64], lxp_arena *arena)
{
    EVP_PKEY *key;
    EVP_MD_CTX *context;
    uint8_t public_key[32];
    uint8_t digest[32];
    size_t public_length = sizeof(public_key);
    size_t signature_length = 64U;
    int signed_ok;
    lxp_result status;
    if (header == NULL || private_key == NULL || signature == NULL ||
        arena == NULL || !authorization_matches(header, authorization))
        return LXP_ERR_AUTH_SCOPE;
    key = EVP_PKEY_new_raw_private_key(EVP_PKEY_ED25519, NULL, private_key,
                                       32U);
    if (key == NULL || EVP_PKEY_get_raw_public_key(
            key, public_key, &public_length) != 1 || public_length != 32U ||
        lxp_ct_memcmp(public_key, authorization->public_key, 32U) != 0) {
        EVP_PKEY_free(key);
        return LXP_ERR_AUTH_SCOPE;
    }
    status = lxp_batch_header_hash(header, arena, digest);
    context = status == LXP_OK ? EVP_MD_CTX_new() : NULL;
    signed_ok = context != NULL &&
        EVP_DigestSignInit(context, NULL, NULL, NULL, key) == 1 &&
        EVP_DigestSign(context, signature, &signature_length, digest,
                       sizeof(digest)) == 1 && signature_length == 64U;
    EVP_MD_CTX_free(context);
    EVP_PKEY_free(key);
    lxp_secure_zero(digest, sizeof(digest));
    lxp_secure_zero(public_key, sizeof(public_key));
    return status != LXP_OK ? status : signed_ok ? LXP_OK :
           LXP_ERR_BAD_SIGNATURE;
}

lxp_result lxp_batch_verify_signature(
    const lxp_batch_header *header, const uint8_t *signature,
    size_t signature_length,
    const lxp_sequencer_authorization *authorization, lxp_arena *arena)
{
    uint8_t digest[32];
    lxp_result status;
    if (header == NULL || signature == NULL || signature_length != 64U ||
        arena == NULL) return LXP_ERR_BAD_SIGNATURE;
    if (!authorization_matches(header, authorization))
        return LXP_ERR_AUTH_SCOPE;
    status = lxp_batch_header_hash(header, arena, digest);
    if (status == LXP_OK)
        status = lxp_ed25519_verify_raw(authorization->public_key, signature,
                                        digest, sizeof(digest));
    lxp_secure_zero(digest, sizeof(digest));
    return status;
}
