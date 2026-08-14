#include "layerx/lxp_qualification.h"

#include <stdio.h>

int main(void)
{
    lxp_result status = lxp_qual_fault_boundaries();
    if (status == LXP_OK) status = lxp_partition_sim();
    if (status == LXP_OK) status = lxp_sequencer_loss_sim();
    if (status != LXP_OK) {
        (void)fprintf(stderr, "fault qualification failed: %d\n", (int)status);
        return 1;
    }
    return 0;
}
