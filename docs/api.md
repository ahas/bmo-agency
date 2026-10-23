# The boundary API

This document is agency's boundary: every route it exposes, every event it publishes, every hook a
consumer registers, and the tables whose shape a consumer reads or writes. A consumer reaches agency
only across this boundary and never into a crate or a module behind it; agency reaches a consumer only
by publishing an event or calling a hook the consumer registered. What a turn is told, whom an agent
answers, which role and model a piece of work needs, and whether an answer may go out are the
consumer's, decided above this boundary and handed in; agency supplies, carries and controls, and judges
none of it.

`docs/openapi.json` is the machine contract: every route's parameters, bodies, answers and codes, the
events, the two tools of the harness plugin, and the removed and moved-out members. It is generated from
the service's own types by `bmo-agency openapi`, and a test fails where the committed document differs
from what the code generates. This document states what the routes are for and how the pieces fit; where
a field's exact shape matters, the OpenAPI document is the one to read.

## The shapes the boundary is made of

- **A route** is an operation agency's server answers: a verb and a path, a request body and a response
  body, and the status codes and kebab-case outcome codes it answers with.
- **An event** is something agency publishes as it happens. Agency does not wait on a subscriber and reads
  nothing back from one.
- **A hook** is a decision agency hands up: a consumer registers one, and agency calls it at the point
  named. Every hook has a default that does nothing.
- **A table** whose contract is shared is one a consumer reads or writes directly beside agency.
- **A client** is the Rust crate and the TypeScript package agency publishes from `docs/openapi.json`.

## Versioning

- The contract carries the repository's one version, in semver; the workspace manifest is its source. Before
  the first release the version is `0.1.0` plus `-rev.N`, and the current version is `0.1.0-rev.14`.
- A change to this document and a change to the boundary go in one commit. A change to a route, an event, a
  hook or a shared table's contract repairs this document in the same commit.
- The major moves on a removed or renamed route, field or event, the minor on an addition, the patch on
  anything else. Before the first release the revision moves instead.

## The control port

- Every route is answered by agency's server, `bin/agency-server`, on the loopback, on the port
  `AGENCY_CONTROL_PORT` names (3220 where it names none), under `Authorization: Bearer <token>` with the
  token `AGENCY_CONTROL_TOKEN` holds. A request without the token is answered 401.
- Both variables stand in the data root's `env/agency.env`. A process of agency's whose environment holds
  neither reads them there.
- A route takes and answers JSON, keys in camelCase, times in epoch milliseconds, ids as signed 64-bit
  integers. A failure is a non-2xx status with `code` and `error`. A list is a page: `items` and `more`,
  asked with `after` or `before` and `limit`.
- A route that changes state takes a PATCH with the field it changes; an action that leaves a record is a
  POST creating it; a computation that keeps nothing is a POST named for its result.
- No path carries a version. A gateway mounts the service under `/api/agency/` and passes the path on
  unchanged.

## Routes by resource

### health

| Method | Path | Does |
|---|---|---|
| GET | `/health` | Readiness and version. |

### events

| Method | Path | Does |
|---|---|---|
| GET | `/events` | Agency's one stream, as server-sent events or a page with `Accept: application/json`. |
| POST | `/events` | A process of agency's own publishes an event. |

An event's SSE name is `<resource>.<change>`, its id is the service's event number, and its data is
`{"id","kind","at","data"}`, the inner `data` the member as a GET answers it, or its key alone for a
deleted member. A reader resumes with `Last-Event-ID` or `after`, and narrows with `kinds`.

### hooks

| Method | Path | Does |
|---|---|---|
| GET | `/hooks` | The hook points and what is registered at each. |
| PUT | `/hooks/{point}` | Register a downstream's URL at a hook point. |
| DELETE | `/hooks/{point}` | Take a registration away. |
| GET | `/road-verbs` | The verbs a room's road performs, as the consumer answers them. |

The hook points are `turn-submitting` and `session-summarising`, both registered by the gate and the
receiver. `road-verbs` is what the procedures tool asks a room's road.

### agents

