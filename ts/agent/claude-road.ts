// Whether a Claude Code program's environment points it at an endpoint other than Anthropic's — an own
// LLM service's road (docs/own-llm.md, 「How requests go」). Such a program opens no session on a
// Claude login and writes no prefix to Anthropic's cache, so what guards a login's program does not
// guard it. The variable is the CLI's own name for where its requests go.
export function pointedElsewhere(env: Readonly<Record<string, string | undefined>>): boolean {
  const named = env.ANTHROPIC_BASE_URL;

  return named !== undefined && named !== "";
}
