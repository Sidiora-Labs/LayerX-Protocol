#!/usr/bin/env node
import { AgentIntegrationError } from "./config.js";
import { createMcpIntegration } from "./mcp.js";

interface NodeStream {
  write(chunk: string, callback: (error?: Error | null) => void): boolean;
}

interface NodeProcess {
  readonly env: Readonly<Record<string, string | undefined>>;
  readonly stdin: AsyncIterable<Uint8Array>;
  readonly stdout: NodeStream;
  readonly stderr: NodeStream;
  exitCode: number;
}

function runtime(): NodeProcess {
  const scope = globalThis as { readonly process?: NodeProcess };
  if (scope.process === undefined) {
    throw new AgentIntegrationError("client-runtime-refused");
  }
  return scope.process;
}

function writer(stream: NodeStream): (line: string) => Promise<void> {
  return (line) => new Promise<void>((resolve, reject) => {
    stream.write(line, (error) => {
      if (error === null || error === undefined) {
        resolve();
        return;
      }
      reject(error);
    });
  });
}

function describeFailure(error: unknown): string {
  if (error instanceof AgentIntegrationError) {
    return error.code;
  }
  if (error instanceof Error) {
    return error.name;
  }
  return "unknown-failure";
}

async function main(): Promise<void> {
  const host = runtime();
  const integration = createMcpIntegration({ environment: host.env });
  try {
    await integration.server.serve({ input: host.stdin, write: writer(host.stdout) });
  } finally {
    integration.destroy();
  }
}

main().catch((error: unknown) => {
  const host = runtime();
  void writer(host.stderr)(`layerx-mcp-server: ${describeFailure(error)}\n`);
  host.exitCode = 1;
});
