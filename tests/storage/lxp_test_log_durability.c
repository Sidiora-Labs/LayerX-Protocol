#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_storage.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

static int run_child(const char *directory, uint32_t abort_boundary)
{
    const uint8_t activity[] = { 1U, 2U };
    const uint8_t receipt[] = { 3U, 4U };
    const uint8_t state_diff[] = { 5U, 6U };
    const uint8_t batch[] = { 7U, 8U };
    lxp_log log;
    if (lxp_log_segment_create(&log, directory, 0U, 4096U) != LXP_OK)
        return 1;
    if (lxp_log_append(&log, LXP_LOG_ACTIVITY, 11U, activity,
                       (uint32_t)sizeof(activity), NULL) != LXP_OK ||
        lxp_log_write_boundary(&log) != LXP_OK) return 1;
    if (lxp_log_fault_point(1U, abort_boundary)) _exit(81);
    if (lxp_log_append(&log, LXP_LOG_RECEIPT, 11U, receipt,
                       (uint32_t)sizeof(receipt), NULL) != LXP_OK ||
        lxp_log_append(&log, LXP_LOG_STATE_DIFF, 11U, state_diff,
                       (uint32_t)sizeof(state_diff), NULL) != LXP_OK ||
        lxp_log_write_boundary(&log) != LXP_OK) return 1;
    if (lxp_log_fault_point(2U, abort_boundary)) _exit(82);
    if (lxp_log_append(&log, LXP_LOG_BATCH_HEADER, 11U, batch,
                       (uint32_t)sizeof(batch), NULL) != LXP_OK ||
        lxp_log_write_boundary(&log) != LXP_OK) return 1;
    if (lxp_log_fault_point(3U, abort_boundary)) _exit(83);
    return lxp_log_close(&log) == LXP_OK ? 0 : 1;
}

int main(void)
{
    uint32_t boundary;
    for (boundary = 1U; boundary <= 3U; ++boundary) {
        char directory[] = "/tmp/lxp-durable-XXXXXX";
        char path[128];
        pid_t child;
        int child_status;
        lxp_log log;
        uint64_t durable;
        if (mkdtemp(directory) == NULL) return 1;
        child = fork();
        if (child < 0) return 1;
        if (child == 0) _exit(run_child(directory, boundary));
        if (waitpid(child, &child_status, 0) != child ||
            !WIFEXITED(child_status) || WEXITSTATUS(child_status) == 0)
            return 1;
        if (snprintf(path, sizeof(path), "%s/%020u.lxp", directory, 0U) < 0)
            return 1;
        if (lxp_log_open(&log, path) != LXP_OK ||
            lxp_log_durable_head(&log, &durable) != LXP_OK) return 1;
        if ((boundary == 1U && durable != UINT64_MAX) ||
            (boundary > 1U && durable != 11U)) return 1;
        if (lxp_log_close(&log) != LXP_OK || unlink(path) != 0 ||
            rmdir(directory) != 0) return 1;
    }
    return 0;
}