| Method | Path | Does |
|---|---|---|
| GET | `/agents` | Every agent. |
| GET | `/agents/{agentName}` | One agent: role, gates, reasoning, voice, reset point, stand-in. |
| PATCH | `/agents/{agentName}` | Change an agent's gates, reasoning, role or voice. |

### prompt-documents

| Method | Path | Does |
|---|---|---|
| GET | `/prompt-documents` | Deployed prompt documents: base prompts, rulebooks, personas, agent definitions, system prompts. |
| GET | `/prompt-documents/{documentName}` | One prompt document with its version. |
| PATCH | `/prompt-documents/{documentName}` | Change a base prompt or its chooser. |
| GET | `/rulebooks` | The shared rulebooks named, as the deploy tree holds them. |

### document-moves

| Method | Path | Does |
|---|---|---|
| POST | `/document-moves` | Word that session documents moved, so sessions read them again. |

### sessions

| Method | Path | Does |
|---|---|---|
| GET | `/sessions` | Every session, by agent or scope. |
| POST | `/sessions` | Start a session for an agent at a scope. A run with a fixed prompt is such a session naming its prompt document. |
| GET | `/sessions/{sessionId}` | A session's state: provider, model, place, holder, why the prefix changed. |
| PATCH | `/sessions/{sessionId}` | Move a conversation's provider and model, or a thrown-away session's model, or hand a worker's session over to another holder. |
| DELETE | `/sessions/{sessionId}` | End a session, or clear a room's session. |
| POST | `/sessions/{sessionId}/turns` | Send a turn into a session; words to a worker are a turn into its session. |
| GET | `/sessions/{sessionId}/turns/{turnId}` | A turn and what it has written so far. |
| PATCH | `/sessions/{sessionId}/turns/{turnId}` | Interrupt a turn. |
| POST | `/sessions/{sessionId}/turns/{turnId}/inputs` | Push words into a working turn. |
| POST | `/sessions/{sessionId}/answers` | Answer a question a turn asked. |
| POST | `/sessions/{sessionId}/folds` | Fold a session, compacting its context. |

A job's run is a worker's session turn with the work parts in the turn's context. A session is started at
one scope — `agent`, `room`, `worker`, `editor`, `apart` or `conversation` — and the scope says what is
kept. Its id names the agent and the scope: the agent's name alone for an agent's session,
`<agent>@<room>`, `<agent>@worker` or `<agent>@editor` for a room's, a worker's or an editor's, and a
number the service gives it for a thrown-away session (`<agent>@apart:<n>`) and a conversation
(`<agent>@conversation:<n>`), so every start of one of those is a session of its own.

A conversation is the caller's: its start names the provider, the model and, where it names one, the
folder its turns go out on, and its turns are ordinary prompts with the context around them and no work
parts. A provider whose program keeps a session continues the one its last answered turn left; OpenRouter
keeps its dsh session inside bmo-tui's pane, which carries the next turn although agency stores no id for
it. The session stands in the server's memory from its start to its end.

A conversation whose agent is a gate work key carries a gate's one turn: the turn's `gate` part holds the
gate's instructions as the program's whole system prompt, whether it thinks, and the model, effort and
answer room its mapping resolved to. The turn goes out with no tools, no MCP server and no session kept,
and its answer is the gate's verdict.

A room's session is readable by its id from its start, before its first turn has opened a generation:
the read answers the provider, model and place its agent's row names, at generation 0, and its first
turn opens the first generation on that road. A start writes nothing of the session, so the list of
sessions names a room's once its first turn has opened one.

A conversation moves its provider and model on the change route, each left as it stands where the body
names only the other, and starts its next turn on a fresh session where the road keeps one. A
thrown-away session's model moves the same way, while its provider stays Claude Code's, where its turns
go out. A hand-over is a worker's alone: it closes the worker's held
session on the model, leaves its note for the next turn and drops the record, and the holder the body
names is kept for the session's state; a worker with nothing open keeps the holder alone. An agent's and
a room's session go out on the provider and model their agent's row names, and that row is where those
move. `holder` and `prefixChangeReason` are read back on the session wherever it is listed or read. A
worker's state answers the provider its session stands on: the provider the worker is pinned to, else the
one its held session last answered on, else `claude-code`.

