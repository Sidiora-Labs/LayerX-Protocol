use std::env;
use std::fs;

use layerx_programs_runtime::{
    AbiError, AuthorizationContext, AuthorizedExecutionRequest, Capability, CapabilitySet,
    CompositionContext, CompositionRules, Executor, PrincipalId, ProgramCatalog, ProgramId,
    ReceiptOracle, ReceiptView, Storage, WasmEngine, CALL_ENTRY_EXPORT,
};

struct NoReceipts;

impl ReceiptOracle for NoReceipts {
    fn verified_receipt(&self, _: [u8; 32]) -> Result<ReceiptView, AbiError> {
        Err(AbiError::ReceiptMismatch)
    }
}

fn main() {
    let artifact = env::args()
        .nth(1)
        .unwrap_or_else(|| panic!("candidate response fixture artifact path is required"));
    let wasm = fs::read(&artifact).unwrap_or_else(|error| panic!("read {artifact}: {error}"));
    let engine = WasmEngine::declared().unwrap_or_else(|error| panic!("engine: {error}"));
    let root = engine
        .validate_candidate_v2(&wasm)
        .unwrap_or_else(|error| panic!("root validation: {error}"));
    let child = engine
        .validate_candidate_v2(&wasm)
        .unwrap_or_else(|error| panic!("child validation: {error}"));
    let root_id = ProgramId::new([6; 32]).unwrap_or_else(|error| panic!("root id: {error}"));
    let child_id = ProgramId::new([7; 32]).unwrap_or_else(|error| panic!("child id: {error}"));
    let principal = PrincipalId::new([8; 32]).unwrap_or_else(|error| panic!("principal: {error}"));
    let capabilities = CapabilitySet::new([Capability::Call { program: child_id }])
        .unwrap_or_else(|error| panic!("capability: {error}"));
    let mut catalog = ProgramCatalog::new();
    catalog.insert(child_id, child);
    let input = [0x11, 0, 0xff, 0x22];
    let mut storage = Storage::new();
    let record = Executor::declared()
        .execute_authorized_candidate(
            &mut storage,
            AuthorizedExecutionRequest {
                module: &root,
                program: root_id,
                authorization: AuthorizationContext::new(principal, capabilities),
                receipts: &NoReceipts,
                entrypoint: CALL_ENTRY_EXPORT,
                calldata: &input,
                composition: CompositionContext::catalog(catalog, CompositionRules::declared()),
                response_capacity: input.len(),
            },
        )
        .unwrap_or_else(|error| panic!("candidate execution: {error}"));

    assert_eq!(record.response.code, 0);
    assert_eq!(record.response.bytes, input);
    assert_eq!(record.call_graph.edges().len(), 1);
    assert_eq!(record.call_graph.edges()[0].callee(), child_id);
    assert_eq!(record.execution.usage.output_bytes, 7);
}
