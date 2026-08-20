import {
  createAgentIntegration,
  type AgentIntegrationOptions,
  type LayerXAgentIntegration,
} from "./integration.js";
import { renderOutcome, type AgentToolExecutor, type ToolDefinition, type ToolJsonObject } from "./tools.js";

export interface VercelAiTool {
  readonly type: "function";
  readonly description: string;
  readonly parameters: ToolJsonObject;
  readonly inputSchema: ToolJsonObject;
  execute(input: unknown): Promise<ToolJsonObject>;
}

export type VercelAiToolSet = Readonly<Record<string, VercelAiTool>>;

export interface LayerXVercelAiIntegration extends LayerXAgentIntegration {
  readonly vercelAiTools: VercelAiToolSet;
}

export function vercelAiTool(executor: AgentToolExecutor, definition: ToolDefinition): VercelAiTool {
  return {
    type: "function",
    description: definition.description,
    parameters: definition.inputSchema,
    inputSchema: definition.inputSchema,
    execute: async (input) => renderOutcome(await executor.execute(definition.name, input ?? {})),
  };
}

export function vercelAiToolSet(executor: AgentToolExecutor): VercelAiToolSet {
  const tools: Record<string, VercelAiTool> = {};
  for (const definition of executor.definitions) {
    tools[definition.name] = vercelAiTool(executor, definition);
  }
  return tools;
}

export function createVercelAiIntegration(options: AgentIntegrationOptions): LayerXVercelAiIntegration {
  const integration = createAgentIntegration(options);
  return {
    ...integration,
    vercelAiTools: vercelAiToolSet(integration.tools),
  };
}
