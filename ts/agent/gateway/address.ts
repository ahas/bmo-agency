// Where agency's gateway answers, for every caller alike (docs/own-llm.md, 「The gateway」): the address
// the process names in `BMO_LLM_GATEWAY_URL`, which a check standing a launcher of its own on another
// port names for its own runs, and the gateway serving the bot otherwise, as the registered models'
// policy file declares it.
import declaredRegistered from "@agency/policy/providers/registered.toml";

import type { Sampling } from "../harness";

export const defaultGatewayUrl: string = declaredRegistered.gateway.url;

export function gatewayUrl(env: Record<string, string | undefined> = process.env): string {
  const named = env.BMO_LLM_GATEWAY_URL?.trim() ?? "";

  return named === "" ? defaultGatewayUrl : named.replace(/\/+$/, "");
}

// The gateway's own words for how long a request's caller waits, for an answer and for a streamed one,
// in seconds: the gateway gives up where its caller does rather than holding a node's card for a
// caller gone (docs/own-llm.md, 「The gateway」).
export function gatewayWaitHeaders(waitMs: number): Record<string, string> {
  const seconds = String(Math.max(1, Math.ceil(waitMs / 1_000)));

  return { "x-litellm-timeout": seconds, "x-litellm-stream-timeout": seconds };
}

// The header a run names its sampling settings in, which the gateway's sampling hook writes into the
// request the backend reads (docs/own-llm.md, 「The gateway」); none where the run names none.
export function gatewaySamplingHeaders(sampling: Sampling | undefined): Record<string, string> {
  return sampling === undefined || Object.keys(sampling).length === 0 ? {} : { "x-bmo-sampling": JSON.stringify(sampling) };
}

// The header a run names its program's long-waiting tools in, which the gateway's long-tools hook
// copies into the request the backend reads (docs/own-llm.md, 「How requests go」); none where the
// program names none. The names are the program's own, handed in by its adapter.
export function gatewayLongToolHeaders(names: readonly string[] | undefined): Record<string, string> {
  return names === undefined || names.length === 0 ? {} : { "x-bmo-long-tools": names.join(",") };
}
