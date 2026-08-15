// Generated from the LayerX Agent API schema. Do not hand-edit.

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

export interface IdempotentMutation<T> {
  requestId: bigint;
  key: Uint8Array;
  bodyDigest: Uint8Array;
  operation: T;
}

export type ErrorClass = {{ERRORS}};
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
