import { join } from "node:path";

import { dataPath } from "@agency/support/roots";

import { claudeCodeAt, claudeResidentAt } from "./claude-code";
import { machineCatalogue, missingOnMachine } from "./provider-facts";
import { providerDeclaration } from "./provider-declaration";
import { reasoningLevels } from "./reasoning";
import declaredDeepSeek from "@agency/policy/providers/deepseek.toml";

import type { ClaudeEndpoint, EndpointModel } from "./claude-code";
import type { AgentProvider, HarnessRun, RunLine } from "./harness";
import type { ProviderReasoning } from "./reasoning";

// DeepSeek's own API, reached through Claude Code's Anthropic-compatible endpoint
// (docs/cache-fallback.md, 「The DeepSeek road」). Its key is the endpoint account's key file; its
// models are the two its policy declares. A run's thinking and effort reach DeepSeek in the words
// its endpoint takes: the program's `--effort` is carried as the request's effort, and a run that
// does not think sends no thinking.

const declared = providerDeclaration(declaredDeepSeek);
const providerName = "deepseek";

interface DeclaredModel {
  id: string;
  label: string;
  context_tokens: number;
  answer_tokens: number;
  thinking: boolean;
}

const endpointPolicy = declaredDeepSeek.endpoint as {
  base_url: string;
  base_env: string;
  wait_seconds: number;
  model: DeclaredModel[];
};
const presence = declaredDeepSeek.presence as { key_file: string; missing: string; not_held: string };
const carried = declaredDeepSeek.lines as RunLine[];

// The effort word DeepSeek's own API knows as a launch without thinking.
const off = "none";

function modelNamed(model: string): DeclaredModel | null {
  return endpointPolicy.model.find((one) => one.id === model) ?? null;
}

function homes(): string {
  return dataPath("claude-roads", providerName);
}

// What a run on this road is handed: the model's own home, window and answer ceiling, and whether the
// run thinks — the agent's switch, and no thinking where its effort names `none`.
export const deepSeekEndpoint: ClaudeEndpoint = {
  provider: providerName,
  get baseUrl() {
    return process.env[endpointPolicy.base_env] || endpointPolicy.base_url;
  },
  keyPath: () => dataPath("env", presence.key_file),
  async model(run: Pick<HarnessRun, "model" | "effort" | "thinking">, _longTools?: readonly string[]): Promise<EndpointModel> {
    const held = modelNamed(run.model);

    if (held === null) throw new Error(`${presence.not_held}: ${run.model}`);

    return {
      name: held.id,
      label: held.label,
      contextTokens: held.context_tokens,
      answerTokens: held.answer_tokens,
      requestWaitMs: endpointPolicy.wait_seconds * 1_000,
      thinking: run.thinking && run.effort !== off,
      home: join(homes(), held.id),
    };
  },
  // Claude Code's own effort words, the ones its `--effort` takes; a run that does not think goes out
  // with the lowest the program takes, and its thinking is shut off beside it.
  effort: ({ thinking, effort }) => (!thinking ? "low" : effort === "" || effort === off ? "high" : effort),
  homes: homes(),
  // The program gates each call and withholds its own tools as it does on its own road; a research
  // prompt is not offered the endpoint.
  offers: ["tool-gate", "mcp-only-tools"],
};

// A level an agent chose reaches the endpoint where it is one of the scale's levels; its thinking off
// is DeepSeek's `none`; and a mapping's word stands where the agent chose no level.
const deepSeekReasoning: ProviderReasoning = {
  thinking: true,
  levels: async () => reasoningLevels,
  effort: async (mapped, choice, model) => {
    if (choice.thinking === false) return off;
    const held = await deepSeekReasoning.levels(model);

    return choice.effort !== null && held.includes(choice.effort) ? choice.effort : mapped;
  },
};

export const deepSeekProvider: AgentProvider = {
  name: providerName,
  label: declared.label,
  adapter: claudeCodeAt(deepSeekEndpoint),
  unavailable: async () => await missingOnMachine(providerName),
  models: async () => ((await machineCatalogue(providerName)) as { models: { id: string }[] }).models.map((one) => ({ id: one.id })),
  efforts: declared.efforts,
  effortFault: declared.effortFault,
  reasoning: deepSeekReasoning,
  capsAnswerTokens: declared.capsAnswerTokens,
  lendsCoworkers: declared.lendsCoworkers,
  // The key is the endpoint account's; the model's own home holds no login.
  logins: null,
  resident: claudeResidentAt(deepSeekEndpoint, declaredDeepSeek.resident as { interrupt: string[]; words_mid_turn: boolean }),
  lines: carried,
  // A worker pinned to one of its models runs on it; another name is refused with the policy's word.
  namedModelFault: async (model) => (modelNamed(model) === null ? `${presence.not_held}: ${model}` : null),
};
