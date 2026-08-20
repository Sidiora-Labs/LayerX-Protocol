import {
  createAgentIntegration,
  type AgentIntegrationOptions,
  type LayerXAgentIntegration,
} from "./integration.js";
import { renderOutcome, type ToolDefinition, type ToolJsonObject } from "./tools.js";

export interface AnthropicTool {
  readonly name: string;
  readonly description: string;
  readonly input_schema: ToolJsonObject;
}

export interface AnthropicToolUseBlock {
  readonly type: "tool_use";
  readonly id: string;
  readonly name: string;
  readonly input: unknown;
}

export interface AnthropicToolResultBlock {
  readonly type: "tool_result";
  readonly tool_use_id: string;
  readonly content: string;
  readonly is_error: boolean;
}

export interface LayerXAnthropicIntegration extends LayerXAgentIntegration {
  readonly anthropicTools: readonly AnthropicTool[];
  handleToolUse(block: AnthropicToolUseBlock): Promise<AnthropicToolResultBlock>;
  handleToolUseBlocks(content: readonly unknown[]): Promise<readonly AnthropicToolResultBlock[]>;
}

export function anthropicTool(definition: ToolDefinition): AnthropicTool {
  return {
    name: definition.name,
    description: definition.description,
    input_schema: definition.inputSchema,
  };
}

export function anthropicTools(definitions: readonly ToolDefinition[]): readonly AnthropicTool[] {
  return definitions.map(anthropicTool);
}

export function isToolUseBlock(value: unknown): value is AnthropicToolUseBlock {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const block = value as Record<string, unknown>;
  return block["type"] === "tool_use"
    && typeof block["id"] === "string"
    && typeof block["name"] === "string";
}

export function createAnthropicIntegration(options: AgentIntegrationOptions): LayerXAnthropicIntegration {
  const integration = createAgentIntegration(options);
  const handleToolUse = async (block: AnthropicToolUseBlock): Promise<AnthropicToolResultBlock> => {
    const outcome = await integration.tools.execute(block.name, block.input ?? {});
    return {
      type: "tool_result",
      tool_use_id: block.id,
      content: JSON.stringify(renderOutcome(outcome)),
      is_error: !outcome.ok,
    };
  };
  return {
    ...integration,
    anthropicTools: anthropicTools(integration.tools.definitions),
    handleToolUse,
    handleToolUseBlocks: async (content) => {
      const results: AnthropicToolResultBlock[] = [];
      for (const block of content) {
        if (isToolUseBlock(block)) {
          results.push(await handleToolUse(block));
        }
      }
      return results;
    },
  };
}
