import type {
  AgentBudgetLedger,
  AgentReceiptEvidence,
  AgentReceiptResolver,
  AgentSigner,
  BudgetReservation,
  BudgetReserveResult,
  PreparedActivity,
} from "@sidiora/layerx-agent-middleware";
import {
  PlatformSdkError,
  SDK_ERROR_CODES,
  type ProductionTransport,
  type SdkErrorCode,
  type SecretBytes,
  type TransportCall,
} from "@sidiora/layerx-sdk";
import { AgentIntegrationError, parseHex32 } from "./config.js";

const MAXIMUM_RESPONSE_BYTES = 8 * 1024 * 1024;

export interface ServiceEndpointConfig {
  readonly url: string;
  readonly token: SecretBytes;
  readonly timeoutMs: number;
  readonly fetch?: typeof globalThis.fetch;
}

export class LayerXServiceEndpoint {
  readonly #url: string;
  readonly #token: SecretBytes;
  readonly #timeoutMs: number;
  readonly #fetch: typeof globalThis.fetch;

  public constructor(config: ServiceEndpointConfig) {
    this.#url = config.url;
    this.#token = config.token;
    this.#timeoutMs = config.timeoutMs;
    this.#fetch = config.fetch ?? globalThis.fetch;
  }

  public async call(payload: unknown, idempotencyKey?: string): Promise<unknown> {
    const headers = new Headers({ accept: "application/json", "content-type": "application/json" });
    this.#token.withBytes((bytes) => {
      headers.set("authorization", `Bearer ${new TextDecoder("utf-8", { fatal: true }).decode(bytes)}`);
    });
    if (idempotencyKey !== undefined) {
      headers.set("idempotency-key", idempotencyKey);
    }
    let response: Response;
    try {
      response = await this.#fetch(this.#url, {
        method: "POST",
        headers,
        body: JSON.stringify(payload),
        signal: AbortSignal.timeout(this.#timeoutMs),
      });
    } catch {
      throw new PlatformSdkError({ code: "transport-failure", retry: "safe" });
    }
    const text = await readBoundedText(response);
    let body: unknown;
    try {
      body = text.length === 0 ? undefined : JSON.parse(text);
    } catch {
      throw new PlatformSdkError({ code: "decode-failure", retry: "never" });
    }
    if (!response.ok) {
      throw serviceError(response.status, body);
    }
    return body;
  }
}

export class LayerXAgentTransport implements ProductionTransport {
  readonly #endpoint: LayerXServiceEndpoint;

  public constructor(endpoint: LayerXServiceEndpoint) {
    this.#endpoint = endpoint;
  }

  public async call<TRequest, TResponse>(call: TransportCall<TRequest>): Promise<TResponse> {
    if (call.plane !== "agent") {
      throw new PlatformSdkError({ code: "unavailable-capability", retry: "never" });
    }
    const envelope = asObject(await this.#endpoint.call(
      { operation: call.operation, request: call.request },
      call.idempotencyKey,
    ));
    if (envelope["ok"] !== true || envelope["result"] === undefined) {
      throw new PlatformSdkError({ code: "decode-failure", retry: "never" });
    }
    return envelope["result"] as TResponse;
  }
}

export class LayerXBudgetLedger implements AgentBudgetLedger {
  readonly #endpoint: LayerXServiceEndpoint;

  public constructor(endpoint: LayerXServiceEndpoint) {
    this.#endpoint = endpoint;
  }

  public async reserve(request: {
    readonly tenant: string;
    readonly idempotencyKey: string;
    readonly requestDigest: string;
    readonly amount: string;
    readonly asset: string;
  }): Promise<BudgetReserveResult> {
    const response = asObject(await this.#endpoint.call({
      action: "reserve",
      tenant: request.tenant,
      idempotency_key: request.idempotencyKey,
      request_digest: request.requestDigest,
      amount: request.amount,
      asset: request.asset,
    }, request.idempotencyKey));
    const kind = response["kind"];
    if (kind === "conflict") {
      return { kind: "conflict" };
    }
    if (kind === "exhausted") {
      return { kind: "exhausted", available: canonicalAmount(response["available"]) };
    }
    if (kind === "reserved") {
      return { kind: "reserved", reservation: parseReservation(response["reservation"]) };
    }
    throw new AgentIntegrationError("service-refused");
  }

  public async hold(reservationId: string, requestDigest: string, approvalId: string): Promise<BudgetReservation> {
    return parseReservation(await this.#endpoint.call({
      action: "hold",
      reservation_id: reservationId,
      request_digest: requestDigest,
      approval_id: approvalId,
    }));
  }

  public async commit(reservationId: string, requestDigest: string, receiptDigest: string): Promise<BudgetReservation> {
    return parseReservation(await this.#endpoint.call({
      action: "commit",
      reservation_id: reservationId,
      request_digest: requestDigest,
      receipt_digest: receiptDigest,
    }));
  }

  public async release(reservationId: string, requestDigest: string): Promise<BudgetReservation> {
    return parseReservation(await this.#endpoint.call({
      action: "release",
      reservation_id: reservationId,
      request_digest: requestDigest,
    }));
  }
}

export class LayerXRemoteSigner implements AgentSigner {
  readonly #endpoint: LayerXServiceEndpoint;

  public constructor(endpoint: LayerXServiceEndpoint) {
    this.#endpoint = endpoint;
  }

