use std::collections::BTreeSet;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

use layerx_agentd::capability::{Capability, CapabilityDimensions, CapabilityId, RateCeiling};
use layerx_agentd::identity::ProtocolAuthority;
use layerx_agentd::session::{OpenRequest, SessionId, SessionRecord};
use layerx_agentd::store::TenantId;
use layerx_mcp::server::{
    catalogue, DeploymentMode, InvocationOutcome, ReadOnly, Server, ServerError, ToolKind,
};
use layerx_types::ids::Did;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

fn directory(label: &str) -> std::path::PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "layerx-mcp-readonly-{label}-{}-{sequence}",
        std::process::id()
    ))
}

fn records() -> (SessionRecord, Capability) {
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
    let scopes = catalogue()
        .iter()
        .map(|tool| tool.required_scope.to_owned())
        .collect();
    let session = SessionRecord {
        request: OpenRequest {
            session_id: SessionId([7; 32]),
            token_id: [8; 32],
            tenant,
            agent: Did::new(b"did:layerx:model").unwrap_or_else(|error| panic!("DID: {error:?}")),
            authority: ProtocolAuthority::CapabilityGrant(capability.id.0),
            permitted_activity_types: BTreeSet::from([7]),
            scopes,
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
fn read_only_capability_and_catalogue_expose_no_write_surface() {
    let root = directory("catalogue");
    let (session, capability) = records();
    let server = ReadOnly::bind_records(&session, &capability, 50, &root)
        .unwrap_or_else(|error| panic!("bind: {error:?}"));
    let declaration = server.capability_declaration();
    assert_eq!(declaration.mode, DeploymentMode::ReadOnly);
    assert_eq!(declaration.write_tools, 0);
    assert!(!declaration.mutations_reachable);
    assert!(declaration.read_tools > 0);
    assert!(server
        .tools()
        .iter()
        .all(|tool| tool.kind == ToolKind::Read && tool.mutation == "none"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn write_tools_are_absent_through_every_model_reachable_shape() {
    let root = directory("shapes");
    let (session, capability) = records();
    let mut server = ReadOnly::bind_records(&session, &capability, 50, &root)
        .unwrap_or_else(|error| panic!("bind: {error:?}"));
    let write_tools: Vec<_> = catalogue()
        .iter()
        .filter(|tool| tool.kind == ToolKind::Write)
        .collect();
    assert!(!write_tools.is_empty());
    for write in write_tools {
        assert!(!server.tools().iter().any(|tool| tool.name == write.name));
        for name in [
            write.name.to_owned(),
            format!("{}?cursor=write", write.name),
            format!("resources/{}", write.name),
            format!("errors/retry/{}", write.name),
        ] {
            assert!(matches!(
                server.route(&name, b"untrusted".to_vec()),
                Err(ServerError::ToolAbsent)
            ));
        }
    }
    assert_eq!(server.audit_entries(), 0);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn full_mode_write_invocation_cannot_cross_the_read_only_mode_binding() {
    let full_root = directory("full");
    let read_root = directory("read");
    let (session, capability) = records();
    let mut full = Server::bind_records(&session, &capability, 50, &full_root)
        .unwrap_or_else(|error| panic!("full bind: {error:?}"));
    let full_declaration = full.capability_declaration();
    assert_eq!(full_declaration.mode, DeploymentMode::Full);
    assert!(full_declaration.write_tools > 0);
    assert!(full_declaration.mutations_reachable);
    let invocation = full
        .route("activity.submit", b"validated write".to_vec())
        .unwrap_or_else(|error| panic!("full route: {error:?}"));

    let mut read_only = ReadOnly::bind_records(&session, &capability, 50, &read_root)
        .unwrap_or_else(|error| panic!("read-only bind: {error:?}"));
    assert!(matches!(
        read_only.complete(invocation, InvocationOutcome::Completed),
        Err(ServerError::ToolAbsent)
    ));
    assert_eq!(read_only.audit_entries(), 0);
    let _ = fs::remove_dir_all(full_root);
    let _ = fs::remove_dir_all(read_root);
}

#[test]
fn write_only_scope_cannot_construct_a_read_only_server() {
    let root = directory("write-only");
    let (mut session, capability) = records();
    session.request.scopes = BTreeSet::from(["write:submit".to_owned()]);
    assert!(matches!(
        ReadOnly::bind_records(&session, &capability, 50, &root),
        Err(ServerError::NoScope)
    ));
    let _ = fs::remove_dir_all(root);
}