A turn is named from its start: `turn.created` carries its `turnId`, and the send answers with that same
id once the turn ends. The progress read answers the turn named, on every scope that carries turns, with
its `id`, `sessionId`, `prompt`, `context`, `state` (`running` or `ended`), `delivery`, `outcome`,
`answer` and `text`, the last holding every text block the turn has written so far. The progress read
answers the turn named while it runs, and the session's last turn once it has ended; the interrupt and
the words pushed into a turn act on the turn going now. Each answers `unknown-turn` where the session
carries no turn of that id. A session's turn that has ended keeps what it wrote until the session's next
turn begins.

Every route that takes a body names the request type it reads, holding only the fields that route reads;
`docs/openapi.json` names that type, and no route's request is a response type reused.

### turn-contexts

| Method | Path | Does |
|---|---|---|
| POST | `/turn-contexts` | The context the submit hook would add to a typed line. |

### runs

| Method | Path | Does |
|---|---|---|
| GET | `/runs` | Runs by search: live, recent, by agent, room, model, job, worker, names; with the tokens spent so far. |
| POST | `/runs` | Open a run in the ledger. |
| GET | `/runs/{runId}` | One run, its clock bound among its fields. |
| PATCH | `/runs/{runId}` | Close a run, or stop it. |

### log-lines

| Method | Path | Does |
|---|---|---|
| GET | `/log-lines` | Log lines across runs: by run, job or agent, kinds, latest, before or after. |
| POST | `/log-lines` | Add a note in the bot's own words to a run's log, beside the run's own lines. |

### executions

| Method | Path | Does |
|---|---|---|
| GET | `/executions` | Executions, latest first. |
| POST | `/executions` | Launch a run's program in the agent host. |
| GET | `/executions/{executionId}` | One execution. |
| PATCH | `/executions/{executionId}` | Cancel an execution or take the cancel back. |

### overtime-requests

| Method | Path | Does |
|---|---|---|
| GET | `/overtime-requests` | Runs asking for more time. |
| PATCH | `/overtime-requests/{requestId}` | Answer an overtime request. |

### stop-marks

| Method | Path | Does |
|---|---|---|
| GET | `/stop-marks/{jobId}` | Why a job is to stop, if it is. |
| PUT | `/stop-marks/{jobId}` | Mark a job to stop. |
| DELETE | `/stop-marks/{jobId}` | Clear a job's stop mark. |

### pool, seat-requests, reservations and roles

| Method | Path | Does |
|---|---|---|
| GET | `/pool` | The co-worker pool: buckets, seats, waiting work. |
| GET | `/seat-requests` | Open seat requests, by owner or waiting. |
| POST | `/seat-requests` | Ask for a co-worker seat; the tier is resolved from the work key. |
| GET | `/seat-requests/{token}` | One seat request. |
| PATCH | `/seat-requests/{token}` | Move a seat request: admitted, bound to its launched unit, finished. |
| GET | `/reservations` | Held seat reservations. |
| POST | `/reservations` | Reserve seats for an owner. |
| PATCH | `/reservations/{token}` | Release a reservation with its outcome. |
| GET | `/roles` | The roles the pool can lend. |

