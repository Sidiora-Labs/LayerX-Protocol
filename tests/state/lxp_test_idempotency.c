#include "layerx/lxp_state.h"

#include <stdint.h>
#include <string.h>

int main(void)
{
    static const uint8_t actor[] = "did:lxp:alice";
    static const uint8_t first_receipt[] = { 1U, 2U, 3U, 4U };
    uint8_t idempotency_key[32] = { 9U };
    uint8_t balance_key[32] = { 5U };
    lxp_state_store store;
    lxp_state_journal journal;
    const uint8_t *replayed_receipt;
    size_t replayed_length;
    lxp_u128 balance;
    bool found;
    if (lxp_state_store_init(&store, 0U) != LXP_OK ||
        lxp_state_journal_open(&store, 0U, &journal) != LXP_OK ||
        lxp_state_journal_set(&journal, balance_key,
                              (lxp_u128){ 0U, 90U }) != LXP_OK ||
        lxp_idempotency_record(&journal, actor, sizeof(actor) - 1U,
                               idempotency_key, first_receipt,
                               sizeof(first_receipt)) != LXP_OK ||
        lxp_state_journal_commit(&journal) != LXP_OK) return 1;
    if (lxp_idempotency_lookup(&store, actor, sizeof(actor) - 1U,
                               idempotency_key, &replayed_receipt,
                               &replayed_length) != LXP_ERR_IDEMPOTENT_REPLAY ||
        replayed_length != sizeof(first_receipt) ||
        memcmp(replayed_receipt, first_receipt, replayed_length) != 0)
        return 1;
    /* A changed second payload is short-circuited before opening a journal. */
    if (store.next_sequence != 1U || store.idempotency_count != 1U ||
        lxp_state_store_get(&store, balance_key, &balance, &found) != LXP_OK ||
        !found || balance.hi != 0U || balance.lo != 90U) return 1;
    if (lxp_state_store_destroy(&store) != LXP_OK) return 1;
    return 0;
}
