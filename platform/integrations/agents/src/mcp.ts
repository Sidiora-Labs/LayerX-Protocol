import type { JsonValue } from "@sidiora/layerx-seller-middleware";
import { AgentIntegrationError } from "./config.js";
import {
  createAgentIntegration,
  type AgentIntegrationOptions,
  type LayerXAgentIntegration,
} from "./integration.js";
import type { ToolDefinition, ToolJson, ToolJsonObject } from "./tools.js";
import type { AgentWebhookResponse, WebhookHeaderSource } from "./webhooks.js";

export const MCP_PROTOCOL_VERSION = "2025-06-18" as const;

export const SUPPORTED_MCP_PROTOCOL_VERSIONS: readonly string[] = [
  "2025-06-18",
  "2025-03-26",
  "2024-11-05",
];

export const MAXIMUM_MCP_LINE_BYTES = 8 * 1024 * 1024;

export const MAXIMUM_BUFFERED_EVENTS = 256;

export const EVENTS_TOOL: ToolDefinition = {
  name: "layerx_events",
  title: "Read verified LayerX events",
  description:
    "Return LayerX webhook events delivered to this server that passed Ed25519 signature verification and "
    + "replay protection. Deliveries that fail verification are rejected and never reach this queue.",
  inputSchema: {
    type: "object",
    additionalProperties: false,
    required: [],
    properties: {
      limit: {
        type: "integer",
        minimum: 1,
        maximum: MAXIMUM_BUFFERED_EVENTS,
        description: "Maximum number of verified events to drain; defaults to all buffered events.",
      },
    },
  },
};

export type JsonRpcId = string | number | null;

export interface JsonRpcSuccess {
  readonly jsonrpc: "2.0";
  readonly id: JsonRpcId;
  readonly result: ToolJsonObject;
}

export interface JsonRpcFailure {
  readonly jsonrpc: "2.0";
  readonly id: JsonRpcId;
  readonly error: {
    readonly code: number;
    readonly message: string;
  };
}

export type JsonRpcResponse = JsonRpcSuccess | JsonRpcFailure;

export const PARSE_ERROR = -32700;
export const INVALID_REQUEST = -32600;
export const METHOD_NOT_FOUND = -32601;
export const INVALID_PARAMS = -32602;
export const INTERNAL_ERROR = -32603;

export interface McpStreamTransport {
  readonly input: AsyncIterable<Uint8Array | string>;
  write(line: string): void | Promise<void>;
}

export interface LayerXMcpServerConfig {
  readonly integration: LayerXAgentIntegration;
  readonly name?: string;
  readonly version?: string;
  readonly bufferedEvents?: number;
}

export class VerifiedEventQueue {
  readonly #capacity: number;
  readonly #entries: { deliveryId: string; event: Readonly<Record<string, JsonValue>> }[] = [];

  public constructor(capacity: number = MAXIMUM_BUFFERED_EVENTS) {
    if (!Number.isSafeInteger(capacity) || capacity < 1) {
      throw new AgentIntegrationError("invalid-declared-key");
    }
    this.#capacity = capacity;
  }

  public get size(): number {
    return this.#entries.length;
  }

  public push(deliveryId: string, event: Readonly<Record<string, JsonValue>>): void {
    this.#entries.push({ deliveryId, event });
    while (this.#entries.length > this.#capacity) {
      this.#entries.shift();
    }
  }

  public drain(limit: number): readonly { deliveryId: string; event: Readonly<Record<string, JsonValue>> }[] {
    return this.#entries.splice(0, Math.max(0, Math.min(limit, this.#entries.length)));
  }
}

export class LayerXMcpServer {
  readonly #integration: LayerXAgentIntegration;
  readonly #queue: VerifiedEventQueue;
  readonly #name: string;
  readonly #version: string;
  #initialized = false;

  public constructor(config: LayerXMcpServerConfig) {
    this.#integration = config.integration;
    this.#queue = new VerifiedEventQueue(config.bufferedEvents ?? MAXIMUM_BUFFERED_EVENTS);
    this.#name = config.name ?? "layerx";
    this.#version = config.version ?? "0.1.0";
  }

