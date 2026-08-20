#include "layerx/program.h"

#include "host.h"
#include "internal.h"

/*
 * Receipt facts are read through the core's verification authority. The guest
 * never sees raw kernel state and cannot name a digest it was not granted.
 */

lxp_program_status lxp_program_receipt_read(lxp_program_digest receipt_digest,
                                            lxp_program_receipt *out)
{
    uint8_t encoded[LXP_PROGRAM_RECEIPT_BYTES];
    int32_t outcome;
    if (out == NULL) return LXP_PROGRAM_ERR_NULL_ARGUMENT;
    if (lxp_program_bytes32_is_zero(receipt_digest.bytes))
        return LXP_PROGRAM_ERR_RESERVED_IDENTIFIER;
    outcome = lxp_program_host_receipt_read(
        lxp_program_pointer(receipt_digest.bytes),
        lxp_program_length((size_t)LXP_PROGRAM_DIGEST_BYTES),
        lxp_program_pointer(encoded),
        lxp_program_length(sizeof(encoded)));
    if (outcome < 0) return outcome;
    if (outcome != LXP_PROGRAM_RECEIPT_BYTES)
        return LXP_PROGRAM_ERR_RECEIPT_ENCODING;
    lxp_program_copy(out->digest.bytes, encoded,
                     (size_t)LXP_PROGRAM_DIGEST_BYTES);
    out->result_code = lxp_program_read_i32_be(encoded + 32);
    lxp_program_copy(out->asset.bytes, encoded + 36,
                     (size_t)LXP_PROGRAM_ID_BYTES);
    out->amount = lxp_program_amount_from_be(encoded + 68);
    lxp_program_copy(out->state_root, encoded + 84, 32U);
    if (!lxp_program_bytes_equal(out->digest.bytes, receipt_digest.bytes,
                                 (size_t)LXP_PROGRAM_DIGEST_BYTES))
        return LXP_PROGRAM_ERR_EVIDENCE;
    return LXP_PROGRAM_OK;
}
