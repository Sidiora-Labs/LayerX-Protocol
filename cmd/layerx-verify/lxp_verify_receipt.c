#include "layerx/lxp_verify.h"

lxp_result lxp_verify_receipt_against_requirement(
    const lxp_receipt *receipt,
    const lxp_payment_requirement *requirement,
    uint32_t receipt_network_id,
    const uint8_t sequencer_public_key[32],
    lxp_arena *arena)
{
    lxp_result status = lxp_receipt_verify_offline(
        receipt, sequencer_public_key, arena);
    if (status == LXP_OK)
        status = lxp_receipt_match_requirement(
            receipt, requirement, receipt_network_id);
    return status;
}
