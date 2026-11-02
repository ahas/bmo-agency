# Agency's tools

The tools agency itself provides to the tool host: each a folder of its own under `tools/`, with its
`tool.toml`, which the tool host reads as it reads any other tool's. They hand agents the documents
agency keeps under `prompts/` — the chat agent's procedures — let a supervisor hand a small coding task
to a run of the worker role, and let a supervisor delegate a task to a run of any role and read what it has
delegated. A tool that
belongs to no one service is a repository of its own, not a folder here.

## A tool's folder

- **`tools/<name>/`** holds the tool's manifest, `tool.toml`, the schema file of each entry it
  declares, and the Rust crate of its executable, a member of agency's workspace.
- **A deployment places each tool** in its folder's `.placed/` inside the release of agency's own
  deploy tree, under `$HOME/.local/share/bmo-agency` (`docs/agent-control.md`, 「The job's release」),
  the folder the tool host reads: the executable the release's `bin/` holds, beside the manifest and
  the schema files it names. The steps that do it stand in the testbed's declaration, `testbed.toml`.
- **The manifest** says the tool's name, the protocol version it speaks, its build and check
  commands, its run command, and either its entries or the resources it serves, each with the
  capabilities a session must hold. It names no timeout, for the tool or an entry: a call is waited on
  until the executable answers. Its tests hold the manifest to what the executable
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
- **The session record** is the tool host's: the holder's name, the session's namespaces,
  capabilities, grants and shown entries, and the variables handed to every call's environment. The
  data root stands in those variables as `BMO_DATA_DIR`. Agency's own values stand under the issuer's
  `labels`: the line, the store's address and the agent's place where it is a resident program; an apart
  session's caller adds labels of its own beside them (`docs/api.md`, 「sessions」), which agency's tools do
  not read. The turn's own variables stand in `variables`, beside the
  deployment's variables the run's line names in `policy/run-lines.toml` for the placed tools that read
  them, each where the deployment sets it in agency's environment — the chat line's `BMO_GATE_URL` and
  `BMO_GATE_TOKEN_FILE`, the gate's address and the path of its token file, which the gate's translate
  tool calls it with, so no token enters the record. A tool reads only the parts it needs and passes over
  the rest.
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
  `AGENCY_CONTROL_TOKEN` name, the token read from the file `AGENCY_CONTROL_TOKEN_FILE` names where the
  variable is not set. A placed tool's environment holds neither, since the tool host hands it a
  clean one, so it reads them from `env/agency.env` under the record's data root, the file the server's
  unit takes its environment from; a variable its own environment holds stands first.
- **A turn a companion's reset overtook** — one naming an agent and a moment at or before that agent's
  reset point in agency's store (`docs/companions.md`, 「What the reset cuts off」) — is listed nothing and
  refused every read, saying the agent was reset after the turn began.

## The worker ask

- **`tools/worker_ask`** declares `bmo/worker_ask` to a session holding `room.tools`: a supervisor hands a
  small coding task to a run of the `worker` role, with no queue item, lease or report.
- **It asks only past the owner's gate** (`docs/launch-lines.md`, 「The owner's gate」): on a room turn not the
  owner's, every message the turn answers must be answered allowed `launch` in the room by the account
  service, and where it is not, or cannot be asked, the tool answers a failure saying so and starts nothing.
- **Its arguments** are the words and an optional folder. The folder is the asking supervisor's scratch
  workspace under the data root where none is named, made on first use; a repository's checkout may be named
  only in the owner's turn, and the run is then handed that repository's own (`docs/repositories.md`, 「What a
  run in a repository is handed」).