  public async sign(prepared: PreparedActivity): Promise<string> {
    const response = asObject(await this.#endpoint.call({
      preparation_ref: prepared.preparation_ref,
      signing_preimage: prepared.signing_preimage,
      disclosure: prepared.disclosure,
      expiry: prepared.expiry,
    }));
    const signature = response["signature"];
    if (typeof signature !== "string" || signature.length === 0 || signature.length > 16_384) {
      throw new AgentIntegrationError("service-refused");
    }
    return signature;
  }
}

export class LayerXReceiptResolver implements AgentReceiptResolver {
  readonly #endpoint: LayerXServiceEndpoint;

  public constructor(endpoint: LayerXServiceEndpoint) {
    this.#endpoint = endpoint;
  }

  public async resolve(receiptRef: string): Promise<AgentReceiptEvidence> {
    const response = asObject(await this.#endpoint.call({ receipt_ref: receiptRef }));
    const batch = asObject(response["authorized_batch"]);
    const encoded = response["canonical_receipt_base64"];
    if (typeof encoded !== "string" || encoded.length === 0 || encoded.length > 2_796_208) {
      throw new AgentIntegrationError("service-refused");
    }
    return {
      canonicalReceipt: decodeBase64(encoded),
      authorizedBatch: {
        batchId: hexField(batch, "batch_id"),
        asset: hexField(batch, "asset"),
        previousStateRoot: hexField(batch, "previous_state_root"),
        resultingStateRoot: hexField(batch, "resulting_state_root"),
        sequencerPublicKey: hexField(batch, "sequencer_public_key"),
      },
    };
  }
}

async function readBoundedText(response: Response): Promise<string> {
  const body = response.body;
  if (body === null) {
    return "";
  }
  const reader = body.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;
  for (let step = await reader.read(); step.done !== true; step = await reader.read()) {
    const chunk = step.value;
    total += chunk.length;
    if (total > MAXIMUM_RESPONSE_BYTES) {
      await reader.cancel();
      throw new PlatformSdkError({ code: "decode-failure", retry: "never" });
    }
    chunks.push(chunk);
  }
  const merged = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    merged.set(chunk, offset);
    offset += chunk.length;
  }
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(merged);
  } catch {
    throw new PlatformSdkError({ code: "decode-failure", retry: "never" });
  }
}

function serviceError(status: number, body: unknown): PlatformSdkError {
  const failure = body === null || typeof body !== "object" || Array.isArray(body)
    ? {}
    : body as Record<string, unknown>;
  const code = typeof failure["code"] === "string" ? failure["code"] : undefined;
  const retry = typeof failure["retry"] === "string" ? failure["retry"] : undefined;
  const retryAfter = failure["retry_after_ms"];
  return new PlatformSdkError({
    code: isSdkErrorCode(code) ? code : status >= 500 ? "internal-fault" : "core-rejection",
    retry: retry === "safe" || retry === "after" || retry === "unknown-outcome" || retry === "never"
      ? retry
      : status >= 500 ? "unknown-outcome" : "never",
    ...(typeof retryAfter === "number" && Number.isSafeInteger(retryAfter) && retryAfter >= 0
      ? { retryAfterMs: retryAfter }
      : {}),
  });
}

const SDK_ERROR_CODE_VALUES: ReadonlySet<string> = new Set<string>(SDK_ERROR_CODES);

function isSdkErrorCode(value: string | undefined): value is SdkErrorCode {
  return value !== undefined && SDK_ERROR_CODE_VALUES.has(value);
}

function parseReservation(value: unknown): BudgetReservation {
  const object = asObject(value);
  const state = object["state"];
  if (state !== "reserved" && state !== "held" && state !== "committed" && state !== "released") {
    throw new AgentIntegrationError("service-refused");
  }
  return {
    reservationId: boundedField(object, "reservation_id", 512),
    requestDigest: hexTextField(object, "request_digest"),
    amount: canonicalAmount(object["amount"]),
    asset: hexTextField(object, "asset"),
    state,
  };
}

function boundedField(object: Readonly<Record<string, unknown>>, name: string, maximum: number): string {
  const value = object[name];
  if (typeof value !== "string" || value.length === 0 || value.length > maximum || value.includes("\0")) {
    throw new AgentIntegrationError("service-refused");
  }
  return value;
}

function hexTextField(object: Readonly<Record<string, unknown>>, name: string): string {
  const value = object[name];
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    throw new AgentIntegrationError("service-refused");
  }
  return value;
}

function hexField(object: Readonly<Record<string, unknown>>, name: string): Uint8Array {
  const value = object[name];
  if (typeof value !== "string") {
    throw new AgentIntegrationError("service-refused");
  }
  try {
    return parseHex32(value.toLowerCase());
  } catch {
    throw new AgentIntegrationError("service-refused");
  }
}

function canonicalAmount(value: unknown): string {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]*)$/u.test(value) || value.length > 39) {
    throw new AgentIntegrationError("service-refused");
  }
  return value;
}

function decodeBase64(value: string): Uint8Array {
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value)) {
    throw new AgentIntegrationError("service-refused");
  }
  const binary = globalThis.atob(value);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function asObject(value: unknown): Readonly<Record<string, unknown>> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new AgentIntegrationError("service-refused");
  }
  return value as Readonly<Record<string, unknown>>;
}
