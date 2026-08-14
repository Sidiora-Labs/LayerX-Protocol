#include "layerx/lxp_projection.h"
#include "layerx/lxp_fault.h"

#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static sqlite3 *database(lxp_projection *projection)
{
    return (sqlite3 *)projection->database;
}

static int owner(const lxp_projection *projection)
{
    return pthread_equal(projection->owner, pthread_self()) != 0;
}

static char *read_file(const char *path)
{
    FILE *file;
    long length;
    char *contents;
    if (path == NULL) return NULL;
    file = fopen(path, "rb");
    if (file == NULL || fseek(file, 0L, SEEK_END) != 0) {
        if (file != NULL) (void)fclose(file);
        return NULL;
    }
    length = ftell(file);
    if (length < 0 || fseek(file, 0L, SEEK_SET) != 0) {
        (void)fclose(file);
        return NULL;
    }
    contents = malloc((size_t)length + 1U);
    if (contents == NULL || fread(contents, 1U, (size_t)length, file) !=
        (size_t)length) {
        free(contents);
        (void)fclose(file);
        return NULL;
    }
    contents[length] = '\0';
    (void)fclose(file);
    return contents;
}

void lxp_projection_mark_stale(lxp_projection *projection)
{
    if (projection == NULL) return;
    projection->stale = true;
    if (projection->database != NULL && owner(projection))
        (void)sqlite3_exec(database(projection),
            "UPDATE projection_meta SET stale=1 WHERE singleton=1", NULL,
            NULL, NULL);
}

lxp_result lxp_projection_open(lxp_projection *projection,
                               const char *database_path,
                               const char *migration_path)
{
    sqlite3 *handle = NULL;
    char *migration;
    char *error = NULL;
    if (projection == NULL || database_path == NULL) return LXP_ERR_NON_CANONICAL;
    (void)memset(projection, 0, sizeof(*projection));
    if (sqlite3_open_v2(database_path, &handle,
                        SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE |
                        SQLITE_OPEN_NOMUTEX, NULL) != SQLITE_OK) {
        if (handle != NULL) (void)sqlite3_close(handle);
        return LXP_ERR_IO;
    }
    projection->database = handle;
    projection->owner = pthread_self();
    if (sqlite3_exec(handle, "PRAGMA journal_mode=WAL", NULL, NULL, &error) !=
        SQLITE_OK) {
        sqlite3_free(error);
        (void)sqlite3_close(handle);
        projection->database = NULL;
        return LXP_ERR_IO;
    }
    migration = read_file(migration_path);
    if (migration == NULL || sqlite3_exec(handle, migration, NULL, NULL,
                                           &error) != SQLITE_OK) {
        free(migration);
        sqlite3_free(error);
        (void)sqlite3_close(handle);
        projection->database = NULL;
        return LXP_ERR_IO;
    }
    free(migration);
    return LXP_OK;
}

static int bind_record(sqlite3_stmt *statement, uint64_t sequence,
                       const lxp_projection_record *record)
{
    if (sequence > INT64_MAX) return SQLITE_RANGE;
    if (sqlite3_bind_blob(statement, 1, record->activity_id, 32,
                          SQLITE_STATIC) != SQLITE_OK ||
        sqlite3_bind_blob(statement, 2, record->idempotency_key, 32,
                          SQLITE_STATIC) != SQLITE_OK ||
        sqlite3_bind_int64(statement, 3, (sqlite3_int64)sequence) != SQLITE_OK ||
        sqlite3_bind_int(statement, 4, record->result_code) != SQLITE_OK ||
        sqlite3_bind_blob(statement, 5, record->receipt,
                          (int)record->receipt_length, SQLITE_STATIC) != SQLITE_OK)
        return SQLITE_ERROR;
    return SQLITE_OK;
}

