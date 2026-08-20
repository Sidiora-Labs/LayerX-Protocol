import {
  PlatformSdkError,
  ProductionClient,
  idempotencyKey,
  protocolAmount,
  verifyReceipt,
  type AuthorizedReceiptBatch,
  type ReceiptVerification,
} from "@sidiora/layerx-sdk";

export interface AgentSpendRequest {
  readonly tenant: string;
  readonly actor: string;
  readonly authority: string;
  readonly accountSequence: string;
  readonly timestampBound: string;
  readonly idempotencyKey: string;
  readonly feeLimit: string;
  readonly payloadBase64: string;
  readonly payloadHash: string;
  readonly asset: string;
  readonly amount: string;
  readonly recipient: string;
}

export interface BudgetReservation {
  readonly reservationId: string;
  readonly requestDigest: string;
  readonly amount: string;
  readonly asset: string;
  readonly state: "reserved" | "held" | "committed" | "released";
}

export type BudgetReserveResult =
  | { readonly kind: "reserved"; readonly reservation: BudgetReservation }
  | { readonly kind: "exhausted"; readonly available: string }
  | { readonly kind: "conflict" };

export interface AgentBudgetLedger {
  reserve(request: {
    readonly tenant: string;
    readonly idempotencyKey: string;
    readonly requestDigest: string;
    readonly amount: string;
    readonly asset: string;
  }): Promise<BudgetReserveResult>;
  hold(reservationId: string, requestDigest: string, approvalId: string): Promise<BudgetReservation>;
  commit(reservationId: string, requestDigest: string, receiptDigest: string): Promise<BudgetReservation>;
  release(reservationId: string, requestDigest: string): Promise<BudgetReservation>;
}

export interface PreparedActivity {
  readonly preparation_ref: string;
  readonly unsigned_canonical_bytes: string;
  readonly signing_preimage: string;
  readonly disclosure: Readonly<Record<string, unknown>>;
  readonly expiry: string;
}

export interface AgentSigner {
  sign(prepared: PreparedActivity): Promise<string>;
}

export interface Submission {
  readonly submission_ref: string;
  readonly state: string | Readonly<Record<string, unknown>>;
  readonly evidence?: readonly unknown[];
  readonly verification_level?: number;
  readonly transitions?: readonly unknown[];
}

export interface AgentReceiptEvidence {
  readonly canonicalReceipt: Uint8Array;
  readonly authorizedBatch: AuthorizedReceiptBatch;
}

export interface AgentReceiptResolver {
  resolve(receiptRef: string): Promise<AgentReceiptEvidence>;
}

export interface ApprovalHold {
  readonly approvalId: string;
  readonly state: "Held";
  readonly canonicalBytesDigest: string;
  readonly enforcement: "daemon_enforced";
}

export interface AgentMiddlewareConfig {
  readonly client: ProductionClient;
  readonly budgets: AgentBudgetLedger;
  readonly signer: AgentSigner;
  readonly receipts: AgentReceiptResolver;
  readonly maximumTrackPolls?: number;
  readonly wait?: (milliseconds: number) => Promise<void>;
}

export type AgentSpendResult =
  | {
    readonly kind: "verified";
    readonly submission: Submission;
    readonly verification: ReceiptVerification;
    readonly reservation: BudgetReservation;
  }
  | { readonly kind: "approval-hold"; readonly approval: ApprovalHold; readonly reservation: BudgetReservation }
  | { readonly kind: "pending"; readonly submission: Submission; readonly reservation: BudgetReservation }
  | { readonly kind: "unknown"; readonly reservation: BudgetReservation; readonly submission?: Submission }
  | { readonly kind: "refused"; readonly code: string; readonly reservation: BudgetReservation }
  | { readonly kind: "budget-refused"; readonly available: string };

export class AgentMiddleware {
  readonly #client: ProductionClient;
  readonly #budgets: AgentBudgetLedger;
  readonly #signer: AgentSigner;
  readonly #receipts: AgentReceiptResolver;
  readonly #maximumTrackPolls: number;
  readonly #wait: (milliseconds: number) => Promise<void>;

