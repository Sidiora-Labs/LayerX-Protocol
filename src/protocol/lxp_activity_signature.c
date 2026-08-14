#include "layerx/lxp_activity.h"

#include "layerx/lxp_crypto.h"

lxp_result lxp_activity_verify_signature(const lxp_activity *activity)
{
    uint8_t preimage[32];
    lxp_result status;
    if (activity == NULL || activity->authority.length != 32U ||
        activity->authority.bytes == NULL || activity->signature.length != 64U ||
        activity->signature.bytes == NULL) return LXP_ERR_BAD_SIGNATURE;
    status = lxp_activity_signing_preimage(activity, preimage);
    if (status == LXP_OK)
        status = lxp_ed25519_verify_raw(activity->authority.bytes,
                                        activity->signature.bytes, preimage,
                                        sizeof(preimage));
    lxp_secure_zero(preimage, sizeof(preimage));
    return status == LXP_OK ? LXP_OK : LXP_ERR_BAD_SIGNATURE;
}