lxp_result lxp_projection_apply(lxp_projection *projection,
                                uint64_t global_sequence,
                                const lxp_projection_record *record)
{
    sqlite3 *handle;
    sqlite3_stmt *receipt = NULL;
    sqlite3_stmt *balance = NULL;
    sqlite3_stmt *watermark = NULL;
    int status = SQLITE_ERROR;
    if (projection == NULL || projection->database == NULL)
        return LXP_ERR_NON_CANONICAL;
    if (!owner(projection)) return LXP_FATAL_INVARIANT;
    if (record == NULL || (record->receipt == NULL && record->receipt_length != 0U) ||
        record->receipt_length > INT32_MAX || global_sequence > INT64_MAX) {
        lxp_projection_mark_stale(projection);
        return LXP_ERR_PROJECTION_STALE;
    }
    handle = database(projection);
    if (sqlite3_exec(handle, "BEGIN IMMEDIATE", NULL, NULL, NULL) != SQLITE_OK)
        goto finish;
    if (sqlite3_prepare_v2(handle,
            "INSERT INTO receipts VALUES(?,?,?,?,?)", -1, &receipt, NULL) !=
            SQLITE_OK || bind_record(receipt, global_sequence, record) !=
            SQLITE_OK || sqlite3_step(receipt) != SQLITE_DONE)
        goto finish;
    lxp_fault_inject_point(LXP_FAULT_INDEX_RECEIPT_WRITTEN);
    if (sqlite3_prepare_v2(handle,
            "INSERT INTO balances VALUES(?,?,?) "
            "ON CONFLICT(account_id,asset_id) DO UPDATE SET amount=excluded.amount",
            -1, &balance, NULL) != SQLITE_OK ||
        sqlite3_bind_blob(balance, 1, record->account_id, 32, SQLITE_STATIC) !=
            SQLITE_OK ||
        sqlite3_bind_blob(balance, 2, record->asset_id, 32, SQLITE_STATIC) !=
            SQLITE_OK ||
        sqlite3_bind_blob(balance, 3, record->amount, 16, SQLITE_STATIC) !=
            SQLITE_OK || sqlite3_step(balance) != SQLITE_DONE)
        goto finish;
    lxp_fault_inject_point(LXP_FAULT_INDEX_BALANCE_WRITTEN);
    if (sqlite3_prepare_v2(handle,
            "UPDATE projection_meta SET watermark=?, stale=0 WHERE singleton=1",
            -1, &watermark, NULL) != SQLITE_OK ||
        sqlite3_bind_int64(watermark, 1, (sqlite3_int64)global_sequence) !=
            SQLITE_OK || sqlite3_step(watermark) != SQLITE_DONE)
        goto finish;
    lxp_fault_inject_point(LXP_FAULT_INDEX_WATERMARK_WRITTEN);
    if (sqlite3_exec(handle, "COMMIT", NULL, NULL, NULL) != SQLITE_OK)
        goto finish;
    lxp_fault_inject_point(LXP_FAULT_INDEX_COMMITTED);
    status = SQLITE_OK;
finish:
    (void)sqlite3_finalize(receipt);
    (void)sqlite3_finalize(balance);
    (void)sqlite3_finalize(watermark);
    if (status != SQLITE_OK) {
        (void)sqlite3_exec(handle, "ROLLBACK", NULL, NULL, NULL);
        lxp_projection_mark_stale(projection);
        return LXP_ERR_PROJECTION_STALE;
    }
    projection->stale = false;
    return LXP_OK;
}

lxp_result lxp_projection_watermark(lxp_projection *projection,
                                    uint64_t *watermark, bool *has_watermark)
{
    sqlite3_stmt *statement = NULL;
    int64_t value;
    if (projection == NULL || watermark == NULL || has_watermark == NULL ||
        projection->database == NULL) return LXP_ERR_NON_CANONICAL;
    if (!owner(projection)) return LXP_FATAL_INVARIANT;
    if (sqlite3_prepare_v2(database(projection),
        "SELECT watermark FROM projection_meta WHERE singleton=1", -1,
        &statement, NULL) != SQLITE_OK || sqlite3_step(statement) != SQLITE_ROW) {
        (void)sqlite3_finalize(statement);
        lxp_projection_mark_stale(projection);
        return LXP_ERR_PROJECTION_STALE;
    }
    value = sqlite3_column_int64(statement, 0);
    (void)sqlite3_finalize(statement);
    *has_watermark = value >= 0;
    *watermark = value >= 0 ? (uint64_t)value : 0U;
    return LXP_OK;
}

