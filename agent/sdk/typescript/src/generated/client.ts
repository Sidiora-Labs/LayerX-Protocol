// Generated from the LayerX Agent API schema. Do not hand-edit.

const PACKAGE_METADATA = Object.freeze({
  name: "@sidiora/layerx-sdk",
  version: "0.1.0",
  contractMajor: 1,
});

export function layerx_sdk_ts_package(): typeof PACKAGE_METADATA {
  return PACKAGE_METADATA;
}

export type Amount = bigint;
export function parseAmount(value: string): Amount {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new RangeError("invalid Amount");
  const parsed = BigInt(value);
  if (parsed > 340282366920938463463374607431768211455n) throw new RangeError("Amount out of range");
  return parsed;
}
export type BudgetLimit = bigint;
export function parseBudgetLimit(value: string): BudgetLimit {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new RangeError("invalid BudgetLimit");
  const parsed = BigInt(value);
  if (parsed > 340282366920938463463374607431768211455n) throw new RangeError("BudgetLimit out of range");
  return parsed;
}
export type Sequence = bigint;
export function parseSequence(value: string): Sequence {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new RangeError("invalid Sequence");
  const parsed = BigInt(value);
  if (parsed > 18446744073709551615n) throw new RangeError("Sequence out of range");
  return parsed;
}
export type TimestampSeconds = bigint;
export function parseTimestampSeconds(value: string): TimestampSeconds {
  if (!/^(0|[1-9][0-9]*)$/.test(value)) throw new RangeError("invalid TimestampSeconds");
  const parsed = BigInt(value);
  if (parsed > 18446744073709551615n) throw new RangeError("TimestampSeconds out of range");
  return parsed;
}

export enum VerificationLevel {
  Unverified = 0,
  SequencerSigned = 1,
  BatchIncluded = 2,
  StateProven = 3,
  CheckpointFinalised = 4,
  SettlementAnchored = 5,
}

export type SubmissionState =
  | { kind: "Unknown" }
  | { kind: "Executed"; receiptRef: string }
  | { kind: "Failed"; protocolResultCode: number }
  | { kind: "Pending"; stage: string };

export interface VerifiedRead<T> {
  value: T;
  achievedVerificationLevel: VerificationLevel;
  freshness: { chainHead: bigint; latestBatch: string; latestCheckpoint: string; valueSequence: bigint };
}

export function requireVerified<T>(requested: VerificationLevel, read: VerifiedRead<T>): VerifiedRead<T> {
  if (read.achievedVerificationLevel === VerificationLevel.Unverified) {
    throw new Error("unverified_read");
  }
  if (read.achievedVerificationLevel < requested) {
    throw new Error(`verification_below_requested:${requested}:${read.achievedVerificationLevel}`);
  }
  return read;
}

export interface IdempotentMutation<T> {
  requestId: bigint;
  key: Uint8Array;
  bodyDigest: Uint8Array;
  operation: T;
}

export type ErrorClass = "TransportFailure" | "Deadline" | "ProtocolIncompatibility" | "UnavailableCapability" | "CoreRejection" | "VerificationFailure" | "PolicyRefusal" | "CapabilityRefusal" | "BudgetRefusal" | "RateLimit" | "IdempotencyConflict" | "InternalFault";
export interface ApiError {
  errorClass: ErrorClass;
  protocolResultCode: number | null;
  retriable: boolean;
  requestId: bigint;
  reason: string;
}
export type Operation = "agent.register" | "availability.fetch" | "budget.create" | "budget.fund" | "budget.list" | "budget.reconciliation" | "budget.revoke" | "capability.attenuate" | "capability.create" | "capability.list" | "capability.revoke" | "export.offline" | "prepare" | "project" | "read.account" | "read.balance" | "read.batch" | "read.checkpoint" | "read.history" | "read.module_state" | "read.proof_bundle" | "session.close" | "session.list" | "session.open" | "session.refresh" | "sign" | "submit" | "subscription.acknowledge" | "subscription.create" | "subscription.delete" | "subscription.health" | "subscription.list" | "subscription.pause" | "subscription.resume" | "track" | "wait";

export interface Transport {
  call<TRequest, TResponse>(operation: Operation, request: TRequest): Promise<TResponse>;
}

export class Client {
  public constructor(private readonly transport: Transport) {}

  public call<TRequest, TResponse>(operation: Operation, request: TRequest): Promise<TResponse> {
    return this.transport.call<TRequest, TResponse>(operation, request);
  }
}
