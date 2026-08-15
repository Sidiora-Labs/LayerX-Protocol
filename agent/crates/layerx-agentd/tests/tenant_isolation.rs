use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use layerx_agentd::limits::admission::Priority;
use layerx_agentd::limits::quota::{ClientActivity, Resource, SheddingPolicy, TenantQuota};
use layerx_agentd::limits::{shed, Quota};
use layerx_agentd::store::{Store, TenantId};
use layerx_agentd::tenant::{
    ChannelBinding, ChannelKind, Config, IsolationError, RedactionPolicy, Retention, SignerBinding,
    SignerMaterial, TenantIsolation,
};
use layerx_types::verify::VerificationLevel;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

fn directory(label: &str) -> PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "layerx-tenant-isolation-{label}-{}-{sequence}",
        std::process::id()
    ))
}

fn tenant(value: &str) -> TenantId {
    TenantId::new(value).expect("valid tenant")
}

fn quota_config(tenant: TenantId, limit: usize) -> TenantQuota {
    TenantQuota::new(
        tenant,
        Resource::ALL.into_iter().map(|resource| (resource, limit)),
    )
    .expect("valid tenant quota")
}

#[test]
fn signer_bindings_and_key_references_cannot_cross_tenants() {
    let alpha = tenant("alpha");
    let beta = tenant("beta");
    let mut isolation = TenantIsolation::default();
    isolation
        .bind_signer(
            SignerBinding::new(
                alpha.clone(),
                "primary",
                SignerMaterial::LocalEncryptedReference("keystore://alpha/primary".to_owned()),
            )
            .expect("alpha binding is valid"),
        )
        .expect("alpha signer binds");
    isolation
        .bind_signer(
            SignerBinding::new(
                beta.clone(),
                "primary",
                SignerMaterial::External {
                    endpoint: "unix:///run/layerx/beta-signer.sock".to_owned(),
                    public_key: [9; 32],
                },
            )
            .expect("beta binding is valid"),
        )
        .expect("beta signer binds");
    assert_eq!(
        isolation.bind_signer(
            SignerBinding::new(
                alpha.clone(),
                "primary",
                SignerMaterial::External {
                    endpoint: "unix:///run/layerx/replacement.sock".to_owned(),
                    public_key: [0xff; 32],
                },
            )
            .expect("replacement binding is structurally valid")
        ),
        Err(IsolationError::Duplicate)
    );

    assert_eq!(
        isolation
            .signer(&alpha, "primary")
            .map(SignerBinding::tenant),
        Ok(&alpha)
    );
    assert_eq!(
        isolation.signer(&beta, "alpha-only"),
        Err(IsolationError::NotAuthorized)
    );
    assert_eq!(
        isolation.signer(&beta, "missing"),
        Err(IsolationError::NotAuthorized)
    );
    assert_ne!(
        isolation
            .signer(&alpha, "primary")
            .expect("alpha signer")
            .material(),
        isolation
            .signer(&beta, "primary")
            .expect("beta signer")
            .material()
    );
    assert_eq!(
        isolation
            .signer(&alpha, "primary")
            .expect("original alpha signer remains")
            .material(),
        &SignerMaterial::LocalEncryptedReference("keystore://alpha/primary".to_owned())
    );
}

