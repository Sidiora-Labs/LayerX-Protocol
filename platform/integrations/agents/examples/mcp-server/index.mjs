import { createServer } from "node:http";
import { MAXIMUM_WEBHOOK_BYTES, readWebhookListener } from "@sidiora/layerx-agent-integrations";
import { createMcpIntegration } from "@sidiora/layerx-agent-integrations/mcp";

const integration = createMcpIntegration({ environment: process.env });
const mcp = integration.server;
const listener = readWebhookListener(process.env);

const readBody = (request) => new Promise((resolve, reject) => {
  const chunks = [];
  let total = 0;
  request.on("data", (chunk) => {
    total += chunk.length;
    if (total > MAXIMUM_WEBHOOK_BYTES) {
      reject(new Error("payload_too_large"));
      request.destroy();
      return;
    }
    chunks.push(chunk);
  });
  request.on("end", () => resolve(Buffer.concat(chunks)));
  request.on("error", reject);
});

const send = (response, status, headers, body) => {
  response.writeHead(status, headers);
  response.end(body);
};

const webhookPath = process.env.LAYERX_WEBHOOK_PATH ?? "/webhooks/layerx";

const http = listener === undefined ? undefined : createServer((request, response) => {
  void (async () => {
    const path = (request.url ?? "").split("?", 1)[0];
    if (request.method !== "POST" || path !== webhookPath) {
      send(response, 404, { "content-type": "application/json" }, JSON.stringify({ error: "not_found" }));
      return;
    }
    let body;
    try {
      body = await readBody(request);
    } catch {
      send(response, 413, { "content-type": "application/json" }, JSON.stringify({ error: "payload_too_large" }));
      return;
    }
    const outcome = await mcp.deliver(new Uint8Array(body), request.headers);
    send(response, outcome.status, outcome.headers, outcome.body);
  })().catch(() => {
    send(response, 500, { "content-type": "application/json" }, JSON.stringify({ error: "internal_error" }));
  });
});

const writeLine = (stream) => (line) => new Promise((resolve, reject) => {
  stream.write(line, (error) => (error === null || error === undefined ? resolve() : reject(error)));
});

const shutdown = () => {
  integration.destroy();
  if (http === undefined) {
    process.exit(0);
    return;
  }
  http.close(() => process.exit(0));
};

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, shutdown);
}

if (http !== undefined) {
  await new Promise((resolve) => http.listen(listener.port, listener.host, resolve));
  await writeLine(process.stderr)(JSON.stringify({
    webhookListener: `http://${listener.host}:${listener.port}${webhookPath}`,
    tools: mcp.tools.map((tool) => tool.name),
  }) + "\n");
}

try {
  await mcp.serve({ input: process.stdin, write: writeLine(process.stdout) });
} finally {
  integration.destroy();
  http?.close();
}
