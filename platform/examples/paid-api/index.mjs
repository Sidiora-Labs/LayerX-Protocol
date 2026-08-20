import { createServer } from "node:http";
import { mkdir, open, readFile } from "node:fs/promises";
import { join } from "node:path";
import {
  MiddlewareError,
  PAYMENT_SIGNATURE_HEADER,
  ReceiptPayloadAuthority,
  SellerMiddleware,
} from "@sidiora/layerx-seller-middleware";

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

class FileFulfillmentRepository {
  constructor(directory) {
    this.directory = directory;
  }

  async fulfill(proposed, release) {
    await mkdir(this.directory, { recursive: true, mode: 0o700 });
    const path = join(this.directory, `${proposed.idempotencyKey}.json`);
    try {
      return await this.read(path, proposed);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    const resource = await release();
    const record = JSON.stringify({
      requestDigest: proposed.requestDigest,
      receipt: Buffer.from(proposed.canonicalReceipt).toString("base64"),
      resource,
    });
    let file;
    try {
      file = await open(path, "wx", 0o600);
      await file.writeFile(record, "utf8");
      await file.sync();
      return { ...proposed, resource };
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
      return await this.read(path, proposed);
    } finally {
      await file?.close();
    }
  }

  async read(path, proposed) {
    const stored = JSON.parse(await readFile(path, "utf8"));
    const canonicalReceipt = Uint8Array.from(Buffer.from(stored.receipt, "base64"));
    if (stored.requestDigest !== proposed.requestDigest) {
      throw new MiddlewareError("fulfillment-conflict");
    }
    return {
      ...proposed,
      canonicalReceipt,
      resource: stored.resource,
    };
  }
}

const resourceBody = await readFile(required("LAYERX_RESOURCE_FILE"), "utf8");
const batches = { async resolve() { return authorizedBatch; } };
const middleware = new SellerMiddleware({
  paymentRequired: {
    x402Version: 2,
    resource: {
      url: required("LAYERX_RESOURCE_URL"),
      description: required("LAYERX_RESOURCE_DESCRIPTION"),
      mimeType: "application/json",
    },
    accepts: [{
      scheme: required("LAYERX_X402_SCHEME"),
      network: required("LAYERX_X402_NETWORK"),
      amount: required("LAYERX_PRICE"),
      asset: required("LAYERX_ASSET"),
      payTo: required("LAYERX_PAY_TO"),
      maxTimeoutSeconds: Number(required("LAYERX_PAYMENT_TIMEOUT_SECONDS")),
    }],
  },
  authority: new ReceiptPayloadAuthority(batches),
  fulfillments: new FileFulfillmentRepository(process.env.LAYERX_FULFILLMENT_DIR ?? "./fulfillments"),
});

const port = Number(process.env.PORT ?? "8080");
if (!Number.isSafeInteger(port) || port <= 0 || port > 65535) throw new Error("invalid_port");

createServer(async (request, response) => {
  try {
    if (request.method !== "GET" || request.url !== "/paid") {
      response.writeHead(404).end();
      return;
    }
    const header = request.headers[PAYMENT_SIGNATURE_HEADER.toLowerCase()];
    const paymentHeader = Array.isArray(header) ? undefined : header;
    const decision = await middleware.handle("public-paid-api", paymentHeader, async () => resourceBody);
    response.writeHead(decision.status, decision.kind === "payment-required" || decision.kind === "refused" || decision.kind === "released"
      ? decision.headers
      : { "retry-after": "1" });
    if (decision.kind === "payment-required") {
      response.end(JSON.stringify(decision.body));
    } else if (decision.kind === "released") {
      response.end(decision.resource);
    } else {
      response.end();
    }
  } catch (error) {
    const code = error instanceof MiddlewareError ? error.code : "internal-fault";
    response.writeHead(400, { "content-type": "application/json" }).end(JSON.stringify({ error: code }));
  }
}).listen(port, "127.0.0.1", () => {
  process.stdout.write(JSON.stringify({ listening: port, path: "/paid" }) + "\n");
});