- **It takes a slot of the worker role,** as the source `ask`, and a repository's checkout where it names one,
  and waits for both however long it takes: a busy role or checkout is waited for, never refused
  (`docs/role-slots.md`, 「The line」). The run goes out on the worker role's active provider
  (`docs/provider-order.md`), and on the session kept for the worker role at that folder on that provider
  and model where one is valid (`docs/sessions.md`, 「A role's session」).
- **It waits for the run to end,** however long the run takes: no bound of its own and no timeout on its
  wait for agency's answer.
- **Its answer** is the run's answer text and the paths of the files the run created or changed in the
  folder, read from the run's recorded edit diffs. The run is logged like any other, so the console shows it.
- **It refuses only what cannot be asked at all:** a folder on no machine the worker role can run on and a
  checkout named outside the owner's turn, besides the owner's gate. A task worth keeping and reviewing is a
  queue item, filed with `file_item`, not asked this way.

## The delegate

- **`tools/delegate`** declares `bmo/delegate` to a session holding `room.tools`: a supervisor hands a task —
  an investigation, a lookup on another machine, reading code, a problem it cannot settle in its turn,
  anything longer than moments that needs no queue item — to a run of a role, with no item, lease or report.
  The supervisor calls it with `bmo use bmo/delegate --async`, so its turn goes on and the answer comes back
  to it as a wake.
- **It asks only past the owner's gate** (`docs/launch-lines.md`, 「The owner's gate」), as the worker ask
  does, before it asks anything of a slot: a refused turn starts nothing.
- **Its arguments** are the title, one line naming the task, which the delegation is listed by; the
  instruction, carrying every piece of context the task needs, since the run sees nothing of the
  conversation; an optional role; and an optional folder.
  - The role is any role the store holds; where none is named it is the one agency's roles policy names for
    delegated work, `helper`. A role the store does not hold is an error: the call is refused with the name
    and starts nothing.
  - The folder is where the task works: the asking supervisor's scratch workspace under the data root where
    none is named, made on first use, the same one the worker ask defaults to.
- **It runs as the supervisor's double by default:** on the asking supervisor's own provider, model and
  effort as they stand when it is asked, rather than the role's order (`docs/role-slots.md`, 「A delegated
  task」), under the role's prompt and bundles.
- **It takes a slot of the role and waits in its line however long the line takes.** It asks for its
  admission again at a steady pace until it is let in, is never refused for the role being full and never
  gives up for having waited; a slot given back before it was let in ends the call with that said.
- **The work is a run of the role at the task's folder** (`docs/sessions.md`, 「A role's session」): one turn
  carrying the instruction and the asking turn's room, on the session kept for that role at that folder on
  that provider and model where one is valid. So a delegation that follows another at the same folder on the
  same model within the provider's cache lifetime goes on in the same session and reads what came before, and
  one past the cache lifetime or the model's context opens a new one. The run works with its role's program
  and tools — a shell among them, so it reaches other machines as a job does.
  - The call waits for the run's end with no timeout, and nothing bounds the run by a clock.
- **Once the run ends** the slot is finished with what the run came to. A call whose caller goes away while
  its slot still waits cancels the slot, so nothing is left standing in the line; a run already sent is
  carried to its end by agency, and its slot is finished as the call goes away.
- **Its answer** is the run's answer text and the paths of the files created or changed in the folder while
  the task ran, found by comparing the folder before and after, its `.git` directories passed over.
- **Its stage stands on its slot** (`docs/api.md`, 「slots, reservations and places」): the slot is asked with
  the title and the asking supervisor. A delegation is waiting while its slot waits in the line, running once
  its slot is let in and bound to its run, and ended once the slot is finished, with what it came to.
- **`bmo/delegations` lists the asking supervisor's delegations,** declared by the same tool to a session
  holding `room.tools`: each one's title, stage, role, the provider and model it runs on, when it was asked
  and, once ended, what it came to, the ones still waiting or running first and then the latest ended,
  newest first. It reads agency's `GET /delegations` for the supervisor the asking turn is, so a supervisor
  sees its own delegations and no other's, and it takes no argument but an optional count of ended ones to
  list.
- **A task worth keeping and reviewing is still a queue item,** filed with `file_item`; the delegate files
  nothing of the task.
