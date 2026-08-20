#include "layerx/program.h"

#include "host.h"
#include "internal.h"

/*
 * The only monetary primitive a program can reach. It never moves value
 * itself: it records an authenticated 402LXP request that the kernel applies
 * inside the invoking activity's authority, or refuses whole.
 */

lxp_program_status lxp_program_transfer_402(lxp_program_asset asset,
                                            lxp_program_account to,
                                            lxp_program_amount amount)
{
    if (lxp_program_amount_is_zero(amount)) return LXP_PROGRAM_ERR_ZERO_AMOUNT;
    if (lxp_program_bytes32_is_zero(asset.bytes) ||
        lxp_program_bytes32_is_zero(to.bytes))
        return LXP_PROGRAM_ERR_RESERVED_IDENTIFIER;
    return lxp_program_host_transfer_402(
        (int64_t)amount.hi, (int64_t)amount.lo,
        lxp_program_pointer(asset.bytes),
        lxp_program_length((size_t)LXP_PROGRAM_ID_BYTES),
        lxp_program_pointer(to.bytes),
        lxp_program_length((size_t)LXP_PROGRAM_ID_BYTES));
}
