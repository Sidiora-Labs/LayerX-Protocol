#include "layerx/lxp_guarantor.h"

#include <string.h>

lxp_result lxp_guarantor_set_init(lxp_guarantor_set *set)
{
    if (set == NULL) return LXP_ERR_NON_CANONICAL;
    (void)memset(set, 0, sizeof(*set));
    return LXP_OK;
}

lxp_result lxp_guarantor_set_apply(
    lxp_guarantor_set *set, uint64_t governance_sequence,
    bool ordered_governance_activity,
    const lxp_guarantor_bond_state *bond_state)
{
    size_t i;
    if (set == NULL || bond_state == NULL || !ordered_governance_activity)
        return LXP_ERR_AUTH_SCOPE;
    if (governance_sequence != set->last_governance_sequence + 1U)
        return LXP_ERR_SEQUENCE_MISMATCH;
    for (i = 0U; i < set->count; ++i)
        if (memcmp(set->records[i].guarantor_id,
                   bond_state->guarantor_id, 32U) == 0) {
            if (bond_state->joined_epoch != set->records[i].joined_epoch)
                return LXP_ERR_NON_CANONICAL;
            set->records[i] = *bond_state;
            set->last_governance_sequence = governance_sequence;
            ++set->version;
            return LXP_OK;
        }
    if (set->count == LXP_MAX_GUARANTOR_ATTESTATIONS ||
        !bond_state->active || bond_state->joined_epoch == 0U)
        return LXP_ERR_NON_CANONICAL;
    set->records[set->count++] = *bond_state;
    set->last_governance_sequence = governance_sequence;
    ++set->version;
    return LXP_OK;
}

lxp_result lxp_guarantor_eligible(
    const lxp_guarantor_bond_state *bond_state, uint64_t checkpoint_epoch,
    lxp_u128 minimum_bond, bool *eligible)
{
    if (bond_state == NULL || eligible == NULL || checkpoint_epoch == 0U)
        return LXP_ERR_NON_CANONICAL;
    *eligible = bond_state->active && !bond_state->jailed &&
        !bond_state->unresolved_slashing &&
        bond_state->joined_epoch <= checkpoint_epoch &&
        (bond_state->removed_epoch == 0U ||
         bond_state->removed_epoch > checkpoint_epoch) &&
        lxp_u128_cmp(bond_state->bond_amount, minimum_bond) >= 0;
    return LXP_OK;
}