lxp_result lxp_projection_close(lxp_projection *projection)
{
    if (projection == NULL || projection->database == NULL || !owner(projection))
        return LXP_ERR_NON_CANONICAL;
    if (sqlite3_close(database(projection)) != SQLITE_OK) return LXP_ERR_IO;
    projection->database = NULL;
    return LXP_OK;
}

static void write_u32(uint8_t *output, uint32_t value)
{
    output[0] = (uint8_t)(value >> 24U);
    output[1] = (uint8_t)(value >> 16U);
    output[2] = (uint8_t)(value >> 8U);
    output[3] = (uint8_t)value;
}

static uint32_t read_u32(const uint8_t *input)
{
    return ((uint32_t)input[0] << 24U) | ((uint32_t)input[1] << 16U) |
           ((uint32_t)input[2] << 8U) | input[3];
}

lxp_result lxp_projection_record_encode(const lxp_projection_record *record,
                                        uint8_t *output, size_t capacity,
                                        size_t *encoded_length)
{
    size_t fixed = 153U;
    size_t total;
    if (record == NULL || output == NULL || encoded_length == NULL ||
        (record->receipt == NULL && record->receipt_length != 0U) ||
        record->receipt_length > UINT32_MAX) return LXP_ERR_NON_CANONICAL;
    total = fixed + record->receipt_length;
    if (total < fixed || total > capacity) return LXP_ERR_LENGTH_LIMIT;
    output[0] = 1U;
    (void)memcpy(output + 1U, record->activity_id, 32U);
    (void)memcpy(output + 33U, record->idempotency_key, 32U);
    (void)memcpy(output + 65U, record->account_id, 32U);
    (void)memcpy(output + 97U, record->asset_id, 32U);
    (void)memcpy(output + 129U, record->amount, 16U);
    write_u32(output + 145U, (uint32_t)record->result_code);
    write_u32(output + 149U, (uint32_t)record->receipt_length);
    if (record->receipt_length != 0U)
        (void)memcpy(output + fixed, record->receipt, record->receipt_length);
    *encoded_length = total;
    return LXP_OK;
}

lxp_result lxp_projection_record_decode(const uint8_t *input, size_t length,
                                        lxp_projection_record *record)
{
    uint32_t receipt_length;
    if (input == NULL || record == NULL || length < 153U || input[0] != 1U)
        return LXP_ERR_NON_CANONICAL;
    receipt_length = read_u32(input + 149U);
    if ((size_t)receipt_length != length - 153U) return LXP_ERR_TRAILING_BYTES;
    (void)memcpy(record->activity_id, input + 1U, 32U);
    (void)memcpy(record->idempotency_key, input + 33U, 32U);
    (void)memcpy(record->account_id, input + 65U, 32U);
    (void)memcpy(record->asset_id, input + 97U, 32U);
    (void)memcpy(record->amount, input + 129U, 16U);
    record->result_code = (int32_t)read_u32(input + 145U);
    record->receipt = input + 153U;
    record->receipt_length = receipt_length;
    return LXP_OK;
}

