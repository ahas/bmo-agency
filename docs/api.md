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
  the first release the version is `0.1.0` plus `-rev.N`, and the current version is `0.1.0-rev.59`.
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
- The same file names the variables a line hands the session records of its runs
  (`docs/tools.md`, 「The session record」): for the chat line, `BMO_GATE_URL`, the gate's address, and
  `BMO_GATE_TOKEN_FILE`, the path of the gate's token file under the data root's `env/`.
- The same file names the tool host's port, `BMO_MCP_HOST_PORT`, with the value the tool host's own
  `env/pocket.env` gives it (3290 where neither names one): every MCP configuration agency writes points
  its tool host servers at that port, and a line whose port is not the host's has no tool of the host's,
  a dsh session refusing to open on it.
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

The hook point is `turn-submitting`, registered by the gate and the receiver. `road-verbs` is what the procedures tool asks a room's road.

### agents

| Method | Path | Does |
|---|---|---|
| GET | `/agents` | Every agent, each with the login account that owns it. |
| POST | `/agents` | Register an agent of any kind; one that converses is given its brain. |
| GET | `/agents/{agentName}` | One agent: role, gates, reasoning, voice, reset point, stand-in. |
| PATCH | `/agents/{agentName}` | Change an agent's gates, reasoning, role or voice. |
| DELETE | `/agents/{agentName}` | End an agent's registration; `forgetBrain` lets its brain go with it. |

- **Every agent is a registration row** (`docs/agents.md`), and these routes add, change and end any of
  them: the chat agent and the companions, a worker, a gate and a runner alike.
- **An agent is owned by a login account,** `ownerAccount`: the id of the user it belongs to, the same id
  an account's `user_id` names (`docs/store.md`). None means the administrator's, and every agent
  registered without an owner reads so. `GET /agents` and `GET /agents/{agentName}` answer it on every
  agent.
- **Who may register or end which agent is the caller's to judge.** The agent screen is the
  administrator's, and a user registers agents of their own under a permission the caller holds; agency
  writes the owner it is handed and judges no permission.
- **`POST /agents` takes the registration,** each field judged in this order and nothing written until
  all have passed:
  - `name`, the agent's key: a lowercase letter, then lowercase letters, digits and hyphens, forty
    characters at most (the companions policy's `name_pattern`), and held by no agent yet;
  - `label`, given;
  - `kind`, one the roles policy names a role for: `chat`, `worker`, `runner` or `gate`;
  - `role`, a role the policy declares, the kind's own where none is given (`docs/agent-roles.md`);
  - `ownerAccount`, none where the caller gives none;
  - for any kind, `icon`, `thinking`, and `effort`, a level the reasoning policy holds;
  - for the `chat` kind, its pin and its worker, judged as a companion's are (`docs/companions.md`,
    「Where a companion and its rooms come from」): `provider`, `model` and `worker`. The row is written
    as a companion added on the console is: the `room` scope, the `pieces` shape, not the default
    answerer, the brain folder `agents/<name>`, and no prompt folder;
  - for the `worker` kind, `cwd`, the code tree's root where none is given, and `nodeName`.
  - A row of the `runner` or `gate` kind is its name, label, role and owner: what its runs
    go out on is read by its name off the work keys (`docs/model-tiers.md`), so it runs nothing until a
    work key's profile names it.
  - No agent registered here is seeded or the default answerer.
- **A refusal names its field:** 400 `bad-request` with `field` beside `error`, and 409 `name-taken`
  for a name an agent already holds.
- **The answer is 201,** `agent`, the agent as `GET /agents/{agentName}` answers it, and `brain`, what
  became of its brain: `made`, `kept` where one already stood under its key, `none` where no brain is
  deployed or the agent is given none, or `failed`, the failure logged. The registration stands whatever
  the brain answers.
- **An agent of the `chat` kind, or holding the chat role, is given its brain as it is registered,**
  through the brain (「Calls agency makes」): filed under its owner's id as the account key where it names
  an owner, and under `AGENCY_BRAIN_ACCOUNT` where it names none.