  public constructor(config: AgentMiddlewareConfig) {
    this.#client = config.client;
    this.#budgets = config.budgets;
    this.#signer = config.signer;
    this.#receipts = config.receipts;
    this.#maximumTrackPolls = config.maximumTrackPolls ?? 20;
    this.#wait = config.wait ?? ((milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds)));
    if (!Number.isSafeInteger(this.#maximumTrackPolls) || this.#maximumTrackPolls < 0 || this.#maximumTrackPolls > 1_000) {
      throw new AgentMiddlewareError("invalid-request");
    }
  }

  public async spend(request: AgentSpendRequest): Promise<AgentSpendResult> {
    validateSpend(request);
    const amount = protocolAmount(request.amount).toString();
    const mutationKey = idempotencyKey(request.idempotencyKey);
    const requestDigest = await digestSpend(request);
    const reserved = await this.#budgets.reserve({
      tenant: request.tenant,
      idempotencyKey: request.idempotencyKey,
      requestDigest,
      amount,
      asset: request.asset,
    });
    if (reserved.kind === "exhausted") {
      return { kind: "budget-refused", available: reserved.available };
    }
    if (reserved.kind === "conflict") {
      throw new AgentMiddlewareError("idempotency-conflict");
    }
    let prepared: PreparedActivity;
    try {
      prepared = parsePrepared(await this.#client.agent("prepare", {
        actor: request.actor,
        authority: request.authority,
        account_sequence: request.accountSequence,
        timestamp_bound: request.timestampBound,
        idempotency_key: request.idempotencyKey,
        fee_limit: request.feeLimit,
        payload: request.payloadBase64,
        payload_hash: request.payloadHash,
      }, { idempotencyKey: mutationKey }));
    } catch (error) {
      return this.#sdkFailure(error, reserved.reservation, requestDigest);
    }
    const signature = await this.#signer.sign(prepared);
    if (signature.length === 0 || signature.length > 16_384 || signature.includes("\0")) {
      await this.#budgets.release(reserved.reservation.reservationId, requestDigest);
      throw new AgentMiddlewareError("invalid-signature");
    }
    let submission: Submission;
    try {
      submission = parseSubmission(await this.#client.agent("submit", {
        preparation_ref: prepared.preparation_ref,
        signature,
      }, { idempotencyKey: mutationKey }));
    } catch (error) {
      if (error instanceof PlatformSdkError && (error.code === "policy-refusal" || error.code === "budget-refusal")) {
        const approval = await this.#approvalHold(request.tenant, prepared);
        if (approval !== undefined) {
          const held = await this.#budgets.hold(
            reserved.reservation.reservationId,
            requestDigest,
            approval.approvalId,
          );
          return { kind: "approval-hold", approval, reservation: held };
        }
      }
      return this.#sdkFailure(error, reserved.reservation, requestDigest);
    }
    for (let poll = 0; poll < this.#maximumTrackPolls && submissionState(submission) === "Pending"; poll += 1) {
      await this.#wait(Math.min((poll + 1) * 250, 2_500));
      try {
        submission = parseSubmission(await this.#client.agent("track", {
          submission_ref: submission.submission_ref,
        }));
      } catch (error) {
        if (error instanceof PlatformSdkError && error.retry === "unknown-outcome") {
          return { kind: "unknown", reservation: reserved.reservation, submission };
        }
        throw error;
      }
    }
    const state = submissionState(submission);
    if (state === "Unknown") {
      return { kind: "unknown", reservation: reserved.reservation, submission };
    }
    if (state === "Pending") {
      return { kind: "pending", submission, reservation: reserved.reservation };
    }
    if (state === "Failed" || state === "Expired") {
      const released = await this.#budgets.release(reserved.reservation.reservationId, requestDigest);
      return { kind: "refused", code: state.toLowerCase(), reservation: released };
    }
    const receiptRef = executedReceiptRef(submission);
    if (receiptRef === undefined) {
      return { kind: "pending", submission, reservation: reserved.reservation };
    }
    const evidence = await this.#receipts.resolve(receiptRef);
    let verification: ReceiptVerification;
    try {
      verification = await verifyReceipt(evidence.canonicalReceipt, evidence.authorizedBatch);
    } catch {
      throw new AgentMiddlewareError("verification-failure");
    }
    if (
      verification.receipt.amount !== BigInt(amount)
      || !constantTimeHex(verification.receipt.asset, request.asset)
      || !constantTimeHex(verification.receipt.to, request.recipient)
    ) {
      throw new AgentMiddlewareError("verification-failure");
    }
    const receiptDigest = toHex(verification.receiptDigest);
    const committed = await this.#budgets.commit(
      reserved.reservation.reservationId,
      requestDigest,
      receiptDigest,
    );
    if (committed.state !== "committed" || committed.requestDigest !== requestDigest) {
      throw new AgentMiddlewareError("budget-conflict");
    }
    return { kind: "verified", submission, verification, reservation: committed };
  }

  async #sdkFailure(
    error: unknown,
    reservation: BudgetReservation,
    requestDigest: string,
  ): Promise<AgentSpendResult> {
    if (error instanceof PlatformSdkError && error.retry === "unknown-outcome") {
      return { kind: "unknown", reservation };
    }
    if (error instanceof PlatformSdkError && error.retry === "safe") {
      return { kind: "unknown", reservation };
    }
    const released = await this.#budgets.release(reservation.reservationId, requestDigest);
    const code = error instanceof PlatformSdkError ? error.code : "transport-failure";
    return { kind: "refused", code, reservation: released };
  }

  async #approvalHold(tenant: string, prepared: PreparedActivity): Promise<ApprovalHold | undefined> {
    const response = await this.#client.agent<
      { readonly tenant: string; readonly cursor: null; readonly page_limit: number },
      unknown
    >("approval.list", { tenant, cursor: null, page_limit: 100 });
    if (response === null || typeof response !== "object" || Array.isArray(response)) return undefined;
    const approvals = (response as Record<string, unknown>)["approvals"];
    if (!Array.isArray(approvals)) return undefined;
    const digest = disclosureDigest(prepared);
    for (const candidate of approvals) {
      if (candidate === null || typeof candidate !== "object" || Array.isArray(candidate)) continue;
      const object = candidate as Record<string, unknown>;
      if (
        object["state"] === "Held"
        && normalizeDigest(object["canonical_bytes_digest"]) === digest
        && typeof object["approval_id"] === "string"
      ) {
        return {
          approvalId: object["approval_id"],
          state: "Held",
          canonicalBytesDigest: digest,
          enforcement: "daemon_enforced",
        };
      }
    }
    return undefined;
  }
}

