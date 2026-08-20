#include "layerx/program.h"

/*
 * Byte-for-byte the frozen manifest the runtime publishes as
 * layerx_programs_runtime::ABI_MANIFEST. Every authoring language ships the
 * same bytes, which is what makes cross-language identity checkable rather
 * than asserted.
 */
static const char abi_manifest[] =
    "layerx_v1\0"
    "storage_read(i32,i32,i32,i32)->i32\0"
    "storage_write(i32,i32,i32,i32)->i32\0"
    "storage_delete(i32,i32)->i32\0"
    "event_emit(i32,i32,i32,i32)->i32\0"
    "program_call(i32,i32,i32,i32,i32,i32)->i32\0"
    "transfer_402(i64,i64,i32,i32,i32,i32)->i32\0"
    "receipt_read(i32,i32,i32,i32)->i32\0";

const uint8_t *lxp_program_abi_manifest(size_t *length)
{
    if (length != NULL) *length = sizeof(abi_manifest) - 1U;
    return (const uint8_t *)abi_manifest;
}

const char *lxp_program_status_name(lxp_program_status status)
{
    switch (status) {
#define LXP_PROGRAM_STATUS_NAME(name, value) \
    case (value): return #name;
        LXP_PROGRAM_STATUS_LIST(LXP_PROGRAM_STATUS_NAME)
#undef LXP_PROGRAM_STATUS_NAME
    default: return "LXP_PROGRAM_ERR_UNKNOWN";
    }
}

/*
 * The mapping is the one every authoring language applies: a bound, a capacity
 * or an exact-integer overflow becomes LXP_PROGRAM_ERR_BOUNDS, and a reserved
 * value, a duplicate authority or a non-canonical encoding becomes
 * LXP_PROGRAM_ERR_INVALID.
 */
lxp_program_status lxp_program_status_abi(lxp_program_status status)
{
    switch (status) {
    case LXP_PROGRAM_OK:
    case LXP_PROGRAM_ERR_DENIED:
    case LXP_PROGRAM_ERR_INVALID:
    case LXP_PROGRAM_ERR_BOUNDS:
    case LXP_PROGRAM_ERR_METER:
    case LXP_PROGRAM_ERR_EVIDENCE:
        return status;
    case LXP_PROGRAM_ERR_EMPTY_KEY:
    case LXP_PROGRAM_ERR_KEY_TOO_LARGE:
    case LXP_PROGRAM_ERR_VALUE_TOO_LARGE:
    case LXP_PROGRAM_ERR_EMPTY_TOPIC:
    case LXP_PROGRAM_ERR_TOPIC_TOO_LARGE:
    case LXP_PROGRAM_ERR_DATA_TOO_LARGE:
    case LXP_PROGRAM_ERR_INPUT_TOO_LARGE:
    case LXP_PROGRAM_ERR_CAPABILITY_LIMIT:
    case LXP_PROGRAM_ERR_CAPABILITY_BYTES:
    case LXP_PROGRAM_ERR_BUFFER_TOO_SMALL:
    case LXP_PROGRAM_ERR_OVERFLOW:
    case LXP_PROGRAM_ERR_UNDERFLOW:
        return LXP_PROGRAM_ERR_BOUNDS;
    case LXP_PROGRAM_ERR_NULL_ARGUMENT:
    case LXP_PROGRAM_ERR_ZERO_AMOUNT:
    case LXP_PROGRAM_ERR_RESERVED_IDENTIFIER:
    case LXP_PROGRAM_ERR_DUPLICATE_CAPABILITY:
    case LXP_PROGRAM_ERR_RECEIPT_ENCODING:
        return LXP_PROGRAM_ERR_INVALID;
    default:
        return LXP_PROGRAM_ERR_INVALID;
    }
}