- **`DELETE /agents/{agentName}` ends a registration,** taking `force`, `forgetBrain` and `rooms` in its
  query:
  - 404 `not-found` for no agent of the name;
  - 409 `seeded` for an agent the deployment seeds or the default answerer, whatever `force` says, since
    the deployment stands it again;
  - 409 `running` for an agent a turn holds now — its agent lock, or a worker's pane lock — unless
    `force`, which ends it under the turn.
  - A companion's conversation is reset first, as its removal resets it (`docs/companions.md`, 「Starting
    it afresh」), each of the `rooms` named read as a room it shares, so the rooms keep their record.
  - Then in one write its settings, its base prompt choice, its session reset asks, its gate switches, its
    sampling, its reset point, a worker's sessions and the words said to it, and its row; then what
    stands for it: its program, its workspace, its place and home folder, its execution rows, the agent
    host's records of it and its lock. What could not go is logged.
  - **Its brain is data and stays,** unless `forgetBrain` is true: then the brain is let go whole, with
    the harness records kept in its folder.
  - The answer is 204.

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
| GET | `/sessions/{sessionId}` | A session's state: provider, model, place, holder, why the prefix changed, the fold ratio. |
| PATCH | `/sessions/{sessionId}` | Move a conversation's provider and model, or a thrown-away session's model, or hand a worker's session over to another holder. |
| DELETE | `/sessions/{sessionId}` | End a session, or clear a room's session. |
| POST | `/sessions/{sessionId}/turns` | Send a turn into a session; words to a worker are a turn into its session. |
| GET | `/sessions/{sessionId}/turns/{turnId}` | A turn and what it has written so far. |
| PATCH | `/sessions/{sessionId}/turns/{turnId}` | Interrupt a turn. |
| POST | `/sessions/{sessionId}/turns/{turnId}/inputs` | Push words into a working turn. |
| POST | `/sessions/{sessionId}/answers` | Answer a question a turn asked. |
| POST | `/sessions/{sessionId}/folds` | Fold a session, compacting its context. |

A job's run is a worker's session turn with the work parts in the turn's context. A session is started at
one scope — `agent`, `room`, `worker`, `apart` or `conversation` — and the scope says what is
kept. Its id names the agent and the scope: the agent's name alone for an agent's session,
`<agent>@<room>` or `<agent>@worker` for a room's or a worker's, and a
number the service gives it for a thrown-away session (`<agent>@apart:<n>`) and a conversation
(`<agent>@conversation:<n>`), so every start of one of those is a session of its own.

A conversation is the caller's: its start names the provider, the model and, where it names one, the
folder its turns go out on, and its turns are ordinary prompts with the context around them and no work
parts. A provider whose program keeps a session continues the one its last answered turn left; OpenRouter
keeps its dsh session inside bmo-tui's pane, which carries the next turn although agency stores no id for
it. The session stands in the server's memory from its start to its end.

