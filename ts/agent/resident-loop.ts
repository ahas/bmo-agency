// Agency's own program in the pane of an agent whose provider has no terminal conversation of its own
// (docs/agent-control.md, 「The interactive surface a provider declares」): the pane's loop starts it
// with the provider it stands in for, the agent's place, its line and its model, and it runs until it
// is ended. What it does is in `loop-program.ts`; this is the screen around it.
import "./registrations";
import { agentProviderNames, type AgentProviderName, type RunLine } from "./harness";
import { ResidentLoop } from "./loop-program";

function flag(name: string): string | undefined {
  const at = process.argv.indexOf(name);

  return at === -1 ? undefined : process.argv[at + 1];
}

const named = flag("--provider");
const place = flag("--place");
const line = flag("--line") as RunLine | undefined;
const model = flag("--model");

if (named === undefined || !agentProviderNames.includes(named as AgentProviderName) || place === undefined || line === undefined || model === undefined) {
  console.error("usage: bun ts/agent/resident-loop.ts --provider <provider> --place <dir> --line <line> --model <model>");
  process.exit(2);
}

const loop = new ResidentLoop({ provider: named as AgentProviderName, place, line, model, env: process.env, write: (text) => process.stdout.write(text) });

// A person at the pane types into it key by key, so the terminal hands over each key as it comes
// rather than a line at a time, and the loop draws what is typed itself.
if (process.stdin.isTTY) process.stdin.setRawMode(true);

process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk: string) => loop.input(chunk));

for (const signal of ["SIGTERM", "SIGHUP"] as const) {
  process.on(signal, () => {
    loop.stop();
    process.exit(0);
  });
}

loop.start();
