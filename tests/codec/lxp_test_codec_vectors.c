#include "layerx/lxp_codec.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct lxp_codec_vector_case {
    char kind[16];
    char name[64];
    uint8_t bytes[256];
    size_t length;
    lxp_result expected;
    char digest[65];
} lxp_codec_vector_case;
#define lxp_codec_vector_case lxp_codec_vector_case

static int hex_value(char c)
{
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

int lxp_codec_vector_load(const char *path, lxp_codec_vector_case *cases,
                          size_t capacity, size_t *count)
{
    FILE *file;
    char line[1024];
    size_t used = 0U;
    if (path == NULL || cases == NULL || count == NULL) return 1;
    file = fopen(path, "rb");
    if (file == NULL) return 1;
    while (fgets(line, sizeof(line), file) != NULL) {
        char *fields[5];
        char *cursor;
        size_t field = 0U;
        size_t hex_length;
        size_t i;
        if (line[0] == '#' || line[0] == '\n') continue;
        cursor = strtok(line, "|\r\n");
        while (cursor != NULL && field < 5U) {
            fields[field++] = cursor;
            cursor = strtok(NULL, "|\r\n");
        }
        if (field != 5U || used >= capacity || fields[3][0] == '\0') {
            (void)fclose(file);
            return 1;
        }
        hex_length = strlen(fields[2]);
        if ((hex_length & 1U) != 0U || hex_length / 2U > sizeof(cases[used].bytes)) {
            (void)fclose(file);
            return 1;
        }
        (void)snprintf(cases[used].kind, sizeof(cases[used].kind), "%s", fields[0]);
        (void)snprintf(cases[used].name, sizeof(cases[used].name), "%s", fields[1]);
        cases[used].length = hex_length / 2U;
        for (i = 0U; i < cases[used].length; ++i) {
            int high = hex_value(fields[2][i * 2U]);
            int low = hex_value(fields[2][i * 2U + 1U]);
            if (high < 0 || low < 0) { (void)fclose(file); return 1; }
            cases[used].bytes[i] = (uint8_t)((unsigned)high * 16U + (unsigned)low);
        }
        cases[used].expected = (lxp_result)strtol(fields[3], NULL, 10);
        (void)snprintf(cases[used].digest, sizeof(cases[used].digest), "%s", fields[4]);
        if (cases[used].expected == LXP_OK && strlen(cases[used].digest) != 64U) {
            (void)fclose(file);
            return 1;
        }
        ++used;
    }
    if (ferror(file) != 0 || fclose(file) != 0 || used == 0U) return 1;
    *count = used;
    return 0;
}

static lxp_result run_case(const lxp_codec_vector_case *vector)
{
    lxp_codec_reader reader;
    if (lxp_codec_reader_init(&reader, vector->bytes, vector->length) != LXP_OK)
        return LXP_ERR_NON_CANONICAL;
    if (strcmp(vector->kind, "u64") == 0) {
        uint64_t value;
        lxp_result result = lxp_codec_read_u64(&reader, &value);
        if (result != LXP_OK) return result;
        return lxp_codec_finish(&reader);
    }
    if (strcmp(vector->kind, "tag") == 0) {
        uint8_t tag;
        return lxp_codec_read_tag(&reader, 3U, &tag);
    }
    if (strcmp(vector->kind, "bytes4") == 0) {
        lxp_byte_span span;
        return lxp_codec_read_bytes(&reader, &span, 4U);
    }
    if (strcmp(vector->kind, "seq") == 0) {
        lxp_byte_span keys[2];
        size_t offset = 0U;
        size_t i;
        for (i = 0U; i < 2U; ++i) {
            size_t length;
            if (offset >= vector->length) return LXP_ERR_TRUNCATED;
            length = vector->bytes[offset++];
            if (length > vector->length - offset) return LXP_ERR_TRUNCATED;
            keys[i].bytes = vector->bytes + offset;
            keys[i].length = length;
            offset += length;
        }
        return lxp_codec_seq_check_sorted(keys, 2U);
    }
    return LXP_ERR_INVALID_TAG;
}

int lxp_codec_vector_run(void)
{
    lxp_codec_vector_case cases[32];
    const char *paths[] = {"tests/vectors/codec/valid.lxv",
                           "tests/vectors/codec/adversarial.lxv"};
    size_t path_index;
    for (path_index = 0U; path_index < 2U; ++path_index) {
        size_t count = 0U;
        size_t i;
        if (lxp_codec_vector_load(paths[path_index], cases, 32U, &count) != 0)
            return 1;
        for (i = 0U; i < count; ++i) {
            lxp_result produced = run_case(&cases[i]);
            if (produced != cases[i].expected) return 1;
            if (produced == LXP_OK) {
                uint8_t storage[8];
                lxp_arena arena;
                lxp_codec_writer writer;
                lxp_codec_reader reader;
                uint64_t value;
                if (lxp_codec_reader_init(&reader, cases[i].bytes,
                                          cases[i].length) != LXP_OK ||
                    lxp_codec_read_u64(&reader, &value) != LXP_OK ||
                    lxp_arena_init(&arena, storage, sizeof(storage)) != LXP_OK ||
                    lxp_codec_writer_init(&writer, &arena, sizeof(storage)) != LXP_OK ||
                    lxp_codec_write_u64(&writer, value) != LXP_OK ||
                    writer.length != cases[i].length ||
                    memcmp(writer.bytes, cases[i].bytes, writer.length) != 0)
                    return 1;
            }
        }
    }
    return 0;
}

int main(void)
{
    return lxp_codec_vector_run();
}
