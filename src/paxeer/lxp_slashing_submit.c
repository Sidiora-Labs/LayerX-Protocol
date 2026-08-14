#include "layerx/lxp_guarantor.h"

#include <string.h>

lxp_result lxp_slashing_submit(
    const lxp_equivocation_evidence *evidence, lxp_guarantor_set *set,
    uint64_t removed_epoch, lxp_arena *arena)
{
    size_t i;
    lxp_result status;
    if (evidence == NULL || set == NULL || arena == NULL ||
        removed_epoch == 0U) return LXP_ERR_NON_CANONICAL;
    status = lxp_equivocation_verify(evidence, arena);
    if (status != LXP_OK) return status;
    if (evidence->kind != LXP_EQUIVOCATION_GUARANTOR)
        return LXP_OK;
    for (i = 0U; i < set->count; ++i)
        if (memcmp(set->records[i].guarantor_id,
                   evidence->guarantor_first.guarantor_id, 32U) == 0) {
            set->records[i].bond_amount = (lxp_u128){0U, 0U};
            set->records[i].active = false;
            set->records[i].jailed = true;
            set->records[i].unresolved_slashing = false;
            set->records[i].removed_epoch = removed_epoch;
            ++set->version;
            return LXP_OK;
        }
    return LXP_ERR_UNKNOWN_FIELD;
}
