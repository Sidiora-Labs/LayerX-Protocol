#include "layerx/lxp_storage.h"

#include <inttypes.h>
#include <stdio.h>

int main(int argc, char **argv)
{
    lxp_log log;
    uint64_t valid_end;
    uint64_t last_offset;
    uint64_t next_sequence;
    uint64_t durable;
    if (argc != 2 || lxp_log_open(&log, argv[1]) != LXP_OK ||
        lxp_log_scan_tail(&log, &valid_end, &last_offset, &next_sequence) !=
            LXP_OK || lxp_log_durable_head(&log, &durable) != LXP_OK)
        return 1;
    (void)printf("{\"segment_bytes\":%" PRIu64
                 ",\"valid_end\":%" PRIu64
                 ",\"last_record_offset\":%" PRIu64
                 ",\"durable_head\":%" PRIu64
                 ",\"resume_sequence\":%" PRIu64 "}\n",
                 log.capacity, valid_end, last_offset, durable, next_sequence);
    return lxp_log_close(&log) == LXP_OK ? 0 : 1;
}