export type AgentMiddlewareErrorCode =
  | "invalid-request"
  | "invalid-signature"
  | "idempotency-conflict"
  | "budget-conflict"
  | "verification-failure"
  | "decode-failure";

export class AgentMiddlewareError extends Error {
  public constructor(public readonly code: AgentMiddlewareErrorCode) {
    super(code);
    this.name = "AgentMiddlewareError";
  }
}

export function platform_mw_agent(): "budget-aware-receipt-verified-agent" {
  return "budget-aware-receipt-verified-agent";
}

function validateSpend(request: AgentSpendRequest): void {
  for (const value of [request.tenant, request.actor, request.authority, request.idempotencyKey]) {
    if (value.length === 0 || value.length > 512 || value.includes("\0")) {
      throw new AgentMiddlewareError("invalid-request");
    }
  }
  protocolAmount(request.amount);
  protocolAmount(request.feeLimit);
  if (!/^(0|[1-9][0-9]*)$/u.test(request.accountSequence)
    || !/^(0|[1-9][0-9]*)$/u.test(request.timestampBound)
    || !/^[0-9a-f]{64}$/u.test(request.payloadHash)
    || !/^[0-9a-f]{64}$/u.test(request.asset)
    || !/^[0-9a-f]{64}$/u.test(request.recipient)
    || request.payloadBase64.length === 0
    || request.payloadBase64.length > 1_398_104
    || !isCanonicalBase64(request.payloadBase64)) {
    throw new AgentMiddlewareError("invalid-request");
  }
}

function parsePrepared(value: unknown): PreparedActivity {
  const object = record(value);
  const prepared = {
    preparation_ref: text(object["preparation_ref"], 512),
    unsigned_canonical_bytes: text(object["unsigned_canonical_bytes"], 1_398_104),
    signing_preimage: text(object["signing_preimage"], 1_398_104),
    disclosure: record(object["disclosure"]),
    expiry: text(object["expiry"], 64),
  };
  return prepared;
}