A seat request carries its `session` where it stands for a project's own turn: the session that turn
continues, or null where the worker's earlier session was cold or past its model's context
(`docs/coworker-pool.md`, 「A project's own session」). A borrowed child's seat names no session.

### places

| Method | Path | Does |
|---|---|---|
| GET | `/places` | Branch places, by node and checkout. |
| POST | `/places` | Take a branch place and its folder. |
| GET | `/places/{placeId}` | One place. |
| PATCH | `/places/{placeId}` | Prepare, remember or retire a place, or record its last write. |

A place's id is its path, percent-encoded in the path segment. A list without a node names the standing
branch checkouts under the data root — one for each slot of each pool, its slot, the role that last ran in
it, whether a pool request holds it now (`taken`) or a branch last answered in it (`remembered`), and the
write it last took. `checkout` narrows the list to that checkout's pool; `nodeName` with `checkout` names
the shadow the node's link keeps of that checkout instead.

### providers, models, pin-checks, tiers and fallback

| Method | Path | Does |
|---|---|---|
| GET | `/providers` | Providers in walk order. |
| GET | `/providers/{providerName}` | One provider. |
| PATCH | `/providers/{providerName}` | Change a provider. |
| GET | `/models` | The registry's models, narrowed by provider or tier. |
| POST | `/models` | Register a model. |
| GET | `/models/{modelKey}` | One model and its health. |
| PATCH | `/models/{modelKey}` | Change a model's registration. |
| DELETE | `/models/{modelKey}` | Take a model's registration away. |
| GET | `/model-catalogue` | The model files the deployment's engines serve, and each engine's latest choice. |
| POST | `/model-catalogue` | Save or remove a catalogue entry, or choose one for its engine. |
| POST | `/pin-checks` | Read a worker pin value: auto, a provider, or a provider's model. |
| GET | `/tiers` | Tiers in ladder order, each with its mapping. |
| PATCH | `/tiers/{tierName}` | Change a tier's mapping for a role. |
| GET | `/fallback` | The fallback roads, the Claude accounts and whether the fallback is answering. |
| PATCH | `/fallback` | Change the fallback roads and the Claude accounts. |

A provider's change throws the owner's switch on it; the last provider left on is not thrown off, since
nothing would answer.

A model is one registration of the registry's provider: the list, the create, the read, the change and the
take-away all read or write the registrations, so the list holds exactly what can be changed and removed. A
model's key names its provider and the model apart — `provider::model`, or the provider as a query and the
model in the path. `provider` names the registry's provider alone, since every other provider's models are
its own catalogue and no route changes them; `tier` narrows the list to the registrations that tier maps to.
A create starts a registration under its own name as its label, at the registry's smallest window, answer
and wait and with no address, and is refused where the name already stands. `tier` and `healthy` are read
back; the model behind a registration is what the change edits.

The model catalogue is a resource of its own, never of `/models`: the files the deployment's own engines
serve, each with its sizes, its source, its nodes and the registration it stands behind, and the latest
choice for each engine. Its entries are not models, and `/models` neither lists nor changes them.

### work-keys, road-blocks and launch-room

| Method | Path | Does |
|---|---|---|
| GET | `/work-keys/{workKey}` | Where a work key goes: provider, model, effort, answer tokens, run bound, run mode. |
| GET | `/road-blocks` | Roads blocked now, by model or worker; none means open. |
| POST | `/road-blocks` | Block a road until a time. |
| DELETE | `/road-blocks/{blockId}` | Open a road again. |
| GET | `/launch-room` | Whether a launch may start, why not, and the jobs and browsers running. |
| PATCH | `/launch-room` | Record that launches are held, and why. |

A block is a resource with an id: its selection — `provider::model` — the moment it opens again in epoch
milliseconds and the reason it was closed. A list returns the blocks whose moment is still ahead, narrowed
to the roads a named model or worker would walk, and a deletion by that id opens the road again.

The launch room answers `held`, every reason it is held, the jobs at work and, of those, the ones whose
place runs a browser. The launcher's own notice is kept beside the machine's reasons and read back with
them; a notice that holds nothing is withdrawn.

### accounts, rest and usage

| Method | Path | Does |
|---|---|---|
| GET | `/accounts` | Accounts: held, kind, home, usage window, rest. |
| PATCH | `/accounts/{accountId}` | Record an account's usage and its rest. |
| GET | `/rest` | Whether the deployment rests, until when, of what kind. |
| PATCH | `/rest` | Set the rest, or clear a rest that has passed. |
| GET | `/stand-in?agent=` | Whether a stand-in answers the agent named while Claude Code's road is closed. |
| GET | `/usage` | Usage by model, agent, user or room over a span, with session contexts. |

The stand-in is asked for an agent, and says whether a stand-in would answer that agent now, which a
consumer gating a room's turn on the rest reads before it lets that turn wait for Claude Code
(`docs/cache-fallback.md`).

An account is reached one of three ways: a directory holding a login, a token file, or an endpoint — an
Anthropic-compatible base URL with a key file and an optional map from the `opus`, `sonnet` and `haiku`
family words to the endpoint's model ids. It may also name a base prompt that a worker's run on it opens
with. The list reports each account's kind, its home and, for an endpoint account, its base URL, key-file
path and model map; a key's content is never reported. Accounts are added and changed through
`PATCH /fallback`'s `add-account` and `update-account` actions, and with `bmo-agency agent --account-add`
and `--account-set`.

Usage groups the jobs whose turns started in the span by the key the query names — the model the job's
turn went out on, the worker (`agent`), the job's holder (`user`) or its room — summing the tokens each
job's model spent, the jobs behind the figure and the span they cover, and the tokens the workers'
sessions on that key currently carry (`contextTokens`). The span is two moments in epoch milliseconds,
yesterday to now where none is given.

### settings

| Method | Path | Does |
|---|---|---|
| GET | `/settings` | Agency's own switches: keepalive, downgrade return. |
| PATCH | `/settings` | Change agency's switches. |

### companions and companion-resets

| Method | Path | Does |
|---|---|---|
| GET | `/companions` | Companion agents, by room. |
| POST | `/companions` | Add a companion agent. |
| PATCH | `/companions/{companionName}` | Change a companion and the rooms it answers in. |
| DELETE | `/companions/{companionName}` | Remove a companion from the deployment. |
| POST | `/companion-resets` | Reset a companion's person ranges and rooms; `dryRun` counts it first. |

### workers, prefix-checks and room-holdings

| Method | Path | Does |
|---|---|---|
| GET | `/workers` | Every worker. |
| POST | `/workers` | Add a worker. |
| GET | `/workers/{workerName}` | One worker: checkout, node, role, sampling, session, runs. |
| PATCH | `/workers/{workerName}` | Change a worker: fields, node placement, sampling, stand it again, ready its bench. |
| DELETE | `/workers/{workerName}` | Remove a worker. |
| GET | `/workers/{workerName}/definition` | The worker's agent definition: graph, trailer, handoff. |

A change that names a node other than the one the worker stands on places it there, and one that names
none brings it back: the node must be one the node service keeps and that takes jobs, the worker must
stand for itself, and the path is written as the node reads it. The checkout this machine reads the
worker through follows the placement. A body that names the node the worker already stands on changes
nothing of its placement.
| GET | `/workers/{workerName}/workbench` | The worker's workbench and scratch folders. |
| DELETE | `/workers/{workerName}/servers` | End the job servers a worker left running. |
| POST | `/prefix-checks` | Which of these files are in a worker's session prefix. |
| GET | `/room-holdings/{roomId}` | What the agency holds of a room. |
| DELETE | `/room-holdings/{roomId}` | Let a room's data go; `dryRun` says what would go. |

### assignments and jobs

| Method | Path | Does |
|---|---|---|
| GET | `/assignments` | Worker assignments, by holder, worker or state. |
| POST | `/assignments` | Assign a worker to an item for the harness holding it, and hold it until the job starts or the hold runs out. |
| GET | `/assignments/{assignmentId}` | One assignment. |
| PATCH | `/assignments/{assignmentId}` | Release an assignment, or hold it longer. |
| GET | `/jobs` | Jobs: recent, by room, by worker, by holder, running. |
| POST | `/jobs` | Run an item on its assigned worker: the harness hands over the item's content and the lease it holds, and the agency launches the worker's session on it. |
| GET | `/jobs/{jobId}` | One job and the tokens its model has spent. |
| PATCH | `/jobs/{jobId}` | End a job with its outcome, or stop it; its assignment is released. |
| GET | `/jobs/{jobId}/prompt` | The prompt the job was launched with. |

A job's `itemId` and `leaseId` are the harness's references, kept as given. The harness renews the lease,
reads the job's progress, questions and outcome from its fields and events, and reports and closes the item
in the job service. Agency calls nothing of the job service and keeps no client of it.

### session-work, session-loads and session-summaries

| Method | Path | Does |
|---|---|---|
| GET | `/session-work/{sessionId}` | The item and results a worker session holds. |
| PATCH | `/session-work/{sessionId}` | Record the item a session serves. |
| PUT | `/session-work/{sessionId}/results/{resultKey}` | Record a result sent into a session. |
| DELETE | `/session-work/{sessionId}/results` | Forget a session's results as it folds. |
| GET | `/session-loads/{sessionId}` | The documents loaded into a session. |
| PUT | `/session-loads/{sessionId}` | Keep a session's loads. |
| DELETE | `/session-loads/{sessionId}` | Forget a session's loads. |
| GET | `/session-summaries/{agentName}` | An agent's kept session summary. |
| PUT | `/session-summaries/{agentName}` | Keep an agent's session summary. |
| DELETE | `/session-summaries/{agentName}` | Clear an agent's session summary. |
| GET | `/session-summaries/{agentName}/task` | The summariser's task for an agent. |

## Events

Agency publishes one stream, `GET /events`. The kinds are:

`agent.changed`, `session.created`, `session.changed`, `session.folded`, `session.deleted`,
`turn.created`, `turn.ended`, `question.created`, `run.created`, `run.changed`, `run.stopped`,
`execution.changed`, `seat-request.changed`, `reservation.changed`, `worker.changed`, `worker.deleted`,
`prompt-document.changed`, `document-move.created`, `overtime-request.created`,
`overtime-request.changed`, `rest.changed`, `fallback.changed`, `account.changed`, `usage.changed`,
`launch-room.changed`, `companion-reset.created`, `place.changed`, `assignment.created`,
`assignment.changed`, `job.created`, `job.changed`, `job.ended`.

A kind agency keeps to its own processes does not cross the stream.

## Hook points

- **`turn-submitting`**: asked before a turn goes out, its answer added to the line.
- **`session-summarising`**: asked when a worker's session closes, its answer the summary kept.

## Tables a consumer shares

- **`room_windows`** is the receiver's: the rooms' listening windows, and agency keeps none.
- **`run_records`** is the receiver's too: what a turn wrote down beside its run, and agency keeps
  none.
- **`runs`, `agent_log`, `agent_executions`** carry a run's ledger, its log lines and the executions
  agency recorded.

## The command

`bmo-agency` is agency's unit command (bmo-cli's `docs/api.md`, 「The unit command」). Asked `nouns`, it prints
the nouns `bmo` dispatches to it, one a line, each marked `home` after a tab where its commands act on the
operations machine. It answers with nothing of the machine read: no settings, no data root and no standard
input, so `bmo` finds it from any directory.

