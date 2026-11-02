# Agency's tools

The tools agency itself provides to the tool host: each a folder of its own under `tools/`, with its
`tool.toml`, which the tool host reads as it reads any other tool's. They hand agents the documents
agency keeps under `prompts/` — the chat agent's procedures — let a chat agent hand a resident worker a
small task, and let a chat agent delegate a task to the helper on a co-worker seat and read what it has
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

- **`tools/worker_ask`** declares `bmo/worker_ask` to a session holding `room.tools`: a chat agent hands a
  resident worker a small coding task as a turn into the worker's session, with no queue item, lease or
  report.
- **It asks only past the owner's gate** (`docs/launch-lines.md`, 「The owner's gate」): on a room turn not the
  owner's, every message the turn answers must be answered allowed `launch` in the room by the account
  service, and where it is not, or cannot be asked, the tool answers a failure saying so and starts nothing.
- **Its arguments** are the worker's name, the words and an optional folder. The folder is the asking
  agent's scratch workspace under the data root where none is named, made on first use; a worker's own
  checkout may be named only in the owner's turn.
- **It waits for the worker's turn to end,** however long the turn takes: no bound of its own and no
  timeout on its wait for agency's answer.
- **Its answer** is the turn's answer text and the paths of the files the turn created or changed in the
  folder, read from the run's recorded edit diffs. The exchange is a turn of the worker's like any other,
  written to its log, so the console shows it.
- **A busy worker is waited for, never refused.** Where the worker is in a turn of its own, a job's or
  another ask's, the ask goes in behind it, sent again at a steady pace until the worker's session takes
  it, for every asker past the owner's gate, and runs once the worker is free.
- **It refuses only what cannot be asked at all:** an unknown worker, a worker that stands on a node where
  the folder is not, and a checkout named outside the owner's turn, besides the owner's gate. A task worth
  keeping and reviewing is a queue item, filed with `file_item`, not asked this way.

## The delegate

- **`tools/delegate`** declares `bmo/delegate` to a session holding `room.tools`: a chat agent hands a task
  — an investigation, a lookup on another machine, reading code, a problem it cannot settle in its turn,
  anything longer than moments that needs no queue item — to the helper (`docs/agent-roles.md`, 「The
  helper role」), with no item, lease or report. The chat agent calls it with `bmo use bmo/delegate
  --async`, so its turn goes on and the answer comes back to it as a wake.
- **It asks only past the owner's gate** (`docs/launch-lines.md`, 「The owner's gate」), as the worker ask
  does, before it asks anything of the pool: a refused turn starts nothing.
- **Its arguments** are the title, one line naming the task, which the delegation is listed by; the
  instruction, carrying every piece of context the task needs, since the helper sees nothing of the
  conversation; an optional role; and an optional folder.
  - The role is an agent role whose agents take delegated work; where none is named it is the one
    agency's roles policy names, `delegate` in `policy/roles.toml`, the helper role. An agent holding it
    carries the task, a free one first.
  - The folder is where the task works: the asking agent's scratch workspace under the data root where
    none is named, made on first use, the same one the worker ask defaults to.
- **It takes a co-worker seat and waits in line however long the line takes.** It asks the pool for a
  seat under the role, as the source `delegate`, keyed by a request of its own, on Claude Code; the seat's
  tier is the role's `role:<name>` assignment (`docs/coworker-pool.md`, 「Who takes a place」). It asks for
  its admission again at a steady pace until it is let in, is never refused for the pool being full and
  never gives up for having waited; a seat the pool gave back before it was let in ends the call with
  that said.
- **The work is a turn of the helper agent's own session** (`docs/sessions.md`, 「A worker's session per
  model」): the seat bound to the turn's run, and one turn sent into the agent's worker session carrying
  the seat's mapping, its work key and the asking turn's room, resumed as a job's turn is. So a delegation
  that follows another on the same model within the provider's cache lifetime goes on in the same
  session and reads its prefix and what came before, the keepalive keeps that session warm as it keeps
  every worker's (`docs/keepalive-schedule.md`), and a session past its cache lifetime or its model's
  context is replaced by a fresh one by the same rules.
  - **The session stands in the agent's own checkout,** which never changes between delegations, since a
    Claude Code session is kept per folder; the task's folder is named in the turn's words, and the turn
    works there with the agent's own program and tools — a shell among them, so it reaches other machines
    as a worker does.
  - **A helper agent carries one turn at a time.** A delegation whose agent is in a turn waits for that
    turn to end and is sent then, however long it takes.
  - The call waits for the turn's end with no timeout, and nothing bounds the turn by a clock.
- **Once the turn ends** the seat is finished with what the turn came to; the session stays held for the
  next delegation. A call whose caller goes away while its seat still waits cancels the seat, so nothing
  is left standing in the line; a turn already sent is carried to its end by agency, and its seat is
  finished as the call goes away.
- **Its answer** is the turn's answer text and the paths of the files created or changed in the folder
  while the task ran, found by comparing the folder before and after, its `.git` directories passed over.
- **Its stage stands on its seat,** where agency answers it (`docs/api.md`, 「pool, seat-requests,
  reservations and roles」): the seat is asked with the title and the asking agent, and bound naming the
  helper agent its turn goes to. A delegation is waiting while its seat waits in the line, running once
  its seat is let in and bound to the helper's turn, and ended once the seat is finished, with what it came
  to.
- **`bmo/delegations` lists the asking agent's delegations,** declared by the same tool to a session holding
  `room.tools`: each one's title, stage, the helper agent it runs on, when it was asked and, once ended, what
  it came to, the ones still waiting or running first and then the latest ended, newest first. It reads
  agency's `GET /delegations` for the agent the asking turn is, so an agent sees its own delegations and no
  other's, and it takes no argument but an optional count of ended ones to list.
- **A task worth keeping and reviewing is still a queue item,** filed with `file_item`; the delegate files
  nothing of the task.