lxp_result lxp_log_replay_range(const lxp_log *log, uint64_t start_offset,
                                uint64_t end_offset, lxp_log_replay_fn replay,
                                void *context)
{
    uint64_t offset = start_offset;
    if (log == NULL || replay == NULL || start_offset > end_offset ||
        end_offset > log->capacity) return LXP_ERR_NON_CANONICAL;
    while (offset < end_offset) {
        lxp_log_record_header header;
        uint8_t *body = NULL;
        lxp_result status = lxp_log_read(log, offset, &header, NULL, 0U);
        if (status != LXP_OK && status != LXP_ERR_LENGTH_LIMIT) return status;
        if (header.body_length != 0U) {
            body = malloc(header.body_length);
            if (body == NULL) return LXP_ERR_IO;
            status = lxp_log_read(log, offset, &header, body,
                                  header.body_length);
            if (status != LXP_OK) {
                free(body);
                return status;
            }
        }
        status = replay(context, &header, body);
        free(body);
        if (status != LXP_OK) return status;
        offset += LXP_LOG_HEADER_BYTES + header.body_length;
    }
    return offset == end_offset ? LXP_OK : LXP_ERR_LOG_TRUNCATED;
}

lxp_result lxp_projection_drop_all(lxp_projection *projection,
                                   const char *migration_path)
{
    char *migration;
    char *error = NULL;
    sqlite3 *handle;
    static const char drop_sql[] =
        "BEGIN IMMEDIATE;"
        "DROP VIEW IF EXISTS balance_view;"
        "DROP TABLE IF EXISTS balances;"
        "DROP TABLE IF EXISTS receipts;"
        "DROP TABLE IF EXISTS module_index;"
        "DROP TABLE IF EXISTS agent_queries;"
        "DROP TABLE IF EXISTS projection_meta;"
        "COMMIT;";
    if (projection == NULL || projection->database == NULL || !owner(projection))
        return LXP_FATAL_INVARIANT;
    handle = database(projection);
    if (sqlite3_exec(handle, drop_sql, NULL, NULL, &error) != SQLITE_OK) {
        sqlite3_free(error);
        lxp_projection_mark_stale(projection);
        return LXP_ERR_PROJECTION_STALE;
    }
    migration = read_file(migration_path);
    if (migration == NULL || sqlite3_exec(handle, migration, NULL, NULL,
                                           &error) != SQLITE_OK) {
        free(migration);
        sqlite3_free(error);
        lxp_projection_mark_stale(projection);
        return LXP_ERR_PROJECTION_STALE;
    }
    free(migration);
    projection->stale = false;
    return LXP_OK;
}

static lxp_result rebuild_record(void *context,
                                 const lxp_log_record_header *header,
                                 const uint8_t *body)
{
    lxp_projection *projection = (lxp_projection *)context;
    lxp_projection_record record;
    lxp_result status;
    if (header->record_kind != (uint8_t)LXP_LOG_RECEIPT) return LXP_OK;
    status = lxp_projection_record_decode(body, header->body_length, &record);
    if (status != LXP_OK) return LXP_FATAL_REPLAY_DIVERGENCE;
    return lxp_projection_apply(projection, header->global_sequence, &record);
}

lxp_result lxp_projection_rebuild(lxp_projection *projection, lxp_log *log,
                                  const char *migration_path)
{
    uint64_t valid_end;
    uint64_t last_offset;
    uint64_t next_sequence;
    uint64_t durable;
    uint64_t replay_end = 0U;
    uint64_t offset = 0U;
    lxp_result status;
    if (projection == NULL || log == NULL) return LXP_ERR_NON_CANONICAL;
    status = lxp_log_scan_tail(log, &valid_end, &last_offset, &next_sequence);
    if (status != LXP_OK) return status;
    status = lxp_log_durable_head(log, &durable);
    if (status != LXP_OK) return status;
    while (offset < valid_end) {
        lxp_log_record_header header;
        status = lxp_log_read(log, offset, &header, NULL, 0U);
        if (status != LXP_OK && status != LXP_ERR_LENGTH_LIMIT) return status;
        if (durable != UINT64_MAX && header.global_sequence <= durable)
            replay_end = offset + LXP_LOG_HEADER_BYTES + header.body_length;
        offset += LXP_LOG_HEADER_BYTES + header.body_length;
    }
    status = lxp_projection_drop_all(projection, migration_path);
    if (status != LXP_OK) return status;
    if (replay_end == 0U) return LXP_OK;
    return lxp_log_replay_range(log, 0U, replay_end, rebuild_record, projection);
}