| Noun | Takes | Calls |
|---|---|---|
| `agent` (`agents`) | the agent flags `--help` prints: the provider and account orders, the accounts and their kinds, an account's endpoint, key file, model map and base prompt, the registered models, a route and its model, and a worker's rows and an agent's thinking, effort and gates | the store at the data root, and the service for what it must be told |
| `agent status` (`agent-status`) | `status`, `stop`, `failure`, `submit`, `start` or `shell`, `--place <dir>`, `--hold-tasks`, `--hold-wording` or `--compose-typed` | the agent's place, and the service for a typed line |
| `model` (`models`) | `add`, `set`, `remove`, `use` or `run-use`, with an entry's file, bytes, hash, engine, registration and settings; `--entry` and `--placed` for an installer; `--json` | the store's model catalogue |
| `coworker pool` | `reserve --role <role> [--tier <tier>] --count <n>` or `release` | the store's pool |
| `coworker hook` | the hook's input on standard input | the store's pool |
| `worker fold` | `--worker <worker>`, `--model <model>`, `--all` or `--dry-run` | the workers' sessions, and the store |
| `reload` | `--path <file>`, repeatable, and `--now` | the store, the checkouts and the deploy tree, and the service for the documents moved |
| `node work` (`work-node`) | `setup <node> [--root <dir>] [--install]`, `check`, `sync`, `runs <node> on\|off` or `remove <node>`; no step lists the workplaces | the store, the node service and herdr |
| `node claude-token` | `<account>`, and `--paste` to take a token from standard input | the store, and the account's program on this machine |
| `node pane` (`node-pane`) | `<node>` | the node service, and the login its pane starts |
| `dev-server` | `start`, `restart` or `stop <worker>`; `list [--sweep]`; `pane <worker>` | the worker's workbench and dev-server pane, and the store |
| `dev-pane` | `<worker>` | the worker's dev-server pane, and the store |
| `companion follow` | `<room> --agent <agent> [--version <version>]` | the store, as the room is kept and its turns are appended |
| `companion reset` | `<agent>`, `--conversation`, `--person`, `--dry-run`, `--alone <rooms>` and `--shared <rooms>` | the service's `/companion-resets` route |
| `claude batch` | `--model`, `--effort`, `--config-dir`, `--system-file`, `--title` and `--bound-seconds`, with the batch on standard input | the provider's program on this machine |
| `live-claude` | `--purpose <what it measures and why>`, or `--list` | the store's run ledger |
| `run list` | `--json`, or `--full` for every row whole | the store, herdr, and the machine's proc files |
| `run stop` | `<kind>:<id>` — `unit:`, `execution:` or `item:` — `--why <reason>` for an item, `--all`, or no target to list what is running | the store, herdr and systemd |
| `machine memory` | `--runtime <runtime>` and `--unit <unit>`, each repeatable, and `--json` | the machine's proc root |

