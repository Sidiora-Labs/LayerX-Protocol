use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use layerx_agent_api::identity::{
    ActivityType, AgentDid, Asset, CapabilityId, Counterparty, ExplicitSet, TenantId as ApiTenantId,
};
use layerx_agent_api::read::{AccountRef, ModuleRef};
use layerx_agent_api::subscription::{
    Cursor as ApiCursor, CursorAcknowledgement, DeliveryTarget, SubscriptionCreate,
    SubscriptionFilter, SubscriptionId, SubscriptionScope, SubscriptionTarget, TenantObject,
};
use layerx_agent_api::Sequence;
use layerx_agentd::events::subscription::{Cursor, Store as SubscriptionStore, SubscriptionError};
use layerx_agentd::store::{ObjectKind, Store, TenantId};
use layerx_types::result::ResultCode;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

fn test_directory(name: &str) -> PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "layerx-agentd-subscription-{name}-{}-{sequence}",
        std::process::id()
    ))
}

fn durable_tenant(name: &str) -> TenantId {
    match TenantId::new(name) {
        Ok(value) => value,
        Err(error) => panic!("durable tenant must be valid: {error}"),
    }
}

fn api_tenant(name: &str) -> ApiTenantId {
    match ApiTenantId::new(name) {
        Ok(value) => value,
        Err(error) => panic!("API tenant must be valid: {error:?}"),
    }
}

fn agent(name: &str) -> AgentDid {
    match AgentDid::new(name) {
        Ok(value) => value,
        Err(error) => panic!("agent must be valid: {error:?}"),
    }
}

fn capability(name: &str) -> CapabilityId {
    match CapabilityId::new(name) {
        Ok(value) => value,
        Err(error) => panic!("capability must be valid: {error:?}"),
    }
}

fn subscription_id(name: &str) -> SubscriptionId {
    match SubscriptionId::new(name) {
        Ok(value) => value,
        Err(error) => panic!("subscription identifier must be valid: {error:?}"),
    }
}

fn target(name: &str) -> DeliveryTarget {
    match DeliveryTarget::new(name) {
        Ok(value) => value,
        Err(error) => panic!("delivery target must be valid: {error:?}"),
    }
}

fn scope(tenant: &str, agent_name: &str, capability_name: &str) -> SubscriptionScope {
    SubscriptionScope {
        tenant: api_tenant(tenant),
        agent: agent(agent_name),
        capability: capability(capability_name),
    }
}

fn tenant_object<T>(tenant: &str, value: T) -> TenantObject<T> {
    TenantObject {
        tenant: api_tenant(tenant),
        value,
    }
}

fn text<T, E: std::fmt::Debug>(result: Result<T, E>, label: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{label} must be valid: {error:?}"),
    }
}

fn filter(tenant: &str, agent_name: &str) -> SubscriptionFilter {
    SubscriptionFilter {
        agents: ExplicitSet::allow(vec![tenant_object(tenant, agent(agent_name))]),
        accounts: ExplicitSet::allow(vec![tenant_object(
            tenant,
            text(AccountRef::new("account-7"), "account"),
        )]),
        activity_types: ExplicitSet::allow(vec![ActivityType(9)]),
        modules: ExplicitSet::allow(vec![tenant_object(
            tenant,
            text(ModuleRef::new("asset"), "module"),
        )]),
        assets: ExplicitSet::allow(vec![tenant_object(
            tenant,
            text(Asset::new("LXR"), "asset"),
        )]),
        counterparties: ExplicitSet::allow(vec![tenant_object(
            tenant,
            text(Counterparty::new("counterparty-4"), "counterparty"),
        )]),
        result_classes: ExplicitSet::allow(vec![ResultCode::from_raw(0)]),
    }
}

fn request(scope: SubscriptionScope, filter: SubscriptionFilter, start: u64) -> SubscriptionCreate {
    SubscriptionCreate {
        scope,
        filter,
        start: ApiCursor(Sequence(start)),
        delivery_target: target("uds://consumer-a"),
    }
}

fn subscription_target(scope: &SubscriptionScope, id: &SubscriptionId) -> SubscriptionTarget {
    SubscriptionTarget {
        scope: scope.clone(),
        subscription_id: id.clone(),
    }
}

