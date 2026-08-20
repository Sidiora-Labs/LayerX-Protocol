import { AGENT_FRAMEWORKS } from "@sidiora/layerx-agent-integrations";
import { createAnthropicIntegration } from "@sidiora/layerx-agent-integrations/anthropic";
import { createLangChainIntegration } from "@sidiora/layerx-agent-integrations/langchain";
import { createOpenAiIntegration } from "@sidiora/layerx-agent-integrations/openai";
import { createVercelAiIntegration } from "@sidiora/layerx-agent-integrations/vercel-ai";

const required = (name) => {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`missing_${name.toLowerCase()}`);
  return value;
};

const object = (value) => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) throw new Error("invalid_json_object");
  return value;
};

const framework = process.env.LAYERX_AGENT_FRAMEWORK ?? "openai";
if (!AGENT_FRAMEWORKS.includes(framework) || framework === "mcp") {
  throw new Error("unsupported_framework");
}

const options = { environment: process.env };
const integrations = {
  openai: () => {
    const integration = createOpenAiIntegration(options);
    return {
      integration,
      tools: integration.openAiTools.map((tool) => tool.function.name),
      async spend(input) {
        const [message] = await integration.handleToolCalls([
          { id: "call_layerx_spend", function: { name: "layerx_spend", arguments: JSON.stringify(input) } },
        ]);
        return object(JSON.parse(message.content));
      },
    };
  },
  anthropic: () => {
    const integration = createAnthropicIntegration(options);
    return {
      integration,
      tools: integration.anthropicTools.map((tool) => tool.name),
      async spend(input) {
        const [block] = await integration.handleToolUseBlocks([
          { type: "tool_use", id: "toolu_layerx_spend", name: "layerx_spend", input },
        ]);
        return object(JSON.parse(block.content));
      },
    };
  },
  langchain: () => {
    const integration = createLangChainIntegration(options);
    return {
      integration,
      tools: integration.langChainTools.map((tool) => tool.name),
      async spend(input) {
        return object(JSON.parse(await integration.langChainTool("layerx_spend").invoke(input)));
      },
    };
  },
  "vercel-ai": () => {
    const integration = createVercelAiIntegration(options);
    return {
      integration,
      tools: Object.keys(integration.vercelAiTools),
      async spend(input) {
        return object(await integration.vercelAiTools["layerx_spend"].execute(input));
      },
    };
  },
};

const runtime = integrations[framework]();
const report = { framework, tools: runtime.tools };

const deliveries = [];
const handler = {
  async handle(event, deliveryId) {
    deliveries.push({ deliveryId, kind: typeof event.type === "string" ? event.type : "unknown" });
  },
};

try {
  const input = object(JSON.parse(required("LAYERX_SPEND_TOOL_INPUT_JSON")));
  report.spend = await runtime.spend(input);
  if (report.spend.ok !== true) process.exitCode = 2;

  const encoded = process.env.LAYERX_WEBHOOK_DELIVERY_JSON;
  if (encoded !== undefined && encoded.length > 0) {
    const delivery = object(JSON.parse(encoded));
    if (typeof delivery.body !== "string") throw new Error("invalid_webhook_delivery");
    const body = Uint8Array.from(Buffer.from(delivery.body, "base64"));
    const headers = object(delivery.headers);
    const first = await runtime.integration.webhooks.respond(body, headers, handler);
    const second = await runtime.integration.webhooks.respond(body, headers, handler);
    report.webhook = {
      first: { status: first.status, body: JSON.parse(first.body) },
      second: { status: second.status, body: JSON.parse(second.body) },
      handled: deliveries,
    };
    if (first.status !== 200 || second.status !== 200) process.exitCode = 3;
    if (deliveries.length !== 1) process.exitCode = 4;
  }

  process.stdout.write(JSON.stringify(report) + "\n");
} finally {
  runtime.integration.destroy();
}
