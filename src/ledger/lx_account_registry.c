#include "layerx/lxp_ledger.h"

#include <string.h>

static bool system_kind(lx_account_kind kind)
{
    return kind >= LX_ACCOUNT_SYSTEM_LIQUIDITY;
}

static lxp_result append_creation(lxp_log *log, const lx_account *account)
{
    uint8_t body[1U + 8U + 2U + LX_ACCOUNT_NAME_MAX + 32U];
    size_t cursor = 0U;
    uint64_t sequence = account->created_at_sequence;
    size_t i;
    if (log == NULL) return LXP_OK;
    body[cursor++] = UINT8_C(0xa1);
    for (i = 0U; i < 8U; ++i)
        body[cursor++] = (uint8_t)(sequence >> ((7U - i) * 8U));
    body[cursor++] = (uint8_t)(account->name_length >> 8U);
    body[cursor++] = (uint8_t)account->name_length;
    (void)memcpy(body + cursor, account->name, account->name_length);
    cursor += account->name_length;
    (void)memcpy(body + cursor, account->id, sizeof(account->id));
    cursor += sizeof(account->id);
    return lxp_log_append(log, LXP_LOG_ACTIVITY, sequence, body,
                          (uint32_t)cursor, NULL);
}

lxp_result lx_account_registry_init(lx_account_registry *registry)
{
    if (registry == NULL) return LXP_ERR_NON_CANONICAL;
    (void)memset(registry, 0, sizeof(*registry));
    return LXP_OK;
}

lxp_result lx_account_lookup(lx_account_registry *registry,
                             const uint8_t *name, size_t name_length,
                             const uint8_t presented_id[32],
                             lx_account **account)
{
    uint8_t derived[32];
    size_t i;
    lxp_result status;
    if (registry == NULL || presented_id == NULL || account == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = lx_account_id_from_string(name, name_length, derived);
    if (status != LXP_OK) return status;
    if (memcmp(derived, presented_id, sizeof(derived)) != 0)
        return LXP_ERR_ACCOUNT_ID_MISMATCH;
    for (i = 0U; i < registry->count; ++i) {
        if (memcmp(registry->accounts[i].id, derived, sizeof(derived)) == 0) {
            if ((size_t)registry->accounts[i].name_length != name_length ||
                memcmp(registry->accounts[i].name, name, name_length) != 0)
                return LXP_ERR_ACCOUNT_ID_MISMATCH;
            *account = &registry->accounts[i];
            return LXP_OK;
        }
    }
    return LXP_ERR_UNKNOWN_ACCOUNT_NAMESPACE;
}

lxp_result lx_account_open(lx_account_registry *registry,
                           const uint8_t *name, size_t name_length,
                           const uint8_t presented_id[32],
                           uint64_t global_sequence,
                           lx_account_open_authority authority,
                           lxp_log *activity_log, lx_account **account)
{
    uint8_t derived[32];
    lx_account_name parsed;
    lx_account *created;
    size_t i;
    lxp_result status;
    if (registry == NULL || presented_id == NULL || account == NULL)
        return LXP_ERR_NON_CANONICAL;
    status = lx_account_name_parse(name, name_length, &parsed);
    if (status != LXP_OK) return status;
    status = lx_account_id_from_string(name, name_length, derived);
    if (status != LXP_OK) return status;
    if (memcmp(derived, presented_id, sizeof(derived)) != 0)
        return LXP_ERR_ACCOUNT_ID_MISMATCH;
    for (i = 0U; i < registry->count; ++i) {
        if (memcmp(registry->accounts[i].id, derived, sizeof(derived)) == 0) {
            if ((size_t)registry->accounts[i].name_length != name_length ||
                memcmp(registry->accounts[i].name, name, name_length) != 0)
                return LXP_ERR_ACCOUNT_ID_MISMATCH;
            *account = &registry->accounts[i];
            return LXP_OK;
        }
    }
    if (system_kind(parsed.kind) && authority == LX_ACCOUNT_OPEN_CREDIT)
        return LXP_ERR_UNAUTHORIZED_DEBIT;
    if (registry->count == LX_ACCOUNT_REGISTRY_CAPACITY)
        return LXP_ERR_ARENA_EXHAUSTED;
    created = &registry->accounts[registry->count];
    (void)memset(created, 0, sizeof(*created));
    (void)memcpy(created->id, derived, sizeof(created->id));
    (void)memcpy(created->name, name, name_length);
    created->name_length = (uint16_t)name_length;
    created->kind = parsed.kind;
    created->created_at_sequence = global_sequence;
    status = append_creation(activity_log, created);
    if (status != LXP_OK) return status;
    ++registry->count;
    *account = created;
    return LXP_OK;
}

lxp_result lx_account_close(lx_account_registry *registry,
                            const uint8_t account_id[32])
{
    size_t i;
    if (registry == NULL || account_id == NULL) return LXP_ERR_NON_CANONICAL;
    for (i = 0U; i < registry->count; ++i) {
        if (memcmp(registry->accounts[i].id, account_id, 32U) == 0) {
            if (!lxp_u128_is_zero(registry->accounts[i].balance) ||
                registry->accounts[i].has_open_reference)
                return LXP_ERR_ACCOUNT_NOT_EMPTY;
            if (i + 1U < registry->count)
                (void)memmove(&registry->accounts[i], &registry->accounts[i + 1U],
                              (registry->count - i - 1U) * sizeof(lx_account));
            --registry->count;
            return LXP_OK;
        }
    }
    return LXP_ERR_UNKNOWN_ACCOUNT_NAMESPACE;
}