#[test]
fn quota_exhaustion_and_shedding_do_not_starve_another_tenant_or_critical_work() {
    let root = directory("capacity");
    let alpha = tenant("alpha");
    let beta = tenant("beta");
    let mut store = Store::open(&root).expect("store opens");
    let mut quota = Quota::new(
        [
            quota_config(alpha.clone(), 1),
            quota_config(beta.clone(), 1),
        ],
        SheddingPolicy {
            window_ms: 1_000,
            maximum_requests: 10,
            maximum_retries: 2,
            maximum_identical_operations: 10,
            shed_for_ms: 5_000,
        },
    )
    .expect("quota config is valid");
    quota
        .create_resource(
            &mut store,
            &alpha,
            "alpha-client",
            Resource::OutboxEntry,
            b"alpha-outbox".to_vec(),
            b"queued".to_vec(),
            10,
        )
        .expect("alpha uses its quota");
    for retry in 0..3_u64 {
        shed(
            &mut quota,
            &mut store,
            ClientActivity {
                tenant: alpha.clone(),
                client_id: "alpha-client".to_owned(),
                operation_digest: [retry as u8; 32],
                retry: true,
                observed_at_ms: 20 + retry,
            },
        )
        .expect("alpha activity is observed");
    }

    quota
        .create_resource(
            &mut store,
            &beta,
            "beta-client",
            Resource::OutboxEntry,
            b"beta-outbox".to_vec(),
            b"queued".to_vec(),
            30,
        )
        .expect("beta retains its own capacity");
    quota
        .admit_work(&store, &beta, "beta-client", Priority::BulkRead, 30)
        .expect("alpha shedding does not affect beta");
    for priority in [Priority::Submission, Priority::ReceiptResolution] {
        quota
            .admit_work(&store, &alpha, "alpha-client", priority, 30)
            .expect("critical lane stays reserved for shed tenant");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn subscriptions_streams_and_mcp_bindings_hide_cross_tenant_existence() {
    let alpha = tenant("alpha");
    let beta = tenant("beta");
    let mut isolation = TenantIsolation::default();
    for kind in [
        ChannelKind::Subscription,
        ChannelKind::Stream,
        ChannelKind::McpServer,
    ] {
        isolation
            .bind_channel(
                ChannelBinding::new(alpha.clone(), kind, "shared-opaque-id")
                    .expect("binding is valid"),
            )
            .expect("channel binds");
        assert_eq!(
            isolation.channel(&beta, kind, "shared-opaque-id"),
            Err(IsolationError::NotAuthorized)
        );
        assert_eq!(
            isolation.channel(&beta, kind, "missing"),
            Err(IsolationError::NotAuthorized)
        );
    }
    assert_eq!(
        isolation.validate_filter(&alpha, [&alpha, &beta]),
        Err(IsolationError::NotAuthorized)
    );
    assert_eq!(isolation.validate_filter(&alpha, [&alpha, &alpha]), Ok(()));
    assert_eq!(
        format!("{:?}", IsolationError::NotAuthorized),
        "NotAuthorized"
    );
}

#[test]
fn policy_redaction_retention_verification_and_approvals_are_per_tenant() {
    let alpha = tenant("alpha");
    let beta = tenant("beta");
    let mut isolation = TenantIsolation::default();
    isolation
        .set_config(Config {
            tenant: alpha.clone(),
            policy_version: "alpha-policy".to_owned(),
            redaction: RedactionPolicy::Strict,
            retention: Retention {
                event_sequences: 100,
                audit_sequences: 200,
                receipt_sequences: 300,
            },
            verification_default: VerificationLevel::CHECKPOINT_FINALISED,
            approval_required_for: BTreeSet::from([7]),
        })
        .expect("alpha config is valid");
    isolation
        .set_config(Config {
            tenant: beta.clone(),
            policy_version: "beta-policy".to_owned(),
            redaction: RedactionPolicy::ReceiptOnly,
            retention: Retention {
                event_sequences: 10,
                audit_sequences: 20,
                receipt_sequences: 30,
            },
            verification_default: VerificationLevel::STATE_PROVEN,
            approval_required_for: BTreeSet::new(),
        })
        .expect("beta config is valid");

    let alpha_config = isolation.config(&alpha).expect("alpha config exists");
    let beta_config = isolation.config(&beta).expect("beta config exists");
    assert_eq!(alpha_config.policy_version, "alpha-policy");
    assert_eq!(beta_config.policy_version, "beta-policy");
    assert_eq!(alpha_config.redaction, RedactionPolicy::Strict);
    assert_eq!(beta_config.redaction, RedactionPolicy::ReceiptOnly);
    assert!(alpha_config.approval_required_for.contains(&7));
    assert!(!beta_config.approval_required_for.contains(&7));
}