  public get integration(): LayerXAgentIntegration {
    return this.#integration;
  }

  public get events(): VerifiedEventQueue {
    return this.#queue;
  }

  public get tools(): readonly ToolDefinition[] {
    return [...this.#integration.tools.definitions, EVENTS_TOOL];
  }

  public deliver(rawBody: Uint8Array, headers: WebhookHeaderSource): Promise<AgentWebhookResponse> {
    return this.#integration.webhooks.respond(rawBody, headers, {
      handle: async (event, deliveryId) => {
        this.#queue.push(deliveryId, event);
      },
    });
  }

  public async handle(message: unknown): Promise<JsonRpcResponse | undefined> {
    if (!isObject(message) || message["jsonrpc"] !== "2.0" || typeof message["method"] !== "string") {
      return failure(identifier(message), INVALID_REQUEST, "invalid request");
    }
    const method = message["method"];
    const id = identifier(message);
    if (id === undefined) {
      if (method === "notifications/initialized") {
        this.#initialized = true;
      }
      return undefined;
    }
    try {
      return success(id, await this.#dispatch(method, message["params"]));
    } catch (error) {
      if (error instanceof McpMethodError) {
        return failure(id, error.rpcCode, error.message);
      }
      if (error instanceof AgentIntegrationError) {
        return failure(id, INVALID_PARAMS, error.code);
      }
      return failure(id, INTERNAL_ERROR, "internal error");
    }
  }

  public async serve(transport: McpStreamTransport): Promise<void> {
    const decoder = new TextDecoder();
    let buffer = "";
    for await (const chunk of transport.input) {
      buffer += typeof chunk === "string" ? chunk : decoder.decode(chunk, { stream: true });
      let newline = buffer.indexOf("\n");
      while (newline >= 0) {
        const line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        await this.#serveLine(line, transport);
        newline = buffer.indexOf("\n");
      }
      if (buffer.length > MAXIMUM_MCP_LINE_BYTES) {
        await transport.write(`${JSON.stringify(failure(null, PARSE_ERROR, "message too large"))}\n`);
        buffer = "";
      }
    }
    const trailing = buffer.trim();
    if (trailing.length > 0) {
      await this.#serveLine(trailing, transport);
    }
  }

  async #serveLine(line: string, transport: McpStreamTransport): Promise<void> {
    const text = line.trim();
    if (text.length === 0) {
      return;
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(text);
    } catch {
      await transport.write(`${JSON.stringify(failure(null, PARSE_ERROR, "parse error"))}\n`);
      return;
    }
    if (Array.isArray(parsed)) {
      const responses: JsonRpcResponse[] = [];
      for (const entry of parsed) {
        const response = await this.handle(entry);
        if (response !== undefined) {
          responses.push(response);
        }
      }
      if (responses.length > 0) {
        await transport.write(`${JSON.stringify(responses)}\n`);
      }
      return;
    }
    const response = await this.handle(parsed);
    if (response !== undefined) {
      await transport.write(`${JSON.stringify(response)}\n`);
    }
  }

  async #dispatch(method: string, params: unknown): Promise<ToolJsonObject> {
    if (method === "initialize") {
      return this.#initialize(params);
    }
    if (method === "ping") {
      return {};
    }
    if (method === "tools/list") {
      return { tools: this.tools.map(describeTool) };
    }
    if (method === "tools/call") {
      return this.#call(params);
    }
    if (method === "layerx/webhook") {
      return this.#webhook(params);
    }
    throw new McpMethodError(METHOD_NOT_FOUND, `unknown method ${method}`);
  }

  #initialize(params: unknown): ToolJsonObject {
    const requested = isObject(params) ? params["protocolVersion"] : undefined;
    const version = typeof requested === "string" && SUPPORTED_MCP_PROTOCOL_VERSIONS.includes(requested)
      ? requested
      : MCP_PROTOCOL_VERSION;
    return {
      protocolVersion: version,
      capabilities: { tools: { listChanged: false } },
      serverInfo: { name: this.#name, version: this.#version },
      instructions:
        "Every LayerX spend returns only after its protocol receipt has been verified locally. "
        + "Treat a refusal as final and never retry a spend with a fresh idempotency key.",
    };
  }

  async #call(params: unknown): Promise<ToolJsonObject> {
    if (!isObject(params) || typeof params["name"] !== "string") {
      throw new McpMethodError(INVALID_PARAMS, "tools/call requires a tool name");
    }
    const name = params["name"];
    const input = params["arguments"] ?? {};
    if (name === EVENTS_TOOL.name) {
      return content(this.#drain(input), false);
    }
    const outcome = await this.#integration.tools.execute(name, input);
    if (outcome.ok) {
      return content({ tool: outcome.tool, result: outcome.result }, false);
    }
    return content({ tool: outcome.tool, code: outcome.code }, true);
  }

  async #webhook(params: unknown): Promise<ToolJsonObject> {
    if (!isObject(params) || typeof params["body"] !== "string") {
      throw new McpMethodError(INVALID_PARAMS, "layerx/webhook requires a base64 body");
    }
    const headers = params["headers"];
    if (!isObject(headers)) {
      throw new McpMethodError(INVALID_PARAMS, "layerx/webhook requires delivery headers");
    }
    const normalized: Record<string, string> = {};
    for (const [name, value] of Object.entries(headers)) {
      if (typeof value !== "string") {
        throw new McpMethodError(INVALID_PARAMS, "delivery headers must be strings");
      }
      normalized[name.toLowerCase()] = value;
    }
    const response = await this.deliver(decodeBase64(params["body"]), normalized);
    return { status: response.status, body: response.body };
  }

  #drain(input: unknown): ToolJsonObject {
    const requested = isObject(input) ? input["limit"] : undefined;
    if (requested !== undefined && (typeof requested !== "number" || !Number.isSafeInteger(requested) || requested < 1)) {
      throw new McpMethodError(INVALID_PARAMS, "limit must be a positive integer");
    }
    const drained = this.#queue.drain(requested === undefined ? MAXIMUM_BUFFERED_EVENTS : requested);
    const events: ToolJson[] = drained.map((entry) => ({ deliveryId: entry.deliveryId, event: entry.event }));
    return {
      tool: EVENTS_TOOL.name,
      result: { events, remaining: this.#queue.size },
    };
  }

  public get initialized(): boolean {
    return this.#initialized;
  }
}

