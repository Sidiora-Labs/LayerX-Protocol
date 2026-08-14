#include "layerx/lxp_governance.h"

#include <stdint.h>
#include <string.h>

static lxp_byte_span span(uint8_t *bytes, size_t length)
{
    return (lxp_byte_span){bytes, length};
}

static int configure(lxp_param_table *table, int reverse)
{
    uint8_t fee_key[] = "fee.base";
    uint8_t limit_key[] = "market.limit";
    if (lxp_param_table_init(table) != LXP_OK) return 1;
    if (reverse) {
        if (lxp_param_set_bounds(table,
                span(fee_key, sizeof(fee_key) - 1U), 2U,
                1U, 1000U, 10U, 1U) != LXP_OK ||
            lxp_param_set_bounds(table,
                span(limit_key, sizeof(limit_key) - 1U), 7U,
                10U, 100000U, 1000U, 1U) != LXP_OK)
            return 1;
    } else if (lxp_param_set_bounds(table,
                   span(limit_key, sizeof(limit_key) - 1U), 7U,
                   10U, 100000U, 1000U, 1U) != LXP_OK ||
               lxp_param_set_bounds(table,
                   span(fee_key, sizeof(fee_key) - 1U), 2U,
                   1U, 1000U, 10U, 1U) != LXP_OK) {
        return 1;
    }
    return 0;
}

int main(void)
{
    uint8_t fee_key[] = "fee.base";
    uint8_t market_a[32] = {1U};
    uint8_t market_b[32] = {2U};
    uint8_t outsider[32] = {3U};
    uint8_t cohort[2][32];
    uint8_t root_a[32];
    uint8_t root_b[32];
    lxp_param_table replica_a;
    lxp_param_table replica_b;
    lxp_param_table unchanged;
    lxp_gov_param_proposal proposal;
    lxp_gov_param_proposal invalid;
    uint64_t value;
    uint32_t version_before;
    uint32_t version_after;

    if (configure(&replica_a, 0) != 0 || configure(&replica_b, 1) != 0)
        return 1;
    (void)memcpy(cohort[0], market_a, 32U);
    (void)memcpy(cohort[1], market_b, 32U);
    (void)memset(&proposal, 0, sizeof(proposal));
    proposal.proposal_id[0] = 11U;
    proposal.target_module = 2U;
    (void)memcpy(proposal.parameter_key, fee_key, sizeof(fee_key) - 1U);
    proposal.parameter_key_length = sizeof(fee_key) - 1U;
    proposal.proposed_value = 25U;
    proposal.activation_epoch = 5U;
    proposal.ordered_sequence = 90U;
    if (lxp_gov_stage_cohort(&proposal, LXP_GOV_ROLLOUT_MARKET,
                             (const uint8_t (*)[32])cohort, 2U) != LXP_OK ||
        lxp_gov_param_propose(&replica_a, &proposal, 2U, 2U,
                              true, true) != LXP_OK ||
        lxp_gov_param_propose(&replica_b, &proposal, 2U, 2U,
                              true, true) != LXP_OK)
        return 1;
    if (lxp_gov_param_enact(
            &replica_a, span(fee_key, sizeof(fee_key) - 1U), 4U, market_a,
            &value, &version_before) != LXP_OK || value != 10U ||
        lxp_gov_activation_apply(&replica_a, 4U, true) != LXP_OK ||
        lxp_gov_activation_apply(&replica_a, 5U, false) !=
            LXP_ERR_PARAMETER_BOUNDS)
        return 1;

    unchanged = replica_a;
    invalid = proposal;
    invalid.proposal_id[0] = 12U;
    invalid.ordered_sequence = 91U;
    invalid.activation_epoch = 3U;
    if (lxp_gov_param_propose(&replica_a, &invalid, 2U, 2U,
                              true, true) != LXP_ERR_PARAMETER_BOUNDS ||
        memcmp(&unchanged, &replica_a, sizeof(replica_a)) != 0)
        return 1;
    invalid = proposal;
    invalid.proposal_id[0] = 13U;
    invalid.ordered_sequence = 92U;
    invalid.target_module = 99U;
    if (lxp_gov_param_propose(&replica_a, &invalid, 2U, 2U,
                              true, true) != LXP_ERR_PARAMETER_BOUNDS ||
        memcmp(&unchanged, &replica_a, sizeof(replica_a)) != 0)
        return 1;

    if (lxp_gov_activation_apply(&replica_a, 5U, true) != LXP_OK ||
        lxp_gov_activation_apply(&replica_b, 5U, true) != LXP_OK ||
        lxp_gov_param_enact(
            &replica_a, span(fee_key, sizeof(fee_key) - 1U), 5U, market_a,
            &value, &version_after) != LXP_OK || value != 25U ||
        version_after <= version_before ||
        lxp_gov_param_enact(
            &replica_a, span(fee_key, sizeof(fee_key) - 1U), 5U, outsider,
            &value, &version_after) != LXP_OK || value != 10U ||
        lxp_gov_parameter_state_root(&replica_a, 5U, market_a, root_a) !=
            LXP_OK ||
        lxp_gov_parameter_state_root(&replica_b, 5U, market_a, root_b) !=
            LXP_OK || memcmp(root_a, root_b, 32U) != 0)
        return 1;

    if (lxp_param_mark_sealed(&replica_a, 5U) != LXP_OK)
        return 1;
    invalid = proposal;
    invalid.proposal_id[0] = 14U;
    invalid.ordered_sequence = 93U;
    invalid.activation_epoch = 5U;
    if (lxp_gov_param_propose(&replica_a, &invalid, 4U, 1U,
                              true, true) != LXP_ERR_PARAMETER_BOUNDS)
        return 1;

    (void)memset(&proposal, 0, sizeof(proposal));
    proposal.proposal_id[0] = 15U;
    proposal.target_module = 2U;
    (void)memcpy(proposal.parameter_key, fee_key, sizeof(fee_key) - 1U);
    proposal.parameter_key_length = sizeof(fee_key) - 1U;
    proposal.proposed_value = 30U;
    proposal.activation_epoch = 7U;
    proposal.ordered_sequence = 94U;
    if (lxp_gov_stage_cohort(&proposal, LXP_GOV_ROLLOUT_ALL, NULL, 0U) !=
            LXP_OK ||
        lxp_gov_param_propose(&replica_a, &proposal, 5U, 2U,
                              true, true) != LXP_OK ||
        lxp_gov_activation_apply(&replica_a, 7U, true) != LXP_OK ||
        lxp_gov_param_enact(
            &replica_a, span(fee_key, sizeof(fee_key) - 1U), 7U, market_a,
            &value, &version_after) != LXP_OK || value != 30U)
        return 1;
    return 0;
}