A conversation whose agent is a gate work key carries a gate's one turn: the turn's `gate` part holds the
gate's instructions as the program's whole system prompt, whether it thinks, and the model, effort and
answer room its mapping resolved to. The turn goes out with no tools and no MCP server, and its answer is
the gate's verdict. On a road whose program carries its own turns it goes into a place standing for the
program its ask composes, whose session goes on across judgements up to a bound, several such places
running at once (`docs/agent-control.md`, 「A gate's places」); on every other road it keeps no session.

A turn's context may name `round`, the receiver's name for the round of the room the turn belongs to,
which the turn's run keeps (「runs」 below).

A turn on the chat line ends its system prompt with the base persona: the base layer of the agent's own
brain, read at the turn's launch (「Calls agency makes」). The turn context's `basePersona` is the fallback,
standing in that place only where the brain gives none (`docs/sessions.md`, 「Document reload」).

A room's session is readable by its id from its start, before its first turn has opened a generation:
the read answers the provider, model and place its agent's row names, at generation 0, and its first
turn opens the first generation on that road. A start writes nothing of the session, so the list of
sessions names a room's once its first turn has opened one.

A conversation moves its provider and model on the change route, each left as it stands where the body
names only the other, and starts its next turn on a fresh session where the road keeps one. A
thrown-away session's model moves the same way, while its provider stays Claude Code's, where its turns
go out. A hand-over is a worker's alone: it closes the worker's held
session on the model and drops the record, handing nothing to the next turn, and the holder the body
names is kept for the session's state; a worker with nothing open keeps the holder alone. An agent's and
a room's session go out on the provider and model their agent's row names, and that row is where those
move. `holder` and `prefixChangeReason` are read back on the session wherever it is listed or read. A
worker's state answers the provider its session stands on: the provider the worker is pinned to, else the
one its held session last answered on, else `claude-code`.

A session's state carries `foldRatio`: the share of its model's window its context may fill before the
session is folded once the turn that crossed it is over. It is the fold-ratio table's value for the
provider the state names and the tier its agent's work key is assigned (「fold-ratios」 below), read
afresh with each read, so a change on the console reaches the next read of every session. It carries
`tier` beside it: that tier, by its name, or null where the key is assigned none, so a consumer keyed by
tier matches the session by the tier the agency reads it at.

A turn sent is carried to its end whether or not its sender still waits on the send: a sender that lets
go of the request, or goes down while the turn works, leaves the turn running, and the turn ends as it
would have, its run closed and its end published as `turn.ended`.

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
| GET | `/runs` | Runs by search: live, recent, by agent, room, round, model, job, worker, names; with the tokens spent so far. |
| POST | `/runs` | Open a run in the ledger. |
| GET | `/runs/{runId}` | One run, its clock bound among its fields. |
| PATCH | `/runs/{runId}` | Close a run, or stop it, or attach the message its answer went out as. |

A run is a row of the ledger (`docs/run-records.md`, 「The run ledger」): a unit the job unit opens and
closes, a job's turn, which agency's job runner opens and closes, and a chat turn's run, which agency
opens as the turn starts and closes as it ends. Each answers
`roomId`, the room it ran for; `round`, the round the receiver named in the turn's context; and
`messageId`, the message its answer went out as once the receiver attaches it, the latest message the
turn answers until then, or null where the turn named neither. A chat turn's run and a job turn's answer
what the turn spent once it is closed: `usage`, its `inputTokens`, `outputTokens`, `cacheReadTokens` and
`cacheWriteTokens` summed over the models that answered, and `spentTokens`, the four together; both are
null on a run whose road reported no figures, and on an open run. Every turn of a round, each question,
item and check of the draft process among them, is a run of its own under the round. The list narrows by `agentName`, by
`roomId`, by `round`, and by `names`, a comma-separated list each matched against a run's unit and its
round. A change naming `messageId` alone attaches the message, on an open or a closed run alike, and
leaves the run's closing as it stands.

### log-lines

| Method | Path | Does |
|---|---|---|
| GET | `/log-lines` | Log lines across runs: by run, job or agent, kinds, latest, before or after. |
| POST | `/log-lines` | Add a note in the bot's own words to a run's log, beside the run's own lines. |

`runId` names a run as its lines were written under it, or a run of the ledger by its id, whose lines
are the ones written under its unit. Each line answers the ledger's `runId` where the ledger holds its run, and
that run's `roomId`, `round` and `messageId`, null where it holds none.

A run's tool calls are its `tool` lines, each naming the tool and carrying a `ref`, and the `tool-result`
line with the same `ref` saying how the call ended in its `state`, `succeeded` or `failed`. A run on a
road whose program carries its own turns has them too, read off that program's record: there a call's
line carries no arguments and its result's text says how long the call ran (`docs/run-records.md`,
「Agent log」).

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

The fallback's view answers, beside the roads and the work keys, `active`, the agent serving the chat
now: the `provider` its road answers on, the `account` it walks to where the provider holds logins, its
`accountName`, how many `accounts` are on, that slot's two headline windows, `session` and `week`, and
its `balance`, the same shape the accounts list answers, for a provider or an endpoint account that
offers one and null otherwise;
and `accounts`, every Claude account with its kind, path, place, switch, login standing, whether it is
chosen and serving, its rest and its own `session`, `week` and `balance`, the same the accounts list
answers.