function parseSubmission(value: unknown): Submission {
  const object = record(value);
  return {
    submission_ref: text(object["submission_ref"], 512),
    state: typeof object["state"] === "string" ? object["state"] : record(object["state"]),
    ...(Array.isArray(object["evidence"]) ? { evidence: object["evidence"] } : {}),
    ...(typeof object["verification_level"] === "number" ? { verification_level: object["verification_level"] } : {}),
    ...(Array.isArray(object["transitions"]) ? { transitions: object["transitions"] } : {}),
  };
}

function submissionState(submission: Submission): "Pending" | "Unknown" | "Executed" | "Failed" | "Expired" {
  const state = submission.state;
  if (typeof state === "string") {
    if (["Prepared", "Signed", "Queued", "Submitted", "Acknowledged"].includes(state)) return "Pending";
    if (state === "Unknown" || state === "Executed" || state === "Failed" || state === "Expired") return state;
  } else {
    for (const candidate of ["Unknown", "Executed", "Failed", "Expired", "Pending"] as const) {
      if (candidate in state) return candidate;
      if (state["kind"] === candidate) return candidate;
    }
  }
  throw new AgentMiddlewareError("decode-failure");
}

function executedReceiptRef(submission: Submission): string | undefined {
  if (typeof submission.state === "object") {
    const executed = submission.state["Executed"];
    if (executed !== null && typeof executed === "object" && !Array.isArray(executed)) {
      const receipt = (executed as Record<string, unknown>)["receiptRef"];
      if (typeof receipt === "string") return receipt;
    }
    if (submission.state["kind"] === "Executed" && typeof submission.state["receiptRef"] === "string") {
      return submission.state["receiptRef"];
    }
  }
  for (const evidence of submission.evidence ?? []) {
    if (evidence !== null && typeof evidence === "object" && !Array.isArray(evidence)) {
      const object = evidence as Record<string, unknown>;
      if (object["class"] === "layerx-receipt" && typeof object["reference"] === "string") {
        return object["reference"];
      }
    }
  }
  return undefined;
}

function disclosureDigest(prepared: PreparedActivity): string {
  const digest = prepared.disclosure["canonical_digest"] ?? prepared.disclosure["canonicalDigest"];
  const normalized = normalizeDigest(digest);
  if (normalized === undefined) {
    throw new AgentMiddlewareError("decode-failure");
  }
  return normalized;
}

async function digestSpend(request: AgentSpendRequest): Promise<string> {
  const canonical = JSON.stringify({
    tenant: request.tenant,
    actor: request.actor,
    authority: request.authority,
    accountSequence: request.accountSequence,
    timestampBound: request.timestampBound,
    idempotencyKey: request.idempotencyKey,
    feeLimit: request.feeLimit,
    payloadHash: request.payloadHash,
    asset: request.asset,
    amount: request.amount,
    recipient: request.recipient,
  });
  const digest = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", new TextEncoder().encode(canonical)));
  return toHex(digest);
}

function constantTimeHex(actual: Uint8Array, expected: string): boolean {
  if (!/^[0-9a-f]{64}$/u.test(expected) || actual.length !== 32) return false;
  let difference = 0;
  for (let index = 0; index < actual.length; index += 1) {
    const expectedByte = Number.parseInt(expected.slice(index * 2, index * 2 + 2), 16);
    difference |= (actual[index] ?? 0) ^ expectedByte;
  }
  return difference === 0;
}

function record(value: unknown): Readonly<Record<string, unknown>> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new AgentMiddlewareError("decode-failure");
  }
  return value as Readonly<Record<string, unknown>>;
}

function normalizeDigest(value: unknown): string | undefined {
  if (typeof value !== "string") return undefined;
  const digest = value.startsWith("sha256:") ? value.slice(7) : value;
  return /^[0-9a-f]{64}$/u.test(digest) ? digest : undefined;
}

function isCanonicalBase64(value: string): boolean {
  if (value.length % 4 !== 0 || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value)) {
    return false;
  }
  return true;
}

function text(value: unknown, maximum: number): string {
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || value.includes("\0")) {
    throw new AgentMiddlewareError("decode-failure");
  }
  return value;
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
