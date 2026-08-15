use std::collections::BTreeSet;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use layerx_agentd::capability::{Capability, CapabilityDimensions, CapabilityId, RateCeiling};
use layerx_agentd::identity::ProtocolAuthority;
use layerx_agentd::session::{OpenRequest, SessionId, SessionRecord};
use layerx_agentd::store::TenantId;
use layerx_mcp::server::{
    DaemonGate, InvocationOutcome, Server, ServerError, ToolKind, REQUIRED_DAEMON_GATES,
};
use layerx_types::ids::Did;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

fn directory(label: &str) -> std::path::PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "layerx-mcp-scope-{label}-{}-{sequence}",
        std::process::id()
    ))
}

fn records(scopes: &[&str]) -> (SessionRecord, Capability) {
    let tenant = TenantId::new("tenant-a").unwrap_or_else(|error| panic!("tenant: {error}"));
    let capability = Capability::new(
        CapabilityId([9; 32]),
        tenant.clone(),
        CapabilityDimensions {
            activity_types: BTreeSet::from([7]),
            counterparties: BTreeSet::from([[2; 32]]),
            assets: BTreeSet::from([[3; 32]]),
            amount_ceiling: 100,
            rate_ceiling: RateCeiling {
                maximum_uses: 2,
                window_sequences: 10,
            },
            purposes: BTreeSet::from(["service-payment".to_owned()]),
            expiry_sequence: 200,
        },
    )
    .unwrap_or_else(|error| panic!("capability: {error:?}"));
    let session = SessionRecord {
        request: OpenRequest {
            session_id: SessionId([7; 32]),
            token_id: [8; 32],
            tenant,
            agent: Did::new(b"did:layerx:model").unwrap_or_else(|error| panic!("DID: {error:?}")),
            authority: ProtocolAuthority::CapabilityGrant(capability.id.0),
            permitted_activity_types: BTreeSet::from([7]),
            scopes: scopes.iter().map(|scope| (*scope).to_owned()).collect(),
            expiry_sequence: 150,
            opening_client: "mcp".to_owned(),
            policy_version: "policy-v1".to_owned(),
        },
        open: true,
        sequence: 0,
        budget_reserved: 0,
        subscription_cursor: 0,
    };
    (session, capability)
}

#[test]
fn server_exposes_only_the_exact_bound_scope_and_separates_read_from_write() {
    let root = directory("filter");
    let (session, capability) = records(&["read:balance", "write:prepare"]);
    let server = Server::bind_records(&session, &capability, 50, &root)
        .unwrap_or_else(|error| panic!("bind: {error:?}"));
    assert_eq!(server.binding().tenant().as_str(), "tenant-a");
    assert_eq!(server.binding().scopes().len(), 2);
    assert_eq!(server.tools().len(), 2);
    assert_eq!(
        server
            .tool("balance.get")
            .unwrap_or_else(|| panic!("balance absent"))
            .kind,
        ToolKind::Read
    );
    assert_eq!(
        server
            .tool("activity.prepare")
            .unwrap_or_else(|| panic!("prepare absent"))
            .kind,
        ToolKind::Write
    );
    assert!(server.tool("history.list").is_none());
    assert!(server.tool("activity.submit").is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn every_reachable_tool_is_an_audited_daemon_invocation_with_all_gates() {
    let root = directory("route");
    let (session, capability) = records(&["read:balance"]);
    let mut server = Server::bind_records(&session, &capability, 50, &root)
        .unwrap_or_else(|error| panic!("bind: {error:?}"));
    assert!(matches!(
        server.route("activity.submit", b"untrusted".to_vec()),
        Err(ServerError::ToolAbsent)
    ));
    let invocation = server
        .route("balance.get", b"schema-validated-arguments".to_vec())
        .unwrap_or_else(|error| panic!("route: {error:?}"));
    assert_eq!(invocation.gates(), REQUIRED_DAEMON_GATES);
    assert_eq!(
        invocation.gates(),
        [
            DaemonGate::Policy,
            DaemonGate::Capability,
            DaemonGate::Budget,
            DaemonGate::RateLimit,
            DaemonGate::Audit,
        ]
    );
    let digest = invocation.arguments_digest();
    let record = server
        .complete(invocation, InvocationOutcome::Completed)
        .unwrap_or_else(|error| panic!("complete: {error:?}"));
    assert_eq!(record.arguments_digest, digest);
    assert_eq!(record.outcome, InvocationOutcome::Completed);
    assert_eq!(server.audit_entries(), 3);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn binding_refuses_unlinked_cross_tenant_expired_and_empty_authority() {
    let root = directory("refuse");
    let (session, capability) = records(&["read:balance"]);
    let mut unlinked = session.clone();
    unlinked.request.authority = ProtocolAuthority::CapabilityGrant([1; 32]);
    assert!(matches!(
        Server::bind_records(&unlinked, &capability, 50, &root),
        Err(ServerError::CapabilityMismatch)
    ));
    let mut cross_tenant = capability.clone();
    cross_tenant.tenant =
        TenantId::new("tenant-b").unwrap_or_else(|error| panic!("tenant: {error}"));
    assert!(matches!(
        Server::bind_records(&session, &cross_tenant, 50, &root),
        Err(ServerError::TenantMismatch)
    ));
    assert!(matches!(
        Server::bind_records(&session, &capability, 150, &root),
        Err(ServerError::ExpiredAuthority)
    ));
    let (empty, capability) = records(&["not-a-tool-scope"]);
    assert!(matches!(
        Server::bind_records(&empty, &capability, 50, &root),
        Err(ServerError::NoScope)
    ));
    let _ = fs::remove_dir_all(root);
}