- **Every noun prints English** and ends with 0, with 1 where its work failed, and with 2 where its words
  are wrong; `--help` prints a noun's whole usage.
- **`agent status` and `agent-status` are one noun:** the two-word spelling is read as the command's own
  `agent-status`, and both reach the same hooks and shell guard.

## The harness plugin

The service publishes `bmo-agency` beside its client: `bmo-agency mcp` serves the MCP server, and each
tool is a thin call of one route through the agency's own client.

- `assign_worker` calls `POST /assignments`.
- `run_job` calls `POST /jobs`.

## The clients

`crates/client` is the Rust client (`bmo-agency-client`) and `packages/client` the TypeScript package
`@agency/client`, both generated from `docs/openapi.json`: every body as a type and a typed call for every
route. A caller uses one of them and writes no client of its own.

- **Installing the package.** `bun pm pack` in `packages/client` writes `agency-client-<version>.tgz`, and a
  downstream checkout names that file in its dependencies as a `file:` address. The pack carries the
  manifest, the entry point and the generated types; `openapi-fetch` comes with it as the package's one
  dependency.
- **Version and types.** The package carries the repository's version, and a test fails where the
  committed types differ from what `docs/openapi.json` generates.

## Calls agency makes

Agency reaches upstreams only, through what their repositories publish: brain, graph, node, arbiter,
pocket, testbed and chat through their clients, and cache-warmer through the schedule library
`bmo-cache-warmer-schedule`.
Of the brain it asks a companion's holding let go or counted, and the discipline a chat launch's system
prompt carries.

Each surface is a crate of its own repository, declared in the workspace manifest and allowed by
`tests/dependencies.rs`; every other crate of another repository, a downstream's client among them,
stays outside the boundary.

It calls no downstream. It calls no job service. The gate and the receiver reach it through the two hook
points above.

## What the redesign took out

`GET/POST /digests`, `GET/POST /searches`, `GET /searches/{searchId}` and `GET/PATCH /research-settings`,
with the tools `browse`, `remote_browser`, `research_branch`, `unread_pages`, `song` and `news_due`, are
gone: each only wrapped a call into an upstream the caller reaches directly. A digest, a research, a
browse and the morning news are ordinary sessions started with `POST /sessions` and the run's prompt
document, and the session calls the upstreams it needs through their own MCP servers.

The agency's job client and its lease taking are gone; the harness takes the lease and hands the item's
content to `POST /jobs`. The hooks the job service and the brain registered here are gone; agency calls
the brain's client itself. The word-chain procedure and its mention among the agent documents are
retired.
