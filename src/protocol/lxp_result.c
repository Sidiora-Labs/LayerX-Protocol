#include "layerx/lxp_result.h"

const char *lxp_result_name(lxp_result result)
{
    switch (result) {
#define LXP_RESULT_CASE(name, value) case name: return #name;
        LXP_RESULT_CODE_LIST(LXP_RESULT_CASE)
#undef LXP_RESULT_CASE
        default:
            return "LXP_ERR_UNKNOWN";
    }
}

lxp_result_domain_id lxp_result_domain(lxp_result result)
{
    if (result == LXP_OK) {
        return LXP_RESULT_DOMAIN_SUCCESS;
    }
    if (result <= -1000) {
        return LXP_RESULT_DOMAIN_FATAL;
    }
    if (result <= -900) {
        return LXP_RESULT_DOMAIN_STORAGE;
    }
    if (result <= -800) {
        return LXP_RESULT_DOMAIN_BATCH;
    }
    if (result <= -700) {
        return LXP_RESULT_DOMAIN_MODULE;
    }
    if (result <= -600) {
        return LXP_RESULT_DOMAIN_METERING;
    }
    if (result <= -500) {
        return LXP_RESULT_DOMAIN_ARITHMETIC;
    }
    if (result <= -400) {
        return LXP_RESULT_DOMAIN_LEDGER;
    }
    if (result <= -300) {
        return LXP_RESULT_DOMAIN_SEQUENCING;
    }
    if (result <= -200) {
        return LXP_RESULT_DOMAIN_AUTHORITY;
    }
    if (result <= -100) {
        return LXP_RESULT_DOMAIN_ENVELOPE;
    }
    if (result < 0) {
        return LXP_RESULT_DOMAIN_CODEC;
    }
    return LXP_RESULT_DOMAIN_UNKNOWN;
}

bool lxp_result_is_fatal(lxp_result result)
{
    return result <= -1000;
}
