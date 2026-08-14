#include "layerx/lxp_admission.h"

#include <string.h>

static int is_clean_rejection(lxp_admission_result result,
                              lxp_result expected)
{
    return result.result_code == expected && !result.assign_global_sequence &&
           !result.consume_account_sequence && !result.charge_fee;
}

int main(void)
{
    static const uint8_t payload[] = { 1U };
    lxp_activity activity;
    lxp_admission_context context = {
        42U, 1000U, 100U, 7U, true, false, true
    };
    lxp_admission_result result;
    (void)memset(&activity, 0, sizeof(activity));
    activity.protocol_version = LXP_PROTOCOL_VERSION;
    activity.network_id = 42U;
    activity.account_sequence = 7U;
    activity.timestamp_bound = (lxp_timestamp_bound){ 950U, 1050U };
    activity.payload = (lxp_byte_span){ payload, sizeof(payload) };
    if (lxp_hash_payload(payload, sizeof(payload), activity.payload_hash) != LXP_OK)
        return 1;
    result = lxp_admit_activity(&activity, &context);
    if (result.result_code != LXP_OK || !result.assign_global_sequence ||
        !result.consume_account_sequence || !result.charge_fee) return 1;
    activity.timestamp_bound = (lxp_timestamp_bound){ 1001U, 1050U };
    if (!is_clean_rejection(lxp_admit_activity(&activity, &context),
                            LXP_ERR_NOT_YET_VALID)) return 1;
    activity.timestamp_bound = (lxp_timestamp_bound){ 900U, 999U };
    if (!is_clean_rejection(lxp_admit_activity(&activity, &context),
                            LXP_ERR_EXPIRED)) return 1;
    activity.timestamp_bound = (lxp_timestamp_bound){ 900U, 1001U };
    if (!is_clean_rejection(lxp_admit_activity(&activity, &context),
                            LXP_ERR_MALFORMED_ENVELOPE)) return 1;
    activity.timestamp_bound = (lxp_timestamp_bound){ 950U, 1050U };
    context.signature_valid = false;
    context.idempotency_key_exists = true;
    if (!is_clean_rejection(lxp_admit_activity(&activity, &context),
                            LXP_ERR_BAD_SIGNATURE)) return 1;
    context.signature_valid = true;
    context.idempotency_key_exists = false;
    context.fee_limit_spendable = false;
    if (!is_clean_rejection(lxp_admit_activity(&activity, &context),
                            LXP_ERR_FEE_UNPAYABLE)) return 1;
    return 0;
}
