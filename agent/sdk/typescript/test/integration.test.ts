import {
  APPROVAL_CONTRACT_INTRODUCED,
  APPROVAL_DECISION_OUTCOMES,
  APPROVAL_ENFORCEMENT_NOTICE,
  APPROVAL_EVENT_KINDS,
  APPROVAL_STATES,
  Client,
  VerificationLevel,
  layerx_sdk_ts_package,
  parseAmount,
  parseSequence,
  requireVerified,
  type ApprovalApproveRequest,
  type ApprovalGetRequest,
  type ApprovalListRequest,
  type ApprovalRejectRequest,
  type Operation,
  type SubmissionState,
  type VerifiedRead,
} from "../src/index.js";

function assert(condition: boolean, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function expectThrow(operation: () => unknown, message: string): void {
  let threw = false;
  try {
    operation();
  } catch {
    threw = true;
  }
  assert(threw, message);
}

export async function verifyTypeScriptPackage(): Promise<void> {
  const metadata = layerx_sdk_ts_package();
  assert(metadata.contractMajor === 1, "contract major drift");
  assert(parseAmount("9007199254740993") === 9007199254740993n, "amount lost precision");
  assert(parseSequence("18446744073709551615") === 18446744073709551615n, "sequence lost precision");
  expectThrow(() => parseAmount("1.5"), "fractional amount accepted");
  expectThrow(() => parseSequence("18446744073709551616"), "overflowing sequence accepted");

  const read: VerifiedRead<bigint> = {
    value: 10n,
    achievedVerificationLevel: VerificationLevel.StateProven,
    freshness: {
      chainHead: 20n,
      latestBatch: "batch-19",
      latestCheckpoint: "checkpoint-18",
      valueSequence: 17n,
    },
  };
  assert(requireVerified(VerificationLevel.StateProven, read) === read, "proven read changed");
  expectThrow(
    () => requireVerified(VerificationLevel.CheckpointFinalised, read),
    "below-requested read accepted",
  );

  const unknown: SubmissionState = { kind: "Unknown" };
  const futureFailure: SubmissionState = { kind: "Failed", protocolResultCode: -77777 };
  assert(unknown.kind === "Unknown", "unknown collapsed");
  assert(futureFailure.protocolResultCode === -77777, "protocol code changed");

  assert(APPROVAL_CONTRACT_INTRODUCED === "1.1", "approval introduction minor changed");
  assert(APPROVAL_ENFORCEMENT_NOTICE.includes("confers no protocol authority"), "approval authority overstated");
  assert(APPROVAL_STATES.join(",") === "Held,Granted,Rejected,Expired,Defective", "approval states diverged");
  assert(APPROVAL_DECISION_OUTCOMES.join(",") === "Granted,Rejected,Expired,Defective,AlreadyDecided,Conflict", "approval outcomes diverged");
  assert(APPROVAL_EVENT_KINDS.join(",") === "Created,Granted,Rejected,Expired,Defective", "approval events diverged");

  const calls: Operation[] = [];
  const client = new Client({
    async call<TRequest, TResponse>(operation: Operation, _request: TRequest): Promise<TResponse> {
      calls.push(operation);
      return {} as TResponse;
    },
  });
  await client.approvalList({ tenant: "tenant-a", cursor: null, pageLimit: 50 } satisfies ApprovalListRequest);
  await client.approvalGet({ tenant: "tenant-a", approvalId: "approval-7" } satisfies ApprovalGetRequest);
  await client.approvalApprove({ tenant: "tenant-a", approvalId: "approval-7", idempotencyKey: "approve-7" } satisfies ApprovalApproveRequest);
  await client.approvalReject({ tenant: "tenant-a", approvalId: "approval-7", idempotencyKey: "reject-7", reason: "not expected" } satisfies ApprovalRejectRequest);
  assert(calls.join(",") === "approval.list,approval.get,approval.approve,approval.reject", "approval operations diverged");
}

await verifyTypeScriptPackage();
