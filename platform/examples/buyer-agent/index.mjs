import {
  BuyerMiddleware,
  LayerXPaymentHttpTransport,
} from "@sidiora/layerx-buyer-middleware";
import { ProductionClient, SecretBytes } from "@sidiora/layerx-sdk";

const required = (name) => {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`missing_${name.toLowerCase()}`);
  return value;
};

const hex32 = (value) => {
  const digits = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]{64}$/.test(digits)) throw new Error("invalid_authorized_batch");
  return Uint8Array.from({ length: 32 }, (_, index) => Number.parseInt(digits.slice(index * 2, index * 2 + 2), 16));
};

const authorization = JSON.parse(required("LAYERX_AUTHORIZED_BATCH_JSON"));
const authorizedBatch = {
  batchId: hex32(authorization.batchId),
  asset: hex32(authorization.asset),
  previousStateRoot: hex32(authorization.previousStateRoot),
  resultingStateRoot: hex32(authorization.resultingStateRoot),
  sequencerPublicKey: hex32(authorization.sequencerPublicKey),
};
const token = new SecretBytes(new TextEncoder().encode(required("LAYERX_TOKEN")));
const transport = new LayerXPaymentHttpTransport({
  baseUrl: required("LAYERX_HUMAN_URL"),
  bearerToken: token,
});
const buyer = new BuyerMiddleware({
  client: new ProductionClient(transport),
  source: required("LAYERX_SOURCE"),
  supported: [{
    scheme: required("LAYERX_X402_SCHEME"),
    network: required("LAYERX_X402_NETWORK"),
  }],
  authorizedBatches: { async resolve() { return authorizedBatch; } },
});

try {
  const result = await buyer.fetch(
    required("LAYERX_RESOURCE_URL"),
    { method: "GET", headers: { accept: "application/json" } },
    required("LAYERX_IDEMPOTENCY_KEY"),
  );
  if (result.kind !== "paid" && result.kind !== "not-payment-required") {
    throw new Error(`payment_${result.kind}`);
  }
  const response = result.response;
  const body = await response.text();
  process.stdout.write(JSON.stringify({
    status: response.status,
    payment: result.kind,
    ...(result.kind === "paid" ? { receiptDigest: result.payment.receiptDigest } : {}),
    body,
  }) + "\n");
} finally {
  token.destroy();
}
