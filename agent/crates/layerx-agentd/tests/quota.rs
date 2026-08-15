use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use layerx_agentd::limits::admission::Priority;
use layerx_agentd::limits::quota::{
    ClientActivity, QuotaError, Resource, SheddingPolicy, SheddingReason, TenantQuota,
};
use layerx_agentd::limits::{shed, Quota};
use layerx_agentd::store::{Store, TenantId};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

fn directory(label: &str) -> PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "layerx-quota-{label}-{}-{sequence}",
        std::process::id()
    ))
}

fn tenant(value: &str) -> TenantId {
    TenantId::new(value).expect("valid test tenant")
}

fn tenant_quota(tenant: TenantId, limit: usize) -> TenantQuota {
    TenantQuota::new(
        tenant,
        Resource::ALL.into_iter().map(|resource| (resource, limit)),
    )
    .expect("complete tenant quota")
}

fn policy() -> SheddingPolicy {
    SheddingPolicy {
        window_ms: 1_000,
        maximum_requests: 5,
        maximum_retries: 2,
        maximum_identical_operations: 3,
        shed_for_ms: 5_000,
    }
}

fn activity(
    tenant: TenantId,
    client_id: &str,
    digest: u8,
    retry: bool,
    observed_at_ms: u64,
) -> ClientActivity {
    ClientActivity {
        tenant,
        client_id: client_id.to_owned(),
        operation_digest: [digest; 32],
        retry,
        observed_at_ms,
    }
}

