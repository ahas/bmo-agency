import { dataPath } from "@agency/support/roots";

import { dshOn } from "./dsh";
import { machineCatalogue, missingOnMachine } from "./provider-facts";
import { providerDeclaration } from "./provider-declaration";
import { reasoningLevels } from "./reasoning";
import declaredDeepSeek from "@agency/policy/providers/deepseek.toml";

import type { DshRoute } from "./dsh";
import type { AgentProvider, RunLine } from "./harness";
import type { ProviderReasoning } from "./reasoning";

// DeepSeek's own API, reached by the dsh harness on its OpenAI-compatible route
// (docs/cache-fallback.md, 「The DeepSeek road」). Its key is the road's key file, handed to dsh as the
// variable its route reads; its models are the two its policy declares. A run's thinking reaches
// DeepSeek as dsh's reasoning level: off where the run does not think.

const declared = providerDeclaration(declaredDeepSeek);
const providerName = "deepseek";

interface DeclaredModel {
  id: string;
  label: string;
}

const models = declaredDeepSeek.model as DeclaredModel[];
const presence = declaredDeepSeek.presence as { key_file: string; missing: string; not_held: string };
const route = declaredDeepSeek.dsh as { api: string; api_env: string; key_env: string; key_file: string };
const carried = declaredDeepSeek.lines as RunLine[];

// The effort word DeepSeek's own API knows as a launch without thinking.
const off = "none";

function modelNamed(model: string): DeclaredModel | null {
  return models.find((one) => one.id === model) ?? null;
}

/** DeepSeek's API as dsh's one route: where it answers, or a stand-in's address the environment names. */
export const deepSeekRoute: DshRoute = {
  provider: providerName,
  keyEnv: route.key_env,
  apiBase: () => process.env[route.api_env] || route.api,
  keyFile: () => dataPath("env", route.key_file),
};

// A level an agent chose reaches the route where it is one of the scale's levels; its thinking off
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
  adapter: dshOn(deepSeekRoute),
  unavailable: async () => await missingOnMachine(providerName),
  models: async () => ((await machineCatalogue(providerName)) as { models: { id: string }[] }).models.map((one) => ({ id: one.id })),
  efforts: declared.efforts,
  effortFault: declared.effortFault,
  reasoning: deepSeekReasoning,
  capsAnswerTokens: declared.capsAnswerTokens,
  lendsCoworkers: declared.lendsCoworkers,
  // The key is the road's own file; the provider holds no login.
  logins: null,
  // Its resident pane is the deployment's bmo-tui on a dsh session, which the server composes; this
  // program carries its headless runs alone.
  resident: null,
  lines: carried,
  // A worker pinned to one of its models runs on it; another name is refused with the policy's word.
  namedModelFault: async (model) => (modelNamed(model) === null ? `${presence.not_held}: ${model}` : null),
};