export interface LayerXMcpIntegration extends LayerXAgentIntegration {
  readonly server: LayerXMcpServer;
}

export function createMcpIntegration(options: AgentIntegrationOptions): LayerXMcpIntegration {
  const integration = createAgentIntegration(options);
  return {
    ...integration,
    server: new LayerXMcpServer({ integration }),
  };
}

class McpMethodError extends Error {
  public readonly rpcCode: number;

  public constructor(rpcCode: number, message: string) {
    super(message);
    this.name = "McpMethodError";
    this.rpcCode = rpcCode;
  }
}

function describeTool(definition: ToolDefinition): ToolJsonObject {
  return {
    name: definition.name,
    title: definition.title,
    description: definition.description,
    inputSchema: definition.inputSchema,
  };
}

function content(structured: ToolJsonObject, isError: boolean): ToolJsonObject {
  return {
    content: [{ type: "text", text: JSON.stringify(structured) }],
    structuredContent: structured,
    isError,
  };
}

function success(id: JsonRpcId, result: ToolJsonObject): JsonRpcSuccess {
  return { jsonrpc: "2.0", id, result };
}

function failure(id: JsonRpcId | undefined, code: number, message: string): JsonRpcFailure {
  return { jsonrpc: "2.0", id: id ?? null, error: { code, message } };
}

function identifier(message: unknown): JsonRpcId | undefined {
  if (!isObject(message)) {
    return null;
  }
  const id = message["id"];
  if (typeof id === "string" || (typeof id === "number" && Number.isFinite(id)) || id === null) {
    return id;
  }
  return undefined;
}

function decodeBase64(value: string): Uint8Array {
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value)) {
    throw new AgentIntegrationError("unverifiable-body");
  }
  const binary = globalThis.atob(value);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function isObject(value: unknown): value is Readonly<Record<string, unknown>> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
