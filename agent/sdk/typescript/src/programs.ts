import type { AuthorizedReceiptBatch, ReceiptVerification } from "./verifier.js";
import { verifyReceiptOutcome } from "./verifier.js";
import type { IdempotencyKey, ProductionClient } from "./production.js";

const HEX32 = /^[0-9a-f]{64}$/u;
const MAX_CALLDATA = 1_048_576;
const MAX_CAPABILITIES = 256;

export interface ProgramFreshness { readonly observedSequence: string; readonly observedAt: string; readonly validThrough: string; readonly stateRoot: string }
export interface ProgramVersion { readonly number: number; readonly codeHash: string; readonly abiVersion: number; readonly interfaceDigest?: string }
export interface ProgramDiscovery { readonly programId: string; readonly lifecycle: "active" | "frozen" | "retired"; readonly activeVersion: ProgramVersion; readonly versions: readonly ProgramVersion[]; readonly receiptDigest: string; readonly freshness: ProgramFreshness }
export interface ProgramInterface { readonly programId: string; readonly version: number; readonly codeHash: string; readonly abiVersion: number; readonly interfaceBytes?: Uint8Array; readonly interfaceDigest?: string; readonly receiptDigest: string; readonly freshness: ProgramFreshness }
export interface ProgramBudget { readonly fuel: string; readonly feeLimit: string }
export interface ProgramCall { readonly programId: string; readonly version: number; readonly codeHash: string; readonly abiVersion: number; readonly entrypoint: string; readonly calldata: Uint8Array; readonly budget: ProgramBudget; readonly capabilities: readonly Uint8Array[]; readonly signedActivity: Uint8Array }
export type ProgramFailure = Readonly<{ kind: "unknown-program" | "inactive-program" | "version-mismatch" | "code-hash-mismatch" | "abi-mismatch" | "entrypoint" | "composition" | "reentrancy" | "depth-exceeded" | "fanout-exceeded" | "guest-refused" | "authority" | "resource" | "settlement" | "callback" | "fault"; resultCode: number; detail: Uint8Array }>;
export type ProgramOutcome = Readonly<{ kind: "completed"; resultCode: number; response: Uint8Array; fuelUsed: string; feeUnits: string }> | Readonly<{ kind: "failed"; failure: ProgramFailure; fuelUsed: string; feeUnits: string }>;
export interface ProgramEvidence { readonly receipt: Uint8Array; readonly authority: AuthorizedReceiptBatch; readonly activityId: Uint8Array; readonly outcome: ProgramOutcome; readonly terminalAttachments: readonly Uint8Array[] }
export type ProgramSubmission = Readonly<{ state: "unknown"; activityId: string; idempotencyKey: string; retainedSignedActivity: Uint8Array }> | Readonly<{ state: "executed"; activityId: string; idempotencyKey: string; evidence: ProgramEvidence }>;
export interface VerifiedProgramExecution { readonly verification: ReceiptVerification; readonly outcome: ProgramOutcome; readonly terminalAttachments: readonly Uint8Array[] }

function bounded(call: ProgramCall): void {
  if (!HEX32.test(call.programId) || !HEX32.test(call.codeHash) || call.entrypoint.length === 0 || call.entrypoint.length > 255 || call.calldata.length > MAX_CALLDATA || call.capabilities.length > MAX_CAPABILITIES || call.capabilities.some((item) => item.length === 0 || item.length > 4096)) throw new TypeError("invalid bounded program call");
}

async function verifyEvidence(evidence: ProgramEvidence, call: ProgramCall): Promise<VerifiedProgramExecution> {
  const verified = await verifyReceiptOutcome(evidence.receipt, evidence.authority);
  if (verified.receipt.moduleId !== 9 || verified.receipt.operation !== 3 || verified.receipt.moduleVersion !== call.abiVersion || evidence.activityId.length !== 32 || !evidence.activityId.every((byte, index) => byte === verified.receipt.activityId[index])) throw new TypeError("program receipt binding failed");
  return Object.freeze({ verification: verified, outcome: evidence.outcome, terminalAttachments: evidence.terminalAttachments });
}

export class ProgramOperations {
  public constructor(private readonly client: ProductionClient) {}
  public discover(programId: string): Promise<ProgramDiscovery> { if (!HEX32.test(programId)) throw new TypeError("invalid program id"); return this.client.agent("program.discover", { program_id: programId, requested_verification_level: "sequencer-signed" }); }
  public interface(programId: string, version: number): Promise<ProgramInterface> { if (!HEX32.test(programId) || version <= 0) throw new TypeError("invalid program selector"); return this.client.agent("program.interface", { program_id: programId, version, requested_verification_level: "sequencer-signed" }); }
  public async simulate(call: ProgramCall): Promise<VerifiedProgramExecution> { bounded(call); const evidence = await this.client.agent<unknown, ProgramEvidence>("program.simulate", call); return verifyEvidence(evidence, call); }
  public async submit(call: ProgramCall, idempotencyKey: IdempotencyKey): Promise<ProgramSubmission | VerifiedProgramExecution> { bounded(call); const result = await this.client.agent<unknown, ProgramSubmission>("program.call", call, { idempotencyKey }); return result.state === "executed" ? verifyEvidence(result.evidence, call) : result; }
  public receipt(idempotencyKey: string, expectedActivityId: string): Promise<ProgramSubmission> { return this.client.agent("program.receipt", { idempotency_key: idempotencyKey, expected_activity_id: expectedActivityId, requested_verification_level: "sequencer-signed" }); }
  public activity(activityId: string): Promise<ProgramSubmission> { if (!HEX32.test(activityId)) throw new TypeError("invalid activity id"); return this.client.agent("program.activity", { activity_id: activityId, requested_verification_level: "sequencer-signed" }); }
}

export function platform_sdk_programs(): string { return "receipt-verified-program-operations-v1"; }
