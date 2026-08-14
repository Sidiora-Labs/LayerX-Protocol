#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_admission.h"
#include "layerx/lxp_batch.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

static int selection_and_validation(void)
{
    lxp_batch_header previous;
    lxp_batch_header candidate;
    (void)memset(&previous, 0, sizeof(previous));
    (void)memset(&candidate, 0, sizeof(candidate));
    previous.timestamp_ms = UINT64_C(1700000000000);
    if (lxp_batch_timestamp_select(&candidate, UINT64_C(1700000001000)) !=
            LXP_OK ||
        lxp_batch_timestamp_select(&candidate, UINT64_C(1700000001000)) !=
            LXP_OK ||
        lxp_batch_timestamp_select(&candidate, UINT64_C(1700000001001)) !=
            LXP_ERR_NON_CANONICAL ||
        lxp_batch_timestamp_validate(&previous, &candidate, 2000U) != LXP_OK)
        return 1;
    candidate.timestamp_ms = previous.timestamp_ms;
    if (lxp_batch_timestamp_validate(&previous, &candidate, 2000U) !=
        LXP_ERR_TIMESTAMP_REGRESSION) return 1;
    candidate.timestamp_ms = previous.timestamp_ms + 2001U;
    return lxp_batch_timestamp_validate(&previous, &candidate, 2000U) ==
           LXP_ERR_TIMESTAMP_REGRESSION ? 0 : 1;
}

static int replay_ignores_host_clock(void)
{
    lxp_batch_header sealed;
    lxp_exec_clock clock;
    lxp_timestamp_bound bound = { UINT64_C(1700000000000),
                                  UINT64_C(1700000002000) };
    uint64_t first;
    uint64_t second;
    lxp_result first_result;
    lxp_result second_result;
    (void)memset(&sealed, 0, sizeof(sealed));
    sealed.timestamp_ms = UINT64_C(1700000001000);
    if (lxp_exec_clock_bind(&clock, &sealed) != LXP_OK ||
        lxp_exec_clock_read(&clock, &first) != LXP_OK) return 1;
    first_result = lxp_activity_check_timestamp_bound(bound, first, 5000U);
    if (setenv("TZ", "Pacific/Kiritimati", 1) != 0 ||
        lxp_exec_clock_read(&clock, &second) != LXP_OK) return 1;
    second_result = lxp_activity_check_timestamp_bound(bound, second, 5000U);
    return first != second || first_result != LXP_OK ||
           second_result != first_result;
}

int main(void)
{
    return selection_and_validation() != 0 ||
           replay_ignores_host_clock() != 0;
}
