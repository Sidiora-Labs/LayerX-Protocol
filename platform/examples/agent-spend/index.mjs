import { AgentMiddleware } from "@sidiora/layerx-agent-middleware";
import { PlatformSdkError, ProductionClient, SecretBytes } from "@sidiora/layerx-sdk";

const required = (name) => {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`missing_${name.toLowerCase()}`);
  return value;
};

const object = (value) => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) throw new Error("invalid_service_response");
  return value;
};

const hex32 = (value) => {
  if (typeof value !== "string" || !/^[0-9a-fA-F]{64}$/.test(value)) throw new Error("invalid_service_response");
  return Uint8Array.from({ length: 32 }, (_, index) => Number.parseInt(value.slice(index * 2, index * 2 + 2), 16));
};

const endpoint = (name) => {
  const url = new URL(required(name));
  if (url.protocol !== "https:" && url.hostname !== "127.0.0.1" && url.hostname !== "localhost") throw new Error(`insecure_${name.toLowerCase()}`);
  return url;
};

class ServiceClient {
  constructor(url, token) {
    this.url = url;
    this.token = token;
  }

  async call(payload, idempotencyKey) {
    const headers = new Headers({ accept: "application/json", "content-type": "application/json" });
    this.token.withBytes((bytes) => headers.set("authorization", `Bearer ${new TextDecoder("utf-8", { fatal: true }).decode(bytes)}`));
    if (idempotencyKey !== undefined) headers.set("idempotency-key", idempotencyKey);
    let response;
    try {
      response = await fetch(this.url, { method: "POST", headers, body: JSON.stringify(payload) });
    } catch {
      throw new PlatformSdkError({ code: "transport-failure", retry: "safe" });
    }
    const body = await response.json().catch(() => undefined);
    if (!response.ok) {
      const failure = body === undefined ? {} : object(body);
      throw new PlatformSdkError({
        code: typeof failure.code === "string" ? failure.code : "transport-failure",
        retry: typeof failure.retry === "string" ? failure.retry : response.status >= 500 ? "unknown-outcome" : "never",
        ...(Number.isSafeInteger(failure.retry_after_ms) ? { retryAfterMs: failure.retry_after_ms } : {}),
      });
    }
    return body;
  }
}

class AgentTransport {
  constructor(service) { this.service = service; }
  async call(call) {
    if (call.plane !== "agent") throw new PlatformSdkError({ code: "unavailable-capability", retry: "never" });
    const response = object(await this.service.call({ operation: call.operation, request: call.request }, call.idempotencyKey));
    if (response.ok !== true || response.result === undefined) throw new PlatformSdkError({ code: "decode-failure", retry: "never" });
    return response.result;
  }
}

class BudgetLedger {
  constructor(service) { this.service = service; }
  reserve(request) { return this.service.call({ action: "reserve", request }, request.idempotencyKey); }
  hold(reservationId, requestDigest, approvalId) { return this.service.call({ action: "hold", reservation_id: reservationId, request_digest: requestDigest, approval_id: approvalId }); }
  commit(reservationId, requestDigest, receiptDigest) { return this.service.call({ action: "commit", reservation_id: reservationId, request_digest: requestDigest, receipt_digest: receiptDigest }); }
  release(reservationId, requestDigest) { return this.service.call({ action: "release", reservation_id: reservationId, request_digest: requestDigest }); }
}

class RemoteSigner {
  constructor(service) { this.service = service; }
  async sign(prepared) {
    const response = object(await this.service.call({ signing_preimage: prepared.signing_preimage, disclosure: prepared.disclosure }));
    if (typeof response.signature !== "string") throw new Error("invalid_service_response");
    return response.signature;
  }
}

class ReceiptResolver {
  constructor(service) { this.service = service; }
  async resolve(receiptRef) {
    const response = object(await this.service.call({ receipt_ref: receiptRef }));
    const batch = object(response.authorized_batch);
    if (typeof response.canonical_receipt_base64 !== "string") throw new Error("invalid_service_response");
    return {
      canonicalReceipt: Uint8Array.from(Buffer.from(response.canonical_receipt_base64, "base64")),
      authorizedBatch: {
        batchId: hex32(batch.batch_id),
        asset: hex32(batch.asset),
        previousStateRoot: hex32(batch.previous_state_root),
        resultingStateRoot: hex32(batch.resulting_state_root),
        sequencerPublicKey: hex32(batch.sequencer_public_key),
      },
    };
  }
}

const token = new SecretBytes(new TextEncoder().encode(required("LAYERX_TOKEN")));
const agentService = new ServiceClient(endpoint("LAYERX_AGENT_RPC_URL"), token);
const middleware = new AgentMiddleware({
  client: new ProductionClient(new AgentTransport(agentService)),
  budgets: new BudgetLedger(new ServiceClient(endpoint("LAYERX_BUDGET_SERVICE_URL"), token)),
  signer: new RemoteSigner(new ServiceClient(endpoint("LAYERX_SIGNER_SERVICE_URL"), token)),
  receipts: new ReceiptResolver(new ServiceClient(endpoint("LAYERX_RECEIPT_SERVICE_URL"), token)),
});

try {
  const request = object(JSON.parse(required("LAYERX_SPEND_REQUEST_JSON")));
  const result = await middleware.spend(request);
  process.stdout.write(JSON.stringify({
    kind: result.kind,
    ...(result.kind === "verified" ? { receiptDigest: Buffer.from(result.verification.receiptDigest).toString("hex") } : {}),
    ...(result.kind === "approval-hold" ? { approvalId: result.approval.approvalId } : {}),
  }) + "\n");
  if (result.kind !== "verified" && result.kind !== "approval-hold" && result.kind !== "pending") process.exitCode = 2;
} finally {
  token.destroy();
}
