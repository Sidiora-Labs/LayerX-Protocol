#include "layerx/lxp_replica.h"
#include "layerx/lxp_crypto.h"
#include "layerx/lxp_protocol.h"
#include "layerx/lxp_sequencer.h"

#include <string.h>

lxp_result lxp_replica_init(lxp_replica *replica, lxp_log *log)
{
    if (replica == NULL || log == NULL || log->descriptor < 0)
        return LXP_ERR_NON_CANONICAL;
    (void)memset(replica, 0, sizeof(*replica));
    replica->log = log;
    replica->execution_enabled = true;
    replica->acknowledgements_enabled = true;
    replica->serving_current_state = true;
    replica->serving_finalised_history = true;
    return LXP_OK;
}

lxp_result lxp_replica_validate_header(
    const lxp_batch_body *body, uint32_t configured_network_id,
    const lxp_sequencer_authorization *authorization, lxp_arena *arena)
{
    if (body == NULL || authorization == NULL || arena == NULL)
        return LXP_ERR_NON_CANONICAL;
    if (!lxp_protocol_version_supported(body->header.protocol_version))
        return LXP_ERR_VERSION_UNSUPPORTED;
    if (!lxp_network_id_matches(configured_network_id,
                                body->header.network_id))
        return LXP_ERR_WRONG_NETWORK;
    return lxp_batch_verify_signature(&body->header,
                                      body->sequencer_signature, 64U,
                                      authorization, arena);
}

lxp_result lxp_replica_chain_link(const lxp_batch_header *previous,
                                  const lxp_batch_header *candidate)
{
    return lxp_batch_range_check(previous, candidate);
}

lxp_result lxp_replica_ingest_batch(
    lxp_replica *replica, const uint8_t *canonical_body, size_t body_length,
    uint32_t configured_network_id,
    const lxp_sequencer_authorization *authorization, lxp_arena *arena,
    bool *acknowledge)
{
    lxp_batch_body decoded;
    lxp_byte_span reencoded;
    size_t mark;
    lxp_result status;
    if (replica == NULL || replica->log == NULL ||
        (canonical_body == NULL && body_length != 0U) || arena == NULL ||
        acknowledge == NULL) return LXP_ERR_NON_CANONICAL;
    *acknowledge = false;
    if (replica->halted || !replica->execution_enabled ||
        !replica->acknowledgements_enabled)
        return LXP_FATAL_REPLAY_DIVERGENCE;
    mark = lxp_arena_mark(arena);
    status = lxp_batch_body_decode(canonical_body, body_length, &decoded);
    if (status == LXP_OK)
        status = lxp_batch_body_encode(&decoded, arena, &reencoded);
    if (status == LXP_OK &&
        (reencoded.length != body_length ||
         lxp_ct_memcmp(reencoded.bytes, canonical_body, body_length) != 0))
        status = LXP_ERR_NON_CANONICAL;
    if (status == LXP_OK)
        status = lxp_replica_validate_header(&decoded,
                    configured_network_id, authorization, arena);
    if (status == LXP_OK && replica->has_head)
        status = lxp_replica_chain_link(&replica->head, &decoded.header);
    if (status == LXP_OK && !replica->has_head &&
        (decoded.header.batch_number != 0U ||
         decoded.header.first_sequence != 0U ||
         decoded.header.last_sequence < decoded.header.first_sequence ||
         !lxp_ct_is_zero(decoded.header.previous_state_root, 32U)))
        status = LXP_ERR_BATCH_GAP;
    if (status == LXP_OK && body_length > UINT32_MAX)
        status = LXP_ERR_LENGTH_LIMIT;
    if (status == LXP_OK)
        status = lxp_log_append(replica->log, LXP_LOG_BATCH_BODY,
                                decoded.header.last_sequence, canonical_body,
                                (uint32_t)body_length, NULL);
    if (status == LXP_OK) status = lxp_log_write_boundary(replica->log);
    if (status == LXP_OK) {
        replica->head = decoded.header;
        replica->has_head = true;
        replica->durable_batch_count += 1U;
        *acknowledge = true;
    }
    (void)lxp_arena_reset(arena, mark);
    return status;
}
