#define _POSIX_C_SOURCE 200809L

#include "layerx/lxp_projection.h"

#include <sqlite3.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(void)
{
    char path[] = "/tmp/lxp-projection-XXXXXX";
    int descriptor = mkstemp(path);
    const uint8_t receipt[] = { 1U, 2U, 3U };
    lxp_projection projection;
    lxp_projection_record record;
    uint64_t watermark;
    bool has_watermark;
    sqlite3_stmt *statement = NULL;
    if (descriptor < 0 || close(descriptor) != 0 || unlink(path) != 0)
        return 1;
    (void)memset(&record, 0, sizeof(record));
    record.activity_id[0] = 1U;
    record.idempotency_key[0] = 2U;
    record.account_id[0] = 3U;
    record.asset_id[0] = 4U;
    record.amount[15] = 9U;
    record.receipt = receipt;
    record.receipt_length = sizeof(receipt);
    if (lxp_projection_open(&projection, path,
                            "migrations/0001_projection.sql") != LXP_OK ||
        lxp_projection_apply(&projection, 7U, &record) != LXP_OK ||
        lxp_projection_watermark(&projection, &watermark, &has_watermark) !=
            LXP_OK || !has_watermark || watermark != 7U) return 1;
    if (sqlite3_prepare_v2((sqlite3 *)projection.database,
            "SELECT amount FROM balance_view", -1, &statement, NULL) != SQLITE_OK ||
        sqlite3_step(statement) != SQLITE_ROW ||
        sqlite3_column_bytes(statement, 0) != 16 ||
        ((const uint8_t *)sqlite3_column_blob(statement, 0))[15] != 9U)
        return 1;
    (void)sqlite3_finalize(statement);
    if (lxp_projection_apply(&projection, 8U, NULL) !=
        LXP_ERR_PROJECTION_STALE || !projection.stale) return 1;
    if (lxp_projection_watermark(&projection, &watermark, &has_watermark) !=
        LXP_OK || watermark != 7U) return 1;
    if (lxp_projection_close(&projection) != LXP_OK || unlink(path) != 0)
        return 1;
    return 0;
}
