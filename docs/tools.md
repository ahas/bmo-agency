# Agency's tools

The tools agency itself provides to the tool host: each a folder of its own under `tools/`, with its
`tool.toml`, which the tool host reads as it reads any other tool's. They hand agents the documents
agency keeps under `prompts/`: the chat agent's procedures and a research run's rulebooks. A tool that
belongs to no one service is a repository of its own, not a folder here.

## A tool's folder

- **`tools/<name>/`** holds the tool's manifest, `tool.toml`, the schema file of each entry it
  declares, and the Rust crate of its executable, a member of agency's workspace.
- **A deployment places each tool** in its folder's `.placed/` inside the release of agency's own
  deploy tree, under `$HOME/.local/share/bmo-agency` (`docs/agent-control.md`, 「The job's release」),
  the folder the tool host reads: the executable the release's `bin/` holds, beside the manifest and
  the schema files it names. The steps that do it stand in the testbed's declaration, `testbed.toml`.
- **The manifest** says the tool's name, the protocol version it speaks, its build and check
  commands, its run command, its timeout, and either its entries or the resources it serves, each
  with the capabilities a session must hold. Its tests hold the manifest to what the executable
  answers.
- **The executable answers one request and ends:** the tool host hands it one JSON object on
  standard input and reads one on standard output.
  - The object carries the protocol version, its `kind` — `call`, `resources_list` or
    `resources_read` — what that kind asks (an entry's full name and its arguments, or a resource's
    `uri`), and the session record the request is made under.
  - A call is answered with a tool result: its `content`, text pieces, and `isError`. A call whose
    arguments do not fit the entry is answered as a failure saying so.
  - A listing is answered `{ "resources": [...] }`; a read `{ "contents": [...] }`, or
    `{ "error": "<why>" }` where the document cannot be read.
  - A request that does not read, or a kind the executable does not answer, ends it unclean with why
    on its error stream, which the tool host takes as a failed call.
- **`crates/tool-call`** is agency's side of that protocol, which every tool here shares: the request,
  the parts of the session record agency's tools read, the answer, and the tree they read from.
- **The session record** names the data root, the store's address, the agent's place where it is a
  resident program, the run's own variables and a research run's settings. A tool reads only the parts
  it needs and passes over the rest.
- **The turn a request is made in** is read afresh from the record: the run the agent's place names
  in its `run.json` — none for a person's turn — or else the run's own variables. Its variables are
  `BMO_TURN_AGENT`, the agent the turn is; `BMO_TURN_BEGAN`, when it took its words, in epoch
  milliseconds; `BMO_TURN_CHAT`, the room it answers; and `BMO_TURN_VIA`, the road it answers on.
- **The tree a tool reads `prompts/` from** is the nearest folder above the one it runs in that holds
  `prompts/`, which is the deploy tree the tool is placed in; else the tree `BMO_DEPLOY_DIR` names;
  else agency's deploy link, `$HOME/.local/share/bmo-agency/deploy`.

## The procedures

- **`tools/procedures`** serves each document under `prompts/agents/chat/procedures/` as a resource,
  one per document, under the `bmo` namespace, to a session holding `room.tools`. A document's address
  is `bmo://procedure/<name>`, its name its file's without `.md`, and it is `text/markdown`.
- **A listing** names every procedure. **A read** answers one document:
  - an address that is no procedure's, or a name no document has, is refused with the names there
    are, before anything else is asked;
  - outside any room's turn, the document is read whole;
  - in a room's turn, it is read as that room's road may use it: a part marked for a verb the road
    does not perform is left out, and the marks are taken off what is left. Its trailing blank lines
    go either way.
- **The marks:** a line ending in `[CAP:<verb>,<verb>]` needs those verbs. A level-two heading marked
  so takes its whole section with it; any other marked line takes itself, the lines it wraps onto and
  every line indented under it. An unmarked line is kept, and what stands under it is held to the
  verbs line by line.
- **The verbs a road performs** are the consumer's that holds the rooms. The tool asks agency's server
  `GET /road-verbs` with the room and the road the turn names, and the server asks the consumer's
  `road-verbs` hook (`docs/api.md`, 「Hooks」). With no consumer registered the road performs no verb,
  and every marked part is left out. Where the server cannot be asked, the read is refused with why.
- **The tool reaches agency's server** on the port and with the token `AGENCY_CONTROL_PORT` and
  `AGENCY_CONTROL_TOKEN` name. A placed tool's environment holds neither, since the tool host hands it a
  clean one, so it reads them from `env/agency.env` under the record's data root, the file the server's
  unit takes its environment from; a variable its own environment holds stands first.
- **A turn a companion's reset overtook** — one naming an agent and a moment at or before that agent's
  reset point in agency's store (`docs/companions.md`, 「What the reset cuts off」) — is listed nothing and
  refused every read, saying the agent was reset after the turn began.

## The rulebook reader

- **`tools/read_rulebook`** declares the entry `web/read_rulebook`, read-only, to a session holding
  `web.rulebooks`. Its one argument, `name`, a non-empty string, names a rulebook as its file under
  `prompts/rules/` is named, without `.md`.
- **It answers one of the run's rulebooks whole,** headed `RULEBOOK prompts/rules/<name>.md:`. A research
  run's prompt carries only the section of each rulebook that decides whether it applies to the search;
  once it does, the run reads the whole here before searching.
- **Which rulebooks are the run's** is its record's `research.rulebooks`. A name the run was not handed
  is refused with the ones it was, and a rulebook that cannot be read is refused with why. A run past
  its research close still reads one, since reading searches nothing.
