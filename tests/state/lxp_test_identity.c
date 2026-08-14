#include "layerx/lxp_identity.h"

#include <stdint.h>
#include <string.h>

int main(void)
{
    static const uint8_t did[] = "did:lxp:alice";
    static const uint8_t main_name[] = "agent:did:lxp:alice:main";
    static const uint8_t budget_name[] = "agent:did:lxp:alice:budget:ops";
    static const uint8_t invalid_name[] = "agent:did:lxp:alice:savings:x";
    uint8_t primary_key[32] = { 1U };
    uint8_t first_did[32];
    uint8_t second_did[32];
    uint8_t main_account[32];
    uint8_t repeated_account[32];
    lxp_identity_store store = { 0 };
    lxp_identity *identity;
    if (lxp_did_id_derive(did, sizeof(did) - 1U, first_did) != LXP_OK ||
        lxp_did_id_derive(did, sizeof(did) - 1U, second_did) != LXP_OK ||
        memcmp(first_did, second_did, 32U) != 0) return 1;
    if (lxp_account_id_derive(main_name, sizeof(main_name) - 1U,
                              main_account) != LXP_OK ||
        lxp_account_id_derive(main_name, sizeof(main_name) - 1U,
                              repeated_account) != LXP_OK ||
        memcmp(main_account, repeated_account, 32U) != 0 ||
        lxp_account_id_derive(budget_name, sizeof(budget_name) - 1U,
                              repeated_account) != LXP_OK ||
        lxp_account_id_derive(invalid_name, sizeof(invalid_name) - 1U,
                              repeated_account) !=
            LXP_ERR_UNKNOWN_ACCOUNT_NAMESPACE) return 1;
    if (lxp_identity_register(&store, did, sizeof(did) - 1U, primary_key,
                              &identity) != LXP_OK ||
        lxp_identity_consume_sequence(identity, 1U) != LXP_ERR_SEQUENCE_GAP ||
        lxp_identity_consume_sequence(identity, 0U) != LXP_OK ||
        lxp_identity_consume_sequence(identity, 0U) != LXP_ERR_SEQUENCE_REUSED ||
        identity->next_sequence != 1U) return 1;
    identity->status = LXP_IDENTITY_FROZEN;
    if (lxp_identity_resolve(&store, did, sizeof(did) - 1U, &identity) !=
        LXP_ERR_IDENTITY_FROZEN) return 1;
    return 0;
}
