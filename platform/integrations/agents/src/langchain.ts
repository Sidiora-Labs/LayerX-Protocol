import { AgentIntegrationError } from "./config.js";
import {
  createAgentIntegration,
  type AgentIntegrationOptions,
  type LayerXAgentIntegration,
} from "./integration.js";
import { renderOutcome, type AgentToolExecutor, type ToolDefinition, type ToolJsonObject } from "./tools.js";

export interface LangChainToolSpec {
  readonly name: string;
  readonly description: string;
  readonly schema: ToolJsonObject;
  readonly returnDirect: false;
  func(input: unknown): Promise<string>;
  invoke(input: unknown): Promise<string>;
}

export interface LayerXLangChainIntegration extends LayerXAgentIntegration {
  readonly langChainTools: readonly LangChainToolSpec[];
  langChainTool(name: string): LangChainToolSpec;
}

export function langChainToolSpec(executor: AgentToolExecutor, definition: ToolDefinition): LangChainToolSpec {
  const run = async (input: unknown): Promise<string> => {
    const outcome = await executor.execute(definition.name, input ?? {});
    return JSON.stringify(renderOutcome(outcome));
  };
  return {
    name: definition.name,
    description: definition.description,
    schema: definition.inputSchema,
    returnDirect: false,
    func: run,
    invoke: run,
  };
}

export function createLangChainIntegration(options: AgentIntegrationOptions): LayerXLangChainIntegration {
  const integration = createAgentIntegration(options);
  const tools = integration.tools.definitions.map((definition) => langChainToolSpec(integration.tools, definition));
  return {
    ...integration,
    langChainTools: tools,
    langChainTool: (name) => {
      const tool = tools.find((candidate) => candidate.name === name);
      if (tool === undefined) {
        throw new AgentIntegrationError("unknown-tool");
      }
      return tool;
    },
  };
}
