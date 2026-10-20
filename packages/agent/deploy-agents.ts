// Agency's prompt-composing entry (docs/agent-documents.md, 「How the documents are composed」): what every
// agent is given — composed prompts and runner directories — written under the agents directory the data
// root holds, and listed. The gates beside agency's own agents are the caller's, named in a JSON file
// whose path is the one argument: an output gate as `{ name, model, system }`, a reference gate as
// `{ name }`, its system prompt composed from its own folder; with none, only agency's own are written.
// The server runs it on every start, the reload path whenever an agent document changes, and a person
// by hand to see or refresh what is written.
import { readFileSync } from "node:fs";
import { agentFaultLine, deployAgents, deployRoot, type GateAgent } from "./agents";

interface NamedGate {
  name: string;
  model?: string;
  system?: string;
}

function gatesNamed(path: string | undefined): GateAgent[] {
  if (path === undefined) {
    return [];
  }

  const named = JSON.parse(readFileSync(path, "utf8")) as NamedGate[];

  return named.map((gate) => {
    const system = gate.system;

    return {
      name: gate.name,
      ...(gate.model === undefined ? {} : { model: gate.model }),
      ...(system === undefined ? {} : { system: () => system }),
    };
  });
}

const deployment = await deployAgents(gatesNamed(process.argv[2]));
const count = (value: number): string => value.toLocaleString("en-US");

console.log(`deployed ${count(deployment.files.length)} files under ${deployRoot()}`);

for (const file of deployment.files) {
  console.log(
    `  ${count(file.bytes).padStart(9)} B  ~${count(Math.round(file.bytes / 2.5)).padStart(7)} tok  ${file.path.slice(deployRoot().length + 1)}`,
  );
}

for (const fault of deployment.faults) {
  console.error(agentFaultLine(fault));
}