A provider's change throws the owner's switch on it; the last provider left on is not thrown off, since
nothing would answer.

Each work key's member of the fallback's `routes` answers, beside its `routeMode` and `account`, its
`registration`: the registration the key runs on at the registry's provider in place of its tier's cell,
or null where it names none (`docs/own-llm.md`, 「A key's own registration」). `PATCH /fallback` with the
action `set-route-registration`, the `key` and a `registration` sets it; `auto`, an empty name or null
takes it away. A registration the registry does not hold, or holds with no address, is `bad-request`, and
the answer is the key and the registration it now names. Every resolution of the key on the registry's
provider names that registration as its model, `GET /work-keys/{workKey}` among them, with or without a
`tier` in its query.

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

### fold-ratios

| Method | Path | Does |
|---|---|---|
| GET | `/fold-ratios` | Every provider's fold ratio at every tier, each with where it comes from. |
| PUT | `/fold-ratios/{provider}/{tier}` | Set a provider's fold ratio at a tier. |

A fold ratio is the share of a model's window a session's context may fill before the session is folded
once the turn that crossed it is over (`docs/sessions.md`, 「Session compaction」), kept per provider and
abstract tier. The list holds one member for every provider agency declares and every tier in ladder
order — the grid a console shows — each with its `provider`, `tier`, `ratio` and `source`: `set` where
the table holds a row, `provider` where the row is missing and the provider's own declared default
stands, `default` where the provider declares none either and 0.85 stands. A set takes `ratio`, a
number above 0 and at most 1, writes the row and answers the member; a provider or a tier that is not
declared is `not-found`, a ratio outside that range `bad-request`. A set reaches the next turn that
reads it; no session is folded by the set itself.

### work-keys, road-blocks and launch-room

| Method | Path | Does |
|---|---|---|
| GET | `/work-keys/{workKey}` | Where a work key goes: provider, model, effort, answer tokens, run bound, run mode. |
| GET | `/road-blocks` | Roads blocked now, by model or worker; none means open. |
| POST | `/road-blocks` | Block a road until a time. |
| DELETE | `/road-blocks/{blockId}` | Open a road again. |
| GET | `/launch-room` | Whether a launch may start, why not, and the jobs and browsers running. |
| PATCH | `/launch-room` | Record that launches are held, and why. |

A work key's read that names no `provider` is resolved first on the provider the key's route is set to,
where that provider is on, and then walks each provider on in the owner's order, and the first that serves
the key answers. The walk passes over a provider whose program does not carry the key's line (a gate's
`gate`), the registry's provider where the registration the key's tier maps to there is not walked, and a
model that is not healthy now: its road shut by a failure still standing or, for Claude Code, by the
deployment's rest, or a registration whose engine says it stands aside. So a gate, which reads its road this
way, goes out on the provider its key is set to, and a key set to the registry's provider with a registration
of its own goes out on that registration.

`runMode` is the road the key stands on: the route the owner wrote for it, else its profile's default —
`auto` for the walk, a provider's name, or `laya` for a gate whose classifier answers it. A gate on `laya`
runs no model the read names; the provider and model it answers are where the walk would take the key, the
road its fallback LLM begins on where it names none.

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
| GET | `/usage` | Usage by model, agent, user, room or round over a span, with session contexts. |

The stand-in is asked for an agent, and says whether a stand-in would answer that agent now, which a
consumer gating a room's turn on the rest reads before it lets that turn wait for Claude Code
(`docs/cache-fallback.md`).

An account is reached one of three ways: a directory holding a login, a token file, or an endpoint — an
Anthropic-compatible base URL with a key file and an optional map from the `opus`, `sonnet` and `haiku`
family words to the endpoint's model ids. It may also name a base prompt that a worker's run on it opens
with. The list reports each account's kind, its home and, for an endpoint account, its base URL, key-file
path and model map; a key's content is never reported. Each directory and token account carries its
two headline windows, `session` and `week`, as the usage-window store holds them under the account
(`docs/usage-windows.md`): each a window's `name`, `utilization` (0 to 1), `resetsAt` and `seenAt` in
epoch seconds, `durationMinutes` and `limitName`, or null where the account holds no such window; an
endpoint account holds none. An endpoint account whose endpoint is a provider's that offers a balance
carries `balance`, what it has left as its balance reader last read it (`docs/usage-windows.md`,
「Balances」): `amount`, `currency` and `seenAt` in epoch seconds, or null where none has been read; a
directory or token account's is null. `window` is the usage a report named, answered by the report's
change and null on the list. Accounts are added and changed through
`PATCH /fallback`'s `add-account` and `update-account` actions, and with `bmo-agency agent --account-add`
and `--account-set`.

