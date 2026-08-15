// Generated from the LayerX Agent API schema. Do not hand-edit.

const PACKAGE_METADATA = Object.freeze({
  name: "@sidiora/layerx-sdk",
  version: "0.1.0",
  contractMajor: 1,
});

export function layerx_sdk_ts_package(): typeof PACKAGE_METADATA {
  return PACKAGE_METADATA;
}

{{SCALARS}}

export enum VerificationLevel {
{{LEVELS}}
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

export type ErrorClass = {{ERRORS}};
export interface ApiError {
  errorClass: ErrorClass;
  protocolResultCode: number | null;
  retriable: boolean;
  requestId: bigint;
  reason: string;
}
export type Operation = {{OPERATIONS}};

export interface Transport {
  call<TRequest, TResponse>(operation: Operation, request: TRequest): Promise<TResponse>;
}

export class Client {
  public constructor(private readonly transport: Transport) {}

  public call<TRequest, TResponse>(operation: Operation, request: TRequest): Promise<TResponse> {
    return this.transport.call<TRequest, TResponse>(operation, request);
  }
}