#[test]
fn restart_resumes_from_acknowledged_cursor_and_remembers_deliveries() {
    let root = test_directory("restart");
    let tenant_id = durable_tenant("tenant-a");
    let subscription_scope = scope("tenant-a", "agent-a", "capability-a");
    let id = subscription_id("subscription-a");
    let target = subscription_target(&subscription_scope, &id);
    let durable = match Store::open(&root) {
        Ok(value) => value,
        Err(error) => panic!("store open failed: {error}"),
    };
    let mut subscriptions = match SubscriptionStore::open(durable, tenant_id.clone()) {
        Ok(value) => value,
        Err(error) => panic!("subscription store open failed: {error}"),
    };
    let created = match subscriptions.create(
        id.clone(),
        request(
            subscription_scope.clone(),
            filter("tenant-a", "agent-a"),
            10,
        ),
    ) {
        Ok(value) => value,
        Err(error) => panic!("subscription create failed: {error}"),
    };
    assert_eq!(created.filter, filter("tenant-a", "agent-a"));
    if let Err(error) = subscriptions.mark_delivered(&target, Cursor(11)) {
        panic!("cursor 11 delivery failed: {error}");
    }
    if let Err(error) = subscriptions.mark_delivered(&target, Cursor(12)) {
        panic!("cursor 12 delivery failed: {error}");
    }
    let acknowledged = CursorAcknowledgement {
        scope: subscription_scope.clone(),
        subscription_id: id.clone(),
        cursor: ApiCursor(Sequence(11)),
    };
    if let Err(error) = subscriptions.acknowledge(&acknowledged) {
        panic!("cursor acknowledgement failed: {error}");
    }
    drop(subscriptions);

    let durable = match Store::open(&root) {
        Ok(value) => value,
        Err(error) => panic!("store restart failed: {error}"),
    };
    let mut restarted = match SubscriptionStore::open(durable, tenant_id) {
        Ok(value) => value,
        Err(error) => panic!("subscription restart failed: {error}"),
    };
    assert!(matches!(restarted.resume_cursor(&target), Ok(Cursor(11))));
    assert_eq!(restarted.list(&subscription_scope).len(), 1);
    if let Err(error) = restarted.mark_delivered(&target, Cursor(12)) {
        panic!("unacknowledged cursor was not redeliverable after restart: {error}");
    }
    let delivered_before_restart = CursorAcknowledgement {
        scope: subscription_scope.clone(),
        subscription_id: id.clone(),
        cursor: ApiCursor(Sequence(12)),
    };
    if let Err(error) = restarted.acknowledge(&delivered_before_restart) {
        panic!("durable delivered cursor was forgotten: {error}");
    }
    let never_delivered = CursorAcknowledgement {
        scope: subscription_scope,
        subscription_id: id,
        cursor: ApiCursor(Sequence(13)),
    };
    assert!(matches!(
        restarted.acknowledge(&never_delivered),
        Err(SubscriptionError::CursorNeverDelivered { cursor: Cursor(13) })
    ));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn tenant_and_agent_scope_are_applied_before_any_filter() {
    let root = test_directory("scope");
    let tenant_id = durable_tenant("tenant-a");
    let subscription_scope = scope("tenant-a", "agent-a", "capability-a");
    let durable = match Store::open(&root) {
        Ok(value) => value,
        Err(error) => panic!("store open failed: {error}"),
    };
    let mut subscriptions = match SubscriptionStore::open(durable, tenant_id.clone()) {
        Ok(value) => value,
        Err(error) => panic!("subscription store open failed: {error}"),
    };

    let mut cross_tenant = filter("tenant-a", "agent-a");
    cross_tenant.accounts = ExplicitSet::allow(vec![tenant_object(
        "tenant-b",
        text(AccountRef::new("secret-account"), "account"),
    )]);
    assert!(matches!(
        subscriptions.create(
            subscription_id("cross-tenant"),
            request(subscription_scope.clone(), cross_tenant, 0),
        ),
        Err(SubscriptionError::InvalidFilter)
    ));

    let same_tenant_other_agent = filter("tenant-a", "agent-b");
    assert!(matches!(
        subscriptions.create(
            subscription_id("cross-agent"),
            request(subscription_scope.clone(), same_tenant_other_agent, 0),
        ),
        Err(SubscriptionError::InvalidFilter)
    ));
    assert!(subscriptions
        .into_durable()
        .list_object_ids(&tenant_id, ObjectKind::Subscription)
        .is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pause_resume_delete_and_cross_scope_non_disclosure_are_durable() {
    let root = test_directory("lifecycle");
    let tenant_id = durable_tenant("tenant-a");
    let subscription_scope = scope("tenant-a", "agent-a", "capability-a");
    let id = subscription_id("subscription-a");
    let target = subscription_target(&subscription_scope, &id);
    let durable = match Store::open(&root) {
        Ok(value) => value,
        Err(error) => panic!("store open failed: {error}"),
    };
    let mut subscriptions = match SubscriptionStore::open(durable, tenant_id.clone()) {
        Ok(value) => value,
        Err(error) => panic!("subscription store open failed: {error}"),
    };
    if let Err(error) = subscriptions.create(
        id.clone(),
        request(subscription_scope.clone(), filter("tenant-a", "agent-a"), 4),
    ) {
        panic!("subscription create failed: {error}");
    }
    let paused = match subscriptions.pause(&target) {
        Ok(value) => value,
        Err(error) => panic!("pause failed: {error}"),
    };
    assert!(paused.paused);
    assert!(matches!(
        subscriptions.mark_delivered(&target, Cursor(5)),
        Err(SubscriptionError::Paused)
    ));

    let wrong_scope = subscription_target(&scope("tenant-b", "agent-a", "capability-a"), &id);
    assert!(matches!(
        subscriptions.get(&wrong_scope),
        Err(SubscriptionError::NotFound)
    ));
    drop(subscriptions);

    let durable = match Store::open(&root) {
        Ok(value) => value,
        Err(error) => panic!("store restart failed: {error}"),
    };
    let mut restarted = match SubscriptionStore::open(durable, tenant_id.clone()) {
        Ok(value) => value,
        Err(error) => panic!("subscription restart failed: {error}"),
    };
    assert!(matches!(restarted.get(&target), Ok(record) if record.paused));
    let resumed = match restarted.resume(&target) {
        Ok(value) => value,
        Err(error) => panic!("resume failed: {error}"),
    };
    assert!(!resumed.paused);
    if let Err(error) = restarted.delete(&target) {
        panic!("delete failed: {error}");
    }
    drop(restarted);

    let durable = match Store::open(&root) {
        Ok(value) => value,
        Err(error) => panic!("final store restart failed: {error}"),
    };
    let final_store = match SubscriptionStore::open(durable, tenant_id) {
        Ok(value) => value,
        Err(error) => panic!("final subscription restart failed: {error}"),
    };
    assert!(final_store.list(&subscription_scope).is_empty());
    assert!(matches!(
        final_store.get(&target),
        Err(SubscriptionError::NotFound)
    ));
    let _ = fs::remove_dir_all(root);
}