Usage groups the jobs whose turns started in the span by the key the query names — the model the job's
turn went out on, the worker (`agent`), the job's holder (`user`) or its room — summing the tokens each
job's model spent, the jobs behind the figure and the span they cover, and the tokens the workers'
sessions on that key currently carry (`contextTokens`). `round` groups the chat turns' runs instead, those
started in the span that name a round, by that round: each group's `tokens` is what its turns spent,
`turns` how many there were, `inputTokens`, `outputTokens`, `cacheReadTokens` and `cacheWriteTokens` the
spend apart, and `firstAt` and `lastAt` the span they cover; its `jobs` is 0 and its `contextTokens` null. The span is two moments in epoch milliseconds,
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
| GET | `/workers/{workerName}` | One worker: checkout, node, role, prefix documents, sampling, session, runs. |
| PATCH | `/workers/{workerName}` | Change a worker: fields, node placement, sampling, stand it again, ready its bench. |
| DELETE | `/workers/{workerName}` | Remove a worker. |
| GET | `/workers/{workerName}/definition` | The worker's agent definition: graph, trailer, handoff. |

A worker carries `prefixDocuments`, the paths of the documents its session's prefix carries, in order
(`docs/worker-bench.md`, 「Prefix documents」); `POST /workers` and `PATCH /workers/{workerName}` take
the list whole, an empty list clearing it, and refuse an empty path or one that climbs with `..`.

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
| POST | `/jobs` | Run an item on its assigned worker: the harness hands over the item's content and the lease it holds, and the agency launches the worker's session on it; refused while no login can carry it. |
| GET | `/jobs/{jobId}` | One job and what its turn spent, `usage`. |
| PATCH | `/jobs/{jobId}` | End a job with its outcome, or stop it; its assignment is released. |
| GET | `/jobs/{jobId}/prompt` | The prompt the job was launched with. |
| POST | `/instruction-texts` | The work-order step's line composed from an instruction. |

A job's `itemId` and `leaseId` are the harness's references, kept as given. The harness renews the lease,
reads the job's progress, questions and outcome from its fields and events, and reports and closes the item
in the job service. Agency calls nothing of the job service and keeps no client of it.

- **A job may carry an instruction,** `instruction`, beside its item or inside it — `repository`,
  `documents`, `sentences` and `checks`, every one required and none empty (`docs/worker-bench.md`, 「A
  small-model instruction」); one that lacks a field is refused 400 naming it. The job keeps it, answers
  it, and its prompt renders it under the item's title. `POST /instruction-texts` answers `text`, the
  one line a work-order step is written as, composed from an instruction.
- **A job is launched only where a login can carry it.** Where every Claude login that carries the job's
  work key is closed — its window full, resting, or waiting for a person to sign in again — `POST /jobs`
  answers 503 `no-login-open`, naming the logins and the moment the first of them reopens, or that each
  waits for a person; nothing is written and the assignment stays held. A work key no login carries goes
  out as it stands.
- **A job ends by itself as its turn ends.** A turn that answered ends the job `ended` with its answer as
  the report. A turn that did not answer ends it `ended` with its outcome (`failed`, `blocked`, `stopped`
  or `reprepared`) and the reason the turn gave as the report. A launch that never reached its turn — no
  session, no line, no herdr — ends it `stopped`, outcome `failed`, with why as the report. Every end that
  is not an answer is also written as a note into the job's log, so the job's lines say why it ended.