#[test]
fn every_durable_resource_refuses_creation_past_its_tenant_quota() {
    let root = directory("resources");
    let tenant = tenant("tenant-a");
    let mut store = Store::open(&root).expect("durable store opens");
    let quota = Quota::new([tenant_quota(tenant.clone(), 1)], policy()).expect("valid quota");

    for (index, resource) in Resource::ALL.into_iter().enumerate() {
        quota
            .create_resource(
                &mut store,
                &tenant,
                "client-a",
                resource,
                vec![index as u8 + 1],
                vec![0xa0 + index as u8],
                100,
            )
            .expect("first durable object fits");
        assert!(matches!(
            quota.create_resource(
                &mut store,
                &tenant,
                "client-a",
                resource,
                vec![index as u8 + 20],
                vec![0xb0 + index as u8],
                100,
            ),
            Err(QuotaError::Exhausted {
                resource: exhausted,
                used: 1,
                limit: 1,
                ..
            }) if exhausted == resource
        ));
    }
    drop(store);

    let reopened = Store::open(&root).expect("durable objects survive restart");
    let health = quota
        .health(&reopened, &tenant, 100)
        .expect("tenant health is available");
    assert_eq!(health.resources.len(), 5);
    assert!(health
        .resources
        .iter()
        .all(|resource| resource.used == 1 && resource.remaining == 0));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn retry_storm_sheds_only_the_pathological_client_and_records_the_decision() {
    let root = directory("retry-storm");
    let tenant = tenant("tenant-a");
    let mut store = Store::open(&root).expect("durable store opens");
    let mut quota = Quota::new([tenant_quota(tenant.clone(), 10)], policy()).expect("valid quota");

    assert_eq!(
        shed(
            &mut quota,
            &mut store,
            activity(tenant.clone(), "bad-client", 1, true, 100)
        )
        .expect("first retry is observed"),
        None
    );
    assert_eq!(
        shed(
            &mut quota,
            &mut store,
            activity(tenant.clone(), "bad-client", 2, true, 110)
        )
        .expect("second retry is observed"),
        None
    );
    let decision = shed(
        &mut quota,
        &mut store,
        activity(tenant.clone(), "bad-client", 3, true, 120),
    )
    .expect("third retry is observed")
    .expect("retry threshold emits a decision");
    assert_eq!(decision.reason, SheddingReason::RetryStorm);
    assert_eq!(decision.shed_until_ms, 5_120);

    assert!(matches!(
        quota.admit_work(&store, &tenant, "bad-client", Priority::BulkRead, 121),
        Err(QuotaError::ClientShed {
            retry_after_ms: 4_999,
            ..
        })
    ));
    quota
        .admit_work(&store, &tenant, "healthy-client", Priority::BulkRead, 121)
        .expect("a sibling client is unaffected");
    drop(store);

    let reopened = Store::open(&root).expect("store reopens");
    assert!(matches!(
        quota.admit_work(&reopened, &tenant, "bad-client", Priority::BulkRead, 122),
        Err(QuotaError::ClientShed { .. })
    ));
    let health = quota
        .health(&reopened, &tenant, 122)
        .expect("health reads durable shed state");
    assert_eq!(health.actively_shed_clients, vec!["bad-client"]);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn hot_loop_and_request_storm_have_distinct_recorded_reasons() {
    let root = directory("reasons");
    let tenant = tenant("tenant-a");
    let mut store = Store::open(&root).expect("durable store opens");
    let mut quota = Quota::new([tenant_quota(tenant.clone(), 10)], policy()).expect("valid quota");

    let mut hot_loop = None;
    for observed_at_ms in 100..104 {
        hot_loop = shed(
            &mut quota,
            &mut store,
            activity(tenant.clone(), "hot-client", 9, false, observed_at_ms),
        )
        .expect("hot-loop observation succeeds");
    }
    assert_eq!(
        hot_loop.expect("hot loop is detected").reason,
        SheddingReason::HotLoop
    );

    let mut request_storm = None;
    for index in 0..6_u64 {
        request_storm = shed(
            &mut quota,
            &mut store,
            activity(
                tenant.clone(),
                "busy-client",
                index as u8,
                false,
                200 + index,
            ),
        )
        .expect("request observation succeeds");
    }
    assert_eq!(
        request_storm.expect("request storm is detected").reason,
        SheddingReason::PathologicalClient
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn another_tenant_keeps_its_quota_and_capacity_during_shedding() {
    let root = directory("tenant-isolation");
    let alpha = tenant("alpha");
    let beta = tenant("beta");
    let mut store = Store::open(&root).expect("durable store opens");
    let mut quota = Quota::new(
        [
            tenant_quota(alpha.clone(), 1),
            tenant_quota(beta.clone(), 2),
        ],
        policy(),
    )
    .expect("valid quotas");

    quota
        .create_resource(
            &mut store,
            &alpha,
            "alpha-client",
            Resource::Subscription,
            b"alpha-1".to_vec(),
            b"record".to_vec(),
            1,
        )
        .expect("alpha consumes its own quota");
    assert!(matches!(
        quota.create_resource(
            &mut store,
            &alpha,
            "alpha-client",
            Resource::Subscription,
            b"alpha-2".to_vec(),
            b"record".to_vec(),
            1,
        ),
        Err(QuotaError::Exhausted { .. })
    ));

    for retry in 0..3 {
        shed(
            &mut quota,
            &mut store,
            activity(
                alpha.clone(),
                "alpha-client",
                retry,
                true,
                10 + retry as u64,
            ),
        )
        .expect("alpha retry is observed");
    }
    quota
        .create_resource(
            &mut store,
            &beta,
            "beta-client",
            Resource::Subscription,
            b"beta-1".to_vec(),
            b"record".to_vec(),
            20,
        )
        .expect("beta keeps independent durable capacity");
    quota
        .admit_work(&store, &beta, "beta-client", Priority::BulkRead, 20)
        .expect("beta is not shed to compensate for alpha");
    let health = quota.health(&store, &beta, 20).expect("beta health");
    assert_eq!(health.resources[0].used, 1);
    assert!(health.actively_shed_clients.is_empty());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn critical_submission_and_receipt_work_remain_reserved_during_a_client_shed() {
    let root = directory("reserved");
    let tenant = tenant("tenant-a");
    let mut store = Store::open(&root).expect("durable store opens");
    let mut quota = Quota::new([tenant_quota(tenant.clone(), 10)], policy()).expect("valid quota");
    for retry in 0..3 {
        shed(
            &mut quota,
            &mut store,
            activity(tenant.clone(), "client-a", retry, true, 100 + retry as u64),
        )
        .expect("retry observation succeeds");
    }

    for priority in [Priority::Submission, Priority::ReceiptResolution] {
        quota
            .admit_work(&store, &tenant, "client-a", priority, 110)
            .expect("critical resolution capacity is protected");
    }
    assert!(matches!(
        quota.admit_work(&store, &tenant, "client-a", Priority::Backfill, 110),
        Err(QuotaError::ClientShed { .. })
    ));
    let _ = fs::remove_dir_all(root);
}