- **A job's turn is a run of the ledger.** As the turn goes out, the job's runner opens a run under the
  job's run name, its worker as the agent and its room, and the job carries it as `runId`; as the turn
  ends the run is closed with how it ended, the tokens its context held and what it spent. A job answers
  `usage`, what its turn spent: `inputTokens`, `outputTokens`, `cacheReadTokens` and `cacheWriteTokens`,
  each summed over the models that answered it, and `tokenCount`, the four together. `usage` is null, and
  `tokenCount` 0, until the turn ends and where its road reported no figures.
- **A job's end releases its assignment,** whichever way it ended, so its worker is free for the next
  assignment. A `running` assignment whose job has ended is released as the assignments are next read.

### session-work and session-loads

| Method | Path | Does |
|---|---|---|
| GET | `/session-work/{sessionId}` | The item and results a worker session holds. |
| PATCH | `/session-work/{sessionId}` | Record the item a session serves. |
| PUT | `/session-work/{sessionId}/results/{resultKey}` | Record a result sent into a session. |
| DELETE | `/session-work/{sessionId}/results` | Forget a session's results as it folds. |
| GET | `/session-loads/{sessionId}` | The documents loaded into a session. |
| PUT | `/session-loads/{sessionId}` | Keep a session's loads. |
| DELETE | `/session-loads/{sessionId}` | Forget a session's loads. |

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
Of the brain it asks an agent's own brain made as the agent is registered, found by the agent's key,
let go whole or only found for a preview, the discipline a chat launch's system prompt carries, and the
base layer of the agent's persona that prompt ends with. Of the tool host it asks, on its control socket
with the issuer token, a session token for each program and run that holds a capability, which it takes
back, a namespace's listing and facts, and the paragraph on finding and using tools that a chat launch's
system prompt carries.

- **Where the brain is.** `AGENCY_BRAIN_URL` names its address and `AGENCY_BRAIN_TOKEN` the token it is
  called with. With no address, no agent has a brain: nothing is made, counted or let go.
- **The account a brain is filed under.** The brain keeps one brain per agent, keyed by an account key
  and the agent's key, and never makes one by itself. A brain agency makes carries, as the account key,
  the id of the login account owning the agent where it names one, and otherwise the key
  `AGENCY_BRAIN_ACCOUNT` names; as the agent's key, the agent's registered name — the key the brain tool
  and the gate look a brain up by. The brain reads no meaning into the account key. With no account to
  file it under, agency makes no brain and logs that it made none.
- **The base persona.** At a chat launch the agent's brain, found by its key, is asked for its persona
  rendered of the layer named `persona` alone; a brain that holds no such layer gives none. The tone and
  room layers are no part of it: they stay in the room's block the caller composes.
- **The persona's values.** A persona piece keeps placeholders: two opening braces, a key of lowercase ASCII
  letters, digits and hyphens joined by dots, and two closing braces, such as `{{owner.name}}`. The
  deployment keeps their values outside every repository, in a TOML file whose path
  `AGENCY_PERSONA_VALUES` names; a key's dots walk the file's tables to a string, so `owner.name` is the
  `name` of its `[owner]` table, and nothing else of the file is read. With no file named, a file that does
  not read, or a key it holds no string for, the placeholder stays unfilled and the brain's persona is not
  used.
- **Making an agent's brain.** As a companion is added, or an agent given a brain is registered
  (「agents」), the brain is asked for the agent's key first; where it answers one, nothing is made. A brain
  that cannot be reached, or refuses, leaves the agent registered and the failure logged.
- **The tool discipline.** At a chat launch whose session is handed the toolbook, the tool host's
  `GET /discipline` is read through its client and its text is put into the system prompt whole; agency
  writes nothing of its own about the toolbook. The control socket is the one `BMO_MCP_HOST_SOCKET` names,
  else `bmo/mcp-host.sock` under the user's runtime directory. A host that cannot be reached, or refuses,
  leaves the paragraph out and the launch logs one sentence saying so.

Each surface is a crate of its own repository, declared in the workspace manifest and allowed by
`tests/dependencies.rs`; every other crate of another repository, a downstream's client among them,
stays outside the boundary.

It calls no downstream. It calls no job service. The gate and the receiver reach it through the hook point
above.

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
