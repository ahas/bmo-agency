# The boundary API

This document is agency's boundary: every route it exposes, every event it publishes, every hook a
consumer registers, and the tables whose shape a consumer reads or writes. A consumer reaches agency
only across this boundary and never into a crate or a module behind it; agency reaches a consumer only
by publishing an event or calling a hook the consumer registered. What a turn is told, whom an agent
answers, which role and model a piece of work needs, and whether an answer may go out are the
consumer's, decided above this boundary and handed in; agency supplies, carries and controls, and judges
none of it.

`docs/openapi.json` is the machine contract: every route's parameters, bodies, answers and codes, the
events, the three tools of the harness plugin, and the removed and moved-out members. It is generated from
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

- The contract carries the repository's one version; the workspace manifest is its source, and the current
  version is `0.1.0-rev.81`. It moves only at a release the owner declares by name (bmo-chat's
  `docs/conventions.md`, 「Versions」); a deployment is told apart by its commit.
- A change to this document and a change to the boundary go in one commit. A change to a route, an event, a
  hook or a shared table's contract repairs this document in the same commit.
- The major moves on a removed or renamed route, field or event, the minor on an addition, the patch on
  anything else. Before the first release the revision moves instead.

## The control port

- Every route is answered by agency's server, `bin/agency-server`, on the loopback, on the port
  `AGENCY_CONTROL_PORT` names (3220 where it names none), under `Authorization: Bearer <token>` with the
  token `AGENCY_CONTROL_TOKEN` holds. A request without the token is answered 401.
- Both variables stand in the data root's `env/agency.env`. A process of agency's whose environment holds
  neither reads them there. A program agency starts is handed the token as the file it stands in,
  `AGENCY_CONTROL_TOKEN_FILE` (`docs/agent-control.md`, 「A token is handed as the file it stands in」), and
  every reader of the token reads that file where the variable is not set.
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
- **A field a body may leave out is optional in the document and the clients,** and a body that leaves it out
  asks nothing of it: a change leaves it as it stands, an ask takes its default. A caller sends only what it
  means; where a route reads `null` as a value of its own — clearing a field, `auto` — the route says so.
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
| PATCH | `/agents/{agentName}` | Change an agent's gates, reasoning, role, voice or model. |
| DELETE | `/agents/{agentName}` | End an agent's registration; `forgetBrain` lets its brain go with it. |

- **Every agent is a registration row** (`docs/agents.md`), and these routes add, change and end any of
  them: the chat agent and the companions, a worker, a gate and a runner alike.
- **An agent is shown with its kind's mark,** `icon`: the mark agency's policy names for its kind
  (`policy/roles.toml`, `[icons]`), so every chat agent, every worker, every runner and every gate looks
  alike wherever an agent is answered — its row, a companion, a session, a worker. A row's own `icon` is kept
  as written and answered only for a kind the policy names no mark for.
- **An agent is owned by a login account,** `ownerAccount`: the id of the user it belongs to, the same id
  an account's `user_id` names (`docs/store.md`). None means the administrator's, and every agent
  registered without an owner reads so. `GET /agents` and `GET /agents/{agentName}` answer it on every
  agent.
- **`GET /agents` answers `roles` beside the agents:** every role the store holds (「agent-roles」), each
  with its `name`, its `code`, its English `label` and its `bundles`, each bundle with its `name`, its `code` and its
  `label`. A code is the kebab-case word a screen words the role or bundle by in its own language: a role's
  is its name, and a bundle's its name with each dot a hyphen, so `room.chat` is `room-chat`.
- **Who may register or end which agent is the caller's to judge.** The agent screen is the
  administrator's, and a user registers agents of their own under a permission the caller holds; agency
  writes the owner it is handed and judges no permission.
- **`POST /agents` takes the registration,** each field judged in this order and nothing written until
  all have passed:
  - `name`, the agent's key: a lowercase letter, then lowercase letters, digits and hyphens, forty
    characters at most (the companions policy's `name_pattern`), and held by no agent yet;
  - `label`, given;
  - `kind`, one the roles policy names a role for: `chat`, `worker`, `runner` or `gate`;
  - `role`, a role the store holds, the kind's own where none is given (`docs/agent-roles.md`);
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
- **`PATCH /agents/{agentName}` changes the fields the body names:** `gates`, `reasoning`, `role`,
  `voice`, and `model`, the agent's own model (`docs/model-tiers.md`, 「An agent's own model」):
  - `{"method": "fixed", "provider": <provider>, "model": <model>}` pins an agent of any kind to one model
    of a provider: a model the Claude CLI's catalogue names on `claude-code`, a member of a provider's own
    road, an offered registration on `registered` — one with an address or naming a card service;
  - `{"method": "fixed", "registration": <name>}` pins it to a registered model by the registration's
    name alone;
  - `{"method": "route"}` lets the supervisor key's road carry it and clears its pin, and is taken by a
    chat agent alone — the chat agent and every companion alike.
  - A refusal of the model is 400 `bad-request` with `field` beside `error`: `model.method` for a method
    that is neither or a route asked for an agent that is no supervisor, `model.provider` for a provider
    agency does not carry or one whose program does not carry the agent's line, `model.model` for a
    model the provider does not hold — a catalogue that reads and does not name it, a member its road does
    not hold, a registration with no address — and `model.registration` for a name the registry holds no
    registration under. A catalogue that cannot be read refuses nothing. A worker's standing program is
    stood again on a new pin.
  - A `role` the store does not hold is refused 400.
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

### agent-roles

| Method | Path | Does |
|---|---|---|
| GET | `/agent-roles` | Every role an agent may hold, and every bundle a role may give. |
| POST | `/agent-roles` | Add a role. |
| PATCH | `/agent-roles/{roleName}` | Change a role's label, bundles, bound, prompt or tier. |
| DELETE | `/agent-roles/{roleName}` | Remove a role no agent holds and no kind is for. |

- **The roles are the store's rows** (`docs/agent-roles.md`, 「The roles」), and these routes are the one
  way to change them; `GET /roles` is the co-worker pool's and lists the co-worker roles, not these.
- **`GET /agent-roles` answers `roles` and `bundles`.** Each role, in the order a screen lists them, is
  `name`; `label`; `bundles`, each `name` and `label`; `maxConcurrent`, its bound or null; `prompt`, its
  text or null; `tier`, the assignment of its work key `role:<name>` or null where none is assigned;
  `agents`, how many agent rows hold it; and `kindDefault`, whether a kind's own is it. `bundles` beside
  them is every bundle the policy declares, each `name` and `label`, in the policy's order.
- **`POST /agent-roles` takes `name`, `label` and `bundles`,** and may add `maxConcurrent`, `prompt` and
  `tier`, and answers 201 with the role as the listing answers it. It is refused 422
  `unprocessable-entity`, with one English sentence and nothing written, for:
  - a name out of the roles policy's pattern or past its length, or one a role or a co-worker role holds;
  - an empty label, or one past the policy's length;
  - `bundles` that is no list of names, or a bundle the policy does not declare;
  - a bound that is no whole number of one or more;
  - a prompt that is not text;
  - a tier the ladder does not hold.
- **A tier given is the role's key's assignment,** written in the same write as the role, as the tier
  settings write one (「work-keys, road-blocks and launch-room」), so the key stands in `GET /work-keys` from
  then on and is changed there as well as here.
- **`PATCH /agent-roles/{roleName}` changes the fields the body names** and answers 200 with the role: any
  of `label`, `bundles`, `maxConcurrent`, `prompt` and `tier`, each judged as `POST` judges it. A
  `maxConcurrent` or a `prompt` given as null clears it, and a `tier` given as null takes the key's
  assignment away. 404 `not-found` for a role the store does not hold.
- **`DELETE /agent-roles/{roleName}`** answers 204, the role and its key's assignment gone in one write;
  404 `not-found` for a role the store does not hold, and 409 `conflict` while an agent row holds it or a
  kind's own is it.

### prompt-documents

| Method | Path | Does |
|---|---|---|
| GET | `/prompt-documents` | Deployed prompt documents: base prompts, rulebooks, personas, agent definitions, system prompts. |
| GET | `/prompt-documents/{documentName}` | One prompt document with its version. |
| PATCH | `/prompt-documents/{documentName}` | Change a base prompt or its chooser. |
| GET | `/rulebooks` | The shared rulebooks named, as the deploy tree holds them. |

### chat-documents

| Method | Path | Does |
|---|---|---|
| GET | `/chat-documents` | The chat agent's own documents the owner may keep — its opening, the absolute rules and the manual — each with the text it stands as, where that text comes from, and its version. |
| PATCH | `/chat-documents/{documentName}` | Save the owner's text of one document, or reset it to its file. |

- **A document stands as the owner's text where one is saved,** and as its file under the deploy tree
  where none is; `source` says which, and `version` is the moment the stored text was last saved.
- **The three names** are `opening`, `absolute-rules` and `manual`; any other name is refused 404.
- **A save** takes `text` of 1 to the bound the base prompts' policy states, and `by`, the caller's
  name; an empty or oversized text is refused 400. **A reset** takes no text and lets the file stand
  again.
- **The chat line reads them afresh at every launch,** so a save reaches the next turn with no
  restart, and a turn keeps its conversation under the changed system prompt as any change of it does
  (`docs/agent-documents.md`, 「The chat agent」).

### document-moves

| Method | Path | Does |
|---|---|---|
| POST | `/document-moves` | Word that session documents moved, so sessions read them again. |

### sessions

| Method | Path | Does |
|---|---|---|
| GET | `/sessions` | Every session, by agent or scope. |
| POST | `/sessions` | Start a session for an agent at a scope. A run on a runner line is a thrown-away session handed that line's instructions. |
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

A thrown-away session's turn is a co-work branch's, or a run on a runner line whose instructions the session's
caller handed in (below). Every thrown-away session but a co-work branch's — one whose scope names no branch
role — carries the reference its first prompt's instruction calls for, picked by the gate's reference
selection over the chat agent's brain and attached to that first prompt, the same way a job's is
(「assignments and jobs」); a later turn of the same session is asked over it and not over it again. A co-work
branch's prompt is composed by its parent and carries none.

A thrown-away session goes out on a runner line with its instructions handed in whole, for a consumer that
composes its own — the research unit, which asks for the seats and the sessions its searches, browses,
rounds and branches go out in, is one: the start names `line`, the runner line (`search` or `browse`), and
`systemPrompt`, the instructions — the platform's harness opening and then that text, as one system prompt,
with the line's standing MCP configuration and the tools it allows (`docs/launch-lines.md`, 「Search runner」
and 「Browse runner」). `systemPrompt` with no line is refused 400. A turn into such a session carries work
parts as every thrown-away session's turn does, and goes out on that line whole, on the model the scope
names and the effort and tier the turn's mapping names, never in a standing branch checkout. Its folder is
the scope's `cwd`, or the line's own deployed directory where the start names none. Its prompt is the
caller's, whole, and its answer is the run's reply text. `boundMs` on the start bounds each of its turns,
which is cut past it and ends failed. A start that hands no instructions goes out on the worker line.
Beside them the start may name:

- **`provider` and `model`:** the road its turns go out on, any provider agency carries whose program carries
  the line (`docs/launch-lines.md`, 「Providers and what a route may promise」) — Claude Code, a
  member road such as DeepSeek's, or the registry's provider under a registration's name — Claude Code where
  the start names no provider. A provider agency does not carry, one the owner switched off, or one whose
  program does not carry the line is refused 503 `line-not-carried`, the sentence saying which.
- **`demandId` and `within`:** on a registration naming a card service, a granted demand of the caller's
  own that every turn of the session stands on, with no other asked, or the caller's granted demand each
  turn's own is asked within (`docs/own-llm.md`, 「An engine the arbiter stands」); each a demand's id, a
  positive whole number, refused 400 otherwise.
- **`labels`:** an object the session's record carries, each key and its value written under that key of the
  record's `labels`, for the placed tools that read them; a key agency writes itself — `line`, `checkout`,
  `archive`, `role` and `place` — is refused 400.
- **`toolsAllowed`:** the capabilities the record keeps, of those the line and its agent's role give it; a
  record keeps them all where the start names none, and the list never widens them.

On Claude Code each turn of such a session goes out in a pane of its own on a session of its own
(`docs/agents.md`, 「Runs apart」). On a road whose program carries its own turns each turn goes into a place of
the session's own, the program composed as a worker's job on that road is, the line's instructions its system
prompt and the line's servers its tools; the place is marked as the session's, stands under no agent's row,
and is ended with the session — its program released, the session token it carried taken back, its workspace
closed and its folder taken away. A server that starts ends every such place the process before it left, since
a thrown-away session does not outlive the process that started it. Agency asks nothing of such a session
after a turn's answer: any further turn, a round among them, is its caller's.

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
sessions names a room's once its first turn has opened one. An agent's session is readable once its
agent holds one; an agent whose provider's program carries its own turns, dsh's, holds no session id of
its own, its turns going through its resident's inbox, so its session is readable from its start as a
room's is: the provider and model its row pins it to, else those its last answer went out on, its
resident's context and that provider's `foldRatio`.

A conversation moves its provider and model on the change route, each left as it stands where the body
names only the other, and starts its next turn on a fresh session where the road keeps one. A
thrown-away session's provider and model move the same way, onto a provider whose program carries its line;
one that names no line goes out on Claude Code alone. A hand-over is a worker's alone: it closes the worker's held
session on the model and drops the record, handing nothing to the next turn, and the holder the body
names is kept for the session's state; a worker with nothing open keeps the holder alone. An agent's and
a room's session go out on the provider and model their agent's row names, and that row is where those
move. `holder` and `prefixChangeReason` are read back on the session wherever it is listed or read. A
worker's state answers the provider its session stands on: the provider the worker is pinned to, else the
one its held session last answered on, else `claude-code`.

A session's state carries `foldRatio`: the share of its model's window its context may fill before the
session is folded once the turn that crossed it is over. It is the fold-ratio table's value for the
provider the state names and the tier its agent's work key is assigned (「fold-ratios」 below), read
afresh with each read, so a change on the console reaches the next read of every session. A room's and an
agent's session carry `context` beside it,
the context their last turn left — `carried`, its tokens, and `window`, the model's window — or null
where no turn has ended on it since agency's process started or since it last folded or ended; on a
resident program that carries its own turns it is read off the program's record, its last answer against
the latest window the record names (`docs/sessions.md`, 「Session compaction」). Beside it stands
`foldsItself`: true for a room's or an agent's session on such a program, which compacts itself at the
session's `foldRatio` and takes no fold call, and false for every other session. It carries
`tier` beside it: that tier, by its name, or null where the key is assigned none, so a consumer keyed by
tier matches the session by the tier the agency reads it at.

A turn sent is carried to its end whether or not its sender still waits on the send: a sender that lets
go of the request, or goes down while the turn works, leaves the turn running, and the turn ends as it
would have, its run closed and its end published as `turn.ended`. Agency's own restart is no end of a
turn either: a turn whose program keeps its record in its place is picked up by the server that starts
next, and its end is published as `turn.ended` under the same `turnId`, a job's turn ending its job; a
turn no record names ends failed (`docs/sessions.md`, 「A restart hands the session over」). The progress
read of a turn picked up after a restart answers `unknown-turn`.

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
| GET | `/runs` | Runs newest first: open or closed, by agent, room, round, names or the search they carried; with the tokens spent so far. |
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
item and check of the draft process among them, is a run of its own under the round.

The list answers the newest runs first, `limit` of them (50 where it names none, at most 500), and pages
by a run's id: with `before`, the runs before that id; with `after`, the runs after it nearest to it; with
neither, the newest. `more` says whether more runs lie beyond the page in the direction it was asked —
older ones, or with `after` newer ones. It narrows by `live` — `true` the runs still open, `false` the
closed ones —, by `agentName`, by `roomId`, by `round`, and by `names`, a comma-separated list each
matched against a run's unit and its round, every filter named holding at once; `search` answers instead
every run that carried that search, newest first. A query naming anything else, or a value that does not
read as its filter's kind, is refused 400. A change naming `messageId` alone attaches the message, on an open or a closed run alike, and
leaves the run's closing as it stands. A closed run answers `counts.tools`, how many tool calls its log
lines held as it closed; an open run's `counts` is empty.

### log-lines

| Method | Path | Does |
|---|---|---|
| GET | `/log-lines` | Log lines across runs: by run, job or agent, kinds, latest, before or after. |
| POST | `/log-lines` | Add a note in the bot's own words to a run's log, beside the run's own lines. |
| GET | `/log/agents/{agentName}/lines` | One agent's log across its runs: the newest page, a page before or after an id, or the page at a moment. |

`runId` names a run as its lines were written under it, or a run of the ledger by its id, whose lines
are the ones written under its unit. Each line answers the ledger's `runId` where the ledger holds its run, and
that run's `roomId`, `round` and `messageId`, null where it holds none.

A run's tool calls are its `tool` lines, each naming the tool and carrying a `ref`, and the `tool-result`
line with the same `ref` saying how the call ended in its `state`, `succeeded` or `failed`. A run on a
road whose program carries its own turns has them too, read off that program's record: there a call's
line carries no arguments and its result's text says how long the call ran (`docs/run-records.md`,
「Agent log」).

A line's `metadata` is the JSON object the writer kept beside it, null where it kept none. A job's lines
keep the metadata their writer filed, and a pane program's shell call carries Claude Code's
`toolUseResult.bashEditDiff` there, with the files, their hunks, the count of files whose hunks were left
out and the paths the call changed.

- **One agent's log across its runs** is `GET /log/agents/{agentName}/lines`, in the same line shape:
  - no cursor reads the newest page, oldest line first, and `hasMore` says older lines wait;
  - `before` reads the page before that line id, oldest line first, and `hasMore` says older lines wait;
  - `after` reads the page after that line id, oldest line first, and `hasMore` says newer lines wait;
  - `at` opens the page where the line at or shortly after that moment stands, beginning at the head of
    its run (a run's name is not unique for ever); `chat` narrows the moment to one room. Such a page
    answers `hasOlder` for the lines before its head, and `hasMore` for newer lines; a moment already
    left out of the log reads the newest page.
  - The page answers `items`, `hasMore` and `hasOlder`; `limit` is 500 where left out and at most 500.

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
| PUT | `/stop-marks/{jobId}` | Mark a job to stop: its turn's end writes it `stopped` with the mark's reason, and it goes out on no further road. |
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
| PATCH | `/pool/limits` | Set how many children of one co-worker tier may be alive on one provider. |

A seat request answers its `state` — `waiting` in its bucket's line, `reserved` under a reservation,
`active` once let in, and `released` or `cancelled` once given back — and the `provider`, `model`,
`effort` and `tier` its work key resolved to, `effort` null where the mapping names none. A seat is asked
for admission by a `PATCH` naming no `bound` or `finished` state, which answers `{ "request": <seat or
null> }`: the seat, `active`, where it is let in now or was already, and null where its turn has not come,
a reservation lets it in rather than its token, or it was given back; a seat still `waiting` is asked
again. A seat's admissions are its owner asking: the line keeps the order seats were asked for in, and a
place goes to the first waiting seat whose owner is asking now (`docs/coworker-pool.md`, 「Requests and
leases」). A seat ahead whose owner has not asked since the asker's own last admission keeps its place and
is passed over, never dropped, and is next again once its owner asks again; an owner that knows itself
gone takes its seat out by cancelling it. A `PATCH` with `{ "state": "bound", "childId": … }`
binds an active seat to the unit it launched and answers the seat. A seat is given back with `{ "state":
"finished", "outcome": … }`, and cancelled, where it never ran, with `{ "state": "finished", "cancel":
true, "outcome": … }`; either answers `{ "held": <bool>, "stoodDown": <bool>, "note": <text or null> }`,
whether the seat was still held, and whether the asking run's own card was lent for it.

A seat on a registration naming a card service asks its demand once the pool lets it in (`docs/own-llm.md`,
「An engine the arbiter stands」): the admission answers `demand` beside `request` — its `id`, `state`,
`nodeName` once a card is granted and `reason` while it waits, null on any other seat — and answers
`request` null until the card is granted, the caller asking again, with no clock of agency's. The demand is
read as it stands, never held open at the master, so every admission answers at once and the caller's own
pace sets how long it waits for its card. An admission
may name `demandId`, a granted demand of the caller's own the seat stands on: the seat then asks none of its
own, whatever registration it resolved to, and the admission answers that demand, read as it stands, as
`demand`, and `request` null while no card is granted it; a demand the master no longer holds is refused
400 `demand-gone`, and the seat's finish gives nothing back, the demand staying the caller's. An admission
may instead name `within`, the asking run's granted demand the seat's own is asked within. Either is a
demand's id, a positive whole number, refused 400 otherwise. The seat's child stands on the demand the seat
stands on, the caller's own or the one the seat asked, by naming its id as the session's `demandId`. A
demand the seat asked is given back as the seat is finished, or, whatever ended its lease, on the pool's next
minute; where the asking run's card was lent, the finish answers once the run's demand is granted again and
its engine answers health, with `stoodDown` true and `note` the sentence the run is told.

`PATCH /pool/limits` takes `provider`, `tier` and `capacity` and answers 200 with the three as written
(`docs/coworker-pool.md`, 「Concurrency limits」). It is refused 422 `unprocessable-entity`, with one English
sentence, for a provider agency does not carry, a tier the ladder does not hold or one no co-worker is
borrowed at, and a capacity that is no whole number from 0 to the most the pool policy allows. The
written limit lets in at once the waiting places it frees.

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

Each provider answers `name`, `label`, `enabled`, `registered`, `gateway`, `coworker` and its `lines`. `gateway`
says its models are served by agency's model gateway — the registry's registrations and the member roads of
Codex, OpenRouter and DeepSeek — so a gate on it is one request through the gateway; `registered` says the
models are the registry's registrations alone. |

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

A road that cannot be taken says why twice over (`docs/provider-presence.md`): each member of the
fallback's `order`, and each road a work key is `offered`, answers `reason`, the English sentence, and
`reasonCode`, its kebab-case code, both null on a road that can be taken; a tier's cell answers its
provider's as `error` and `errorCode`, the code null where the error is the resolution's own. The codes
are a provider's own, from its policy file, and agency's `line-not-carried`, a provider whose program does
not carry the key's line.

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
model in the path. Each answer carries `name` beside its `key`, the registration's own name, which is what
an agent's fixed model holds on the registry's provider (`docs/model-tiers.md`, 「An agent's own model」). `provider` names the
registry's provider alone, since every other provider's models are its own catalogue and no route changes
them; `tier` narrows the list to the registrations that tier maps to.
A create starts a registration under its own name as its label, at the registry's smallest window, answer
and wait and with no address, and is refused where the name already stands. `tier`, `healthy` and
`lastAnsweredAt` are read back: `healthy` is whether the engine answers now, and `lastAnsweredAt` the
moment it last answered a probe, in seconds, so a screen tells a model that is down from one that has
never answered; the model behind a registration is what the change edits. Each answer carries `cardService`,
the arbiter service the registration's engine is or null, and `roleFiles`, whether that service's instances
keep role files; the create and the change take both, `cardService` a service's name or `none` or null, and a
change names at least one of `model`, `cardService` and `roleFiles`. Naming a card service takes the
registration's addresses away, and its `healthy` is the arbiter master's (`docs/own-llm.md`, 「An engine the
arbiter stands」). A registration's addresses are the registry's: each carries `roleFiles`, whether the instance behind it keeps role files
(`docs/own-llm.md`, 「Role files」), set with `bmo-agency agent --register-address … --role-files on|off`
(`docs/own-llm.md`, 「Where it is set」), left as it stands where the command names none and off on a new
address; no route of this boundary sets an address.

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
| GET | `/work-keys` | Every work key with the tier it is assigned, the tiers it may be, and what it resolves to on each provider. |
| GET | `/work-keys/{workKey}` | Where a work key goes: provider, model, effort, answer tokens, run bound, run mode. |
| GET | `/road-blocks` | Roads blocked now, by model or worker; none means open. |
| POST | `/road-blocks` | Block a road until a time. |
| DELETE | `/road-blocks/{blockId}` | Open a road again. |
| GET | `/launch-room` | Whether a launch may start, why not, and the jobs and browsers running. |
| PATCH | `/launch-room` | Record that launches are held, and why. |

The work keys' list is what a tier screen shows beside the ladder `GET /tiers` answers: each member's `key`,
its `code`, the key in kebab case a screen words it by (`search:basic` is `search-basic`), its `label`, the
English name its profile gives or null for a key no profile declares, its `tier`, `choices`, the tiers it
may be assigned, `conflicts`, each `provider`, `previous` and `accepted` for an owner's choice from before the
tiers kept for review (`docs/model-tiers.md`), and `effective`, for each provider of the registry's archive
its `provider`, `selection`, the mapping the key resolves to there, and where it cannot go there `error`,
one short clause, `detail`, the whole of why, and `errorCode`, the kebab-case code every such error
carries: `provider-off`, `tier-migration-unsettled`, `line-not-carried`, a provider's own presence codes,
or `tier-unresolved` for a mapping its tier does not give. The fallback's work keys carry the same `code`. A key is assigned a tier by `PATCH /tiers/{tierName}`
with the action `assign`.

A provider is answered with its `label`, the English name a log keeps, and its `code`, the kebab-case word
a screen words it by, which is its name: `GET /providers`, the fallback's roads and order, a work key's
offered roads and the tier screens alike.

A work key's read that names no `provider` walks the road resolution's walk for the key
(`docs/cache-fallback.md`, 「One road resolution」): the provider the key's route is set to first, then the
rest of the owner's order from it, each only where it can answer now, and the first that serves the key
answers. The walk passes over a provider whose program does not carry the key's line (a gate's
`gate`), the registry's provider where the registration the key's tier maps to there is not walked, a
provider that cannot answer now — off, its credential missing, its balance spent, no login of it open —
and a model that is not healthy now: its road shut by a failure still standing or, for Claude Code, by the
deployment's rest, or a registration whose engine says it stands aside. So a gate, which reads its road this
way, goes out on the provider its key is set to, and a key set to the registry's provider with a registration
of its own goes out on that registration.

`runMode` is the road the key stands on: the route the owner wrote for it, else its profile's default —
`auto` for the walk or a provider's name. A route row naming a road this build does not carry reads as the
profile's default.

A block is a resource with an id: its selection — `provider::model` — the moment it opens again in epoch
milliseconds and the reason it was closed. A list returns the blocks whose moment is still ahead, narrowed
to the roads a named model or worker would walk, and a deletion by that id opens the road again.

The launch room answers `held`, every reason it is held, the jobs at work and, of those, the ones whose
place runs a browser. The launcher's own notice is kept beside the machine's reasons and read back with
them; a notice that holds nothing is withdrawn.

### gate kinds

| Method | Path | Does |
|---|---|---|
| GET | `/gate-kinds` | The gate kinds the gate declared, each with the part of a turn it stands in and its step there. |
| PUT | `/gate-kinds` | The gate declares its kinds and their order, over the declaration standing. |

- **The gate declares its kinds; agency keeps no copy of the gate order.** A gate kind is a work key whose
  line is the gate's, and its order — which of them stand before a reply, which after it and which outside
  any turn — is the gate's own policy (`docs/agent-roles.md`). The agency reads `GET /gates` from the gate
  as it starts and once a minute after, and keeps what the gate answers; `PUT /gate-kinds` is the route the
  gate declares through when it has more to say than that page carries. The declaration is written over
  the one standing whole: a kind the gate no longer names is no kind of a turn.
- **What a turn reads.** An agent's gate switches (`GET /agents/{agentName}`) and whether its own thinking
  and effort reach its runs are read from the declared kinds: a gate named there owns no reasoning, and an
  agent whose turns answer in no room has no gates to switch. Every conversational agent's turn passes the
  declared kinds; agency keeps no subset of its own.
- **A declaration names its gates,** each as the gate publishes it: `gateName`, `part` (`pre`, `post` or
  `apart`), `step`, `condition` or null, `switchable`, `layaProbe`, `defaultLayaMode` and `answerTokens`.
  An empty declaration, or one whose kind stands in no turn part, is refused 400 `bad-request`. A gate
  the address does not answer leaves the declaration already kept as it stands.

### pull request reviews

| Method | Path | Does |
|---|---|---|
| POST | `/github/deliveries` | One GitHub delivery of a `pull_request` event, its body as GitHub sent it, handed on by the console's gateway with its `X-GitHub-Event` and `X-Hub-Signature-256` headers; answers 202 `{taken}` at once and acts after (`docs/pull-request-reviews.md`). |

- **Signed or refused:** the signature is checked against the deployment's webhook secret
  (`BMO_GITHUB_WEBHOOK_SECRET` or `BMO_GITHUB_WEBHOOK_SECRET_FILE`); one that does not match is 401
  `bad-signature`, and a deployment naming no secret answers 503 `no-webhook-secret`. An event or an action it
  does not act on is answered 202 as well.
- **What it does after:** an open pull request a worker holds is filed once as a review item in the job service,
  in the unattended lane only where its author is the repository's `OWNER` or a `MEMBER` and its head stands in
  the same repository, and otherwise as a draft that waits for the owner's approval and is never merged by the
  review; a push is told on that item; a merge opens the held items labelled with the same pull request, and a close is
  told on them.

### translations

| Method | Path | Does |
|---|---|---|
| POST | `/translation-runs` | Translate one text on the `translate` road: `purpose` (`document`, `worker` or `record`), `language` (the target's name as the instructions ask for it), `text`, an optional `note` and optional `terms`; answers 200 with `text`, the translation whole, and the `provider` and `model` it went out on. |

- **One text, one run** (`docs/launch-lines.md`, 「Translate runner」): no session is kept, nothing is
  queued and nothing is cached here; the caller cuts a long text into parts, keeps the translations it
  wants kept, and bounds how many parts it has out at once.
- **A caller that stops waiting** ends its translation there, as one that failed: the card the
  translations out on a registration share stays with the others still out and is given back once none
  is, whether its demand was granted yet or not (`docs/own-llm.md`, 「An engine the arbiter stands」).
- **A note** is the caller's word on this one part — what an earlier answer for it got wrong, or that a
  one-line title is wanted — carried in the run's system prompt after the translator's own, as an
  instruction the model reads and does not render; the input is the language line and the text, and the
  terms where the caller names any, since a translation model renders every line of its input.
- **Terms** are the words of this one part the translation renders as the caller says: each a `source`, as
  the text writes it, and a `target`, how the translation writes it, the source itself where none is
  named, so a name, a figure with its unit, code, an address or a fixed heading the caller lists stays as
  written. They stand at the head of the input as the translation model's own reference translations, one
  line each, and the instruction after them asks for the translation alone (`docs/launch-lines.md`,
  「Translate runner」). An empty list, or none, asks with the language line alone.
- **Refusals:** 400 `bad-request` for a purpose it does not know or an empty text or language; 503
  `no-login-open` with the road resolution's sentence where nothing can carry it; 413 `too-large` where the
  model refused the part as longer than its window takes, so the caller cuts it smaller; 502 `no-answer`
  where the road answered with nothing or failed otherwise, naming why.

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
for the login answering for it — none read at or before the account's unread mark or its mark of waiting
for a person, and none read before its home was pointed elsewhere (`docs/usage-windows.md`, 「What reads
the windows」): each a window's `name`, `utilization` (0 to 1), `resetsAt` and `seenAt` in
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

A companion's settings are its reference switch, its persona, its generation settings, its model method and
its thinking; its system prompt is its base prompt, every companion's role whole and its persona, and no
section of it is chosen per companion (`docs/companions.md`, 「What a turn is given」). `pieces` and
`ownPieces` are no setting: a change naming them is refused 400 as naming no field, and the settings a
companion is answered with name neither.

A companion is answered with the model method it stands on, `method`: `fixed` on a registration, or `route`
on the supervisor key's road (`docs/model-tiers.md`, 「An agent's own model」), with `provider` and `model`
the ones it answers on now. A change takes `registration` as a registered model's name, or null to clear the
pin and answer on the route.

### workers, prefix-checks and room-holdings

| Method | Path | Does |
|---|---|---|
| GET | `/workers` | Every worker. |
| POST | `/workers` | Add a worker. |
| GET | `/workers/{workerName}` | One worker: checkout, node, role, prefix documents, sampling, session, runs. |
| PATCH | `/workers/{workerName}` | Change a worker: fields, node placement, sampling, stand it again, ready its bench. |
| DELETE | `/workers/{workerName}` | Remove a worker. |
| GET | `/workers/{workerName}/definition` | The worker's agent definition: graph, trailer, handoff. |

A worker's `state` is `resting` while it rests; `working` while a job of its is `starting` or `running` —
`jobId` naming that job and `itemId` its item — or while its program still carries a turn no such job
names, one going on in this process or one kept in flight across a restart, `jobId` and `itemId` then null;
and `idle` otherwise, `jobId` and `itemId` null. A worker reads `idle` only where a launch on it can go in.

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
| GET | `/jobs` | Jobs newest first: by state or running, by item, room, worker or holder. |
| POST | `/jobs` | Run an item on its assigned worker: the harness hands over the item's content and the lease it holds, and the agency launches the worker's session on it; refused only while no provider of the order can answer. |
| GET | `/jobs/{jobId}` | One job and what its turn spent, `usage`. |
| PATCH | `/jobs/{jobId}` | End a job with its outcome, or stop it, its turn cut short first; its assignment is released. |
| GET | `/jobs/{jobId}/prompt` | The prompt the job was launched with. |
| POST | `/instruction-texts` | The work-order step's line composed from an instruction. |
| POST | `/item-filings` | File an item in the job service for the turn an agent is carrying now: its room as the item's `scope`, and `asked-by=owner` where every message of that turn is the owner's. |
| POST | `/job-stops` | Stop a job, by its id or by its item, for the turn an agent is carrying now, only where every message of that turn is the owner's. |

The list answers the newest jobs first, `limit` of them (50 where it names none, at most 500), and pages by a
job's id as the runs' list does: with `before`, the jobs before that id; with `after`, the jobs after it nearest
to it; `more` says whether more lie beyond the page in the direction it was asked. It narrows by `state`, one
state or several comma-separated; by `running`, `true` the jobs starting or running and `false` every other;
by `itemId`; by `roomId`; by `workerName`; and by `holderName`, every filter named holding at once. A query
naming anything else, or a value that does not read as its filter's kind, is refused 400.

A job's `itemId` and `leaseId` are the harness's references, kept as given. The harness reads the job's
progress, questions and outcome from its fields and events, and reports and closes the item in the job
service.

- **A running job keeps its item's lease, whoever launched it.** While a job is `starting` or `running`,
  agency renews its item's lease in the job service on the harness's behalf on its own clock
  (`policy/job-leases.toml`), so the lease holds however long the job runs and whether or not the harness
  that took it still stands — its session replaced, its plugin restarted, agency itself restarted under the
  job. A lease the job service no longer holds, run out before a renewal reached it, is taken again on the
  same item for the same holder and assignee, and the job keeps the new lease's id, which its end gives
  back. An item another holder leased meanwhile, or one no longer open, is left as it stands and said in
  agency's log. The harness may renew the lease as well; a renewal of either holds it. Agency keeps no job store; the calls it makes there are the ones a job's end states below,
and the filing of an item.

- **An item a chat agent files goes through agency,** `POST /item-filings`, so what the item says of where it
  was asked and who asked is agency's reading of the turn rather than anything the model writes. It takes the
  `agentName` its harness plugin was started for — never one the model names — and the item's `title`,
  `brief`, `queueName`, and where they apply `priority`, `assigneeName`, `requesterName` and `labels`, and
  raises it in the job service (`POST /items`) on the chat's own caller token where the deployment names one:
  - its `scope` is the room of the turn that agent is carrying now;
  - its labels gain `asked-by=owner` where the receiver marked that turn privileged — every new message in it
    the owner's — and lose any `asked-by` the call names, so the job service's unattended labels open an item
    the owner asked for at once while one the agent raised itself, or anyone else asked for, stays a draft;
  - an agent carrying no turn now files no item: 409 `no-turn`.
  It answers 201 with the item's `id`, `state`, `scope` and `labels`, as the job service answered them; a
  refusal of the job service's is 502 `upstream-failed` with its sentence.

- **A chat agent stops a job on the owner's word,** `POST /job-stops`. It takes the `agentName` its harness
  plugin was started for, never one the model names; exactly one of `jobId`, the job, and `itemId`, the item
  whose `starting` or `running` job it stops; and `reason`, a short line that becomes the job's report. The
  turn's privilege decides, as it does for a filing, so no one else in a room stops a job through the chat
  agent:
  - an agent carrying no turn now stops nothing: 409 `no-turn`;
  - a turn the receiver did not mark privileged stops nothing: 409 `not-owner`;
  - an item with no job `starting` or `running` is 404 `no-job`, and a job already ended or stopped 409.
  The stop is the one below, and its answer is that stop's.

- **A job is stopped by cutting its turn short.** `PATCH /jobs/{jobId}` with `state` `stopped` and
  `POST /job-stops` stop a job alike:
  - the reason, the `report` the stop names, is kept as the job's stop mark, and the job's run is cancelled:
    the cancellation the turn's door reads in the worker's program, cutting the turn short with its surface's
    interrupt, a turn picked up across a restart included;
  - the answer waits for that turn to end, up to `job_wait_seconds` of `policy/run-stop.toml`. The turn's end
    writes the job `stopped`, outcome `stopped`, with the reason as its report, releases its assignment,
    returns its lease and tells its room, so the worker takes the next launch as soon as the stop answers;
  - a job no turn carries is written so at once;
  - a turn still going once the wait runs out is refused 409 `stop-pending`: the stop stands, and the job ends
    stopped the moment its turn does.
  A job's end never writes over a job already ended or stopped.

- **A job's end reaches the room and gives its lease back.** However a job ends — its run finishing, failing,
  or the harness ending or stopping it — the agency returns the item's lease to the job service as the end is
  written, so the item is open again the moment no run holds it, and queues the job's report for the agent
  that filed the item through the receiver's `POST /instructions`: an order under the sort `job`, naming the
  requester its item names, and carrying the item, the worker, the outcome and the report whole, under the
  event `job-ended:#<item>`. The receiver keeps the instruction until a turn takes it, so a chat session
  being replaced when the job ended loses nothing, and it queues the notice for that requester and no other
  agent. The requester reads the report and relays the job's end in a few lines in that room, as its own
  manual says, the report itself staying whole on the item. A job with no room is queued for the owner's
  private room, which is the room the receiver gives an instruction that names none. The room a job answers is
  `roomId` from the launch, or the item's own `roomId`, or the item's `scope` where the item was filed from a
  room; the item a harness hands over carries that `scope` with it, so a job launched from a room is told
  back in that room.

- **A job whose turn goes on across agency's restart ends as its turn does.** Its turn is kept in flight
  while it goes, and the server that starts next picks it up (「sessions」 above): a turn whose program's
  record holds its end, or comes to hold it, ends the job `ended` with the turn's answer as its report,
  as its launch would have, its run closed, its assignment released, its lease returned and its end told
  to the room. The pass below leaves a job whose turn is kept in flight to it.
- **A job left running with no live execution ends as stopped on the pass.** The process that ran it can
  go away with agency's own restart before the run writes its end, on a program that keeps no record the
  next server can read it from. On start, after the turns in flight are picked up, and on the reconcile
  pass that keeps the execution rows, a job still `starting` or `running` whose turn is not kept in flight
  is read against the records the store already keeps: an open execution row of its worker's on the worker line begun at the job's start is a live
  execution, and an open run in the ledger is a live run. A job with neither — or whose run is already
  closed — ends as `stopped` with a report saying the process that ran it went away, its open run is closed,
  its assignment released, and its lease returned and its report told to the room through the same delivery
  as any other end. A job younger than the launch window is left for the next pass, and one whose worker
  stands on another machine is left, since its execution is no record this machine keeps. A job with a live
  execution is left running, and so is one this process is still carrying — waiting for a road, or between
  one road and the next.
- **A job may carry an instruction,** `instruction`, beside its item or inside it — `repository`,
  `documents`, `sentences` and `checks`, every one required and none empty (`docs/worker-bench.md`, 「A
  small-model instruction」); one that lacks a field is refused 400 naming it. The job keeps it, answers
  it, and its prompt renders it under the item's title. `POST /instruction-texts` answers `text`, the
  one line a work-order step is written as, composed from an instruction.
- **A job's prompt carries its item's newest notes, as many as fit the policy's bytes**
  (`policy/job-prompt.toml`, `[notes]`): the notes that do not fit are left out from the oldest, and one
  line says how many were left out and that they stand on the item. The report trailer every job is held
  to is no part of the prompt; it stands in the worker's cached prefix (`docs/launch-lines.md`).
- **A job's first prompt carries the reference its instruction calls for.** As the job is launched, its
  instruction is handed to the gate's reference selection (`POST /reference/selected` of the gate), and
  the entries the gate picks from the chat agent's brain — knowledge, rules and documents — are attached
  to the prompt under a `## Reference` heading. A job with no instruction, or a gate that picks nothing,
  stands on its own words; a worker reads the brain only through what the gate picked here.
- **A job goes out on the road the road resolution answers** (`docs/cache-fallback.md`, 「One road
  resolution」): its worker's pin, else the job key's setting, as the head of the owner's provider order,
  the first provider of that walk that can answer now carrying it. A job is refused only where nothing in
  the order can answer: then `POST /jobs` answers 503 `no-login-open`, its sentence naming every
  provider's reason — each Claude login's own, with the moment the first of them reopens — and what a
  person must do for the first road a person can reopen, a sign-in command or where a balance is topped
  up; nothing is written and the assignment stays held.
- **A job moves when its road stops answering.** A turn that fails for its provider's availability — its
  quota, window, balance or credential refused, its last login closing under it, or its road found unable
  to answer as the turn ends — is launched again on the resolution's next answer: the same job, item,
  lease and checkout, at most once on each provider in one job, its prompt opened with a line saying it
  continues work already in the checkout. A job answers `roads`, every road its turns went out on in
  order — each `provider`, `model`, `account` (null off a provider that holds logins), `at` in epoch
  milliseconds and `reason`, why the road before it was left, null on the first — and its report ends
  saying where it ran and why it moved. Where nothing in the order can answer, the job stays `running`
  and answers `heldFor`, the reason it waits; it goes out again as soon as a provider can answer, and
  `heldFor` is null whenever a turn is out. A failure of the work itself moves nothing.
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
`road.changed`, `launch-room.changed`, `companion-reset.created`, `place.changed`, `assignment.created`,
`assignment.changed`, `job.created`, `job.changed`, `job.ended`.

A kind agency keeps to its own processes does not cross the stream.

- **`road.changed`** carries one fact of a work key's road (`docs/cache-fallback.md`, 「One road
  resolution」), published once per move, after the system has acted, and named in its `fact`:
  - `road-moved`: `key`, `from` and `to` — each a road, its `provider`, `model` and `account`, the
    account null off a provider that holds logins — and `reasons`, why each provider passed could not
    answer, each a `provider` and its `reason`;
  - `road-returned`: `key`, `from`, the road it had moved to, and `to`, the head of its walk it is back on;
  - `road-closed`: `key`, `reasons`, every provider's reason, and `command`, what a person does to reopen
    the first road a person can reopen — the sign-in command with the account's own configuration
    directory, or where a spent balance is topped up — or null where every road only waits for a reset,
    with `reopensAt`, the epoch second the first of them reopens, or null.

  Telling the owner is the consumer's: the moved road as what the system switched to, the closed road
  with its command passed over verbatim. No fact is the end of the matter, and none is said twice.

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

- `file_item` calls `POST /item-filings`, naming the agent its server was started for (`bmo-agency mcp
  --agent <name>`); it is the chat line's one road for filing an item: the job plugin is started without its own
  item creation on every program, and no prompt names it.
- `stop_job` calls `POST /job-stops` the same way, naming the agent its server was started for, and is
  listed beside `file_item` only where the server was started for one.
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

Agency reaches upstreams through what their repositories publish: brain, graph, node, arbiter,
pocket, testbed, gate and chat through their clients, and cache-warmer through the schedule library
`bmo-cache-warmer-schedule`. These routes below it are called: as a job ends, the job service's lease is
given back and the receiver's `POST /instructions` is handed the report for the room's chat agent; after a
chat program's first turn, that same route is handed one fact for each of the session's servers its
program did not attach (`docs/agent-control.md`, 「dsh's providers: OpenRouter and DeepSeek」); and before a chat agent's turn opens a fresh
session in place of one it had, the receiver's `POST /handoffs/{agentName}` is asked for the old session's
handoff, with `reason` and `room` where the turn names one, and waited on (`docs/sessions.md`, 「A restart
hands the session over」); and every minute the job service's `GET /queues`, `GET /items` with `state`
`open` and its port's `GET /items?open=1`, whose `approved` says which items are approved, are read, and
an idle queue is told once through the receiver's `POST /owner/notices` with the code `queue-idle`
(`docs/agent-control.md`, 「An idle queue is told to the owner」); and for pull request reviews the job service's `GET /items` by the `pr` label,
`POST /items`, `POST /items/{itemId}/notes` and `PATCH /items/{itemId}`, and once an hour GitHub's search through
`gh api graphql`, a search for the owner's login (`user:`) (`docs/pull-request-reviews.md`). Each is the documented route of the unit that holds what
must reach it; agency keeps no client of these units and no store of their rows.
Of the brain it asks an agent's own brain made as the agent is registered, found by the agent's key,
let go whole or only found for a preview, the discipline a chat launch's system prompt carries, and the
base layer of the agent's persona that prompt ends with. Of the tool host it asks, on its control socket
with the issuer token, a session token for each program and run that holds a capability, which it takes
back, a namespace's facts, the paragraph on finding and using tools that a chat launch's
system prompt carries, and, after a chat program's first turn, the servers its session token's clients
reached (`GET /tool-sessions/{token}`, its `connected`), where a host that saw none says nothing. Of the gate it asks the reference a job's or a thrown-away session's instruction
calls for, picked over the chat agent's brain, and attaches the picked entries to the run's first prompt; and it
reads the gate kinds and their order as the gate publishes them, so no copy of the gate order stands here.

- **Where the arbiter's master is.** `AGENCY_ARBITER_URL` names its address and `AGENCY_ARBITER_TOKEN_FILE`
  the arbiter deployment's settings file whose `CARD_MASTER_TOKEN` is its token. A run on a registration
  naming a card service asks it `POST /demands` for one card, reads `GET /demands/{demandId}` with its answer
  held open, and gives the demand back with `DELETE /demands/{demandId}`; health reads `GET /cluster`, and
  the gateway's launcher reads it on its beat for each service's standing instances (`docs/own-llm.md`, 「An
  engine the arbiter stands」). With no address, no demand is asked and a run on such a registration is
  refused.
- **Where the gate is.** `BMO_GATE_URL` names its address and `BMO_GATE_TOKEN_FILE` the path of the file
  its token stands in. With no address, no reference is asked and a job or a thrown-away session carries only
  its own words.
- **Where the brain is.** `AGENCY_BRAIN_URL` names its address and `AGENCY_BRAIN_TOKEN` the token it is
  called with, or `AGENCY_BRAIN_TOKEN_FILE` the file it stands in; the node's token is read the same way
  (`AGENCY_NODE_TOKEN`, `AGENCY_NODE_TOKEN_FILE`). With no address, no agent has a brain: nothing is made, counted or let go.
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
- **The tool discipline.** At a chat launch, the tool host's `GET /discipline` is read through its client
  and its text, telling the session how to find and use a hosted tool with `bmo use`, is put into the
  system prompt whole; agency writes nothing of its own about it. The control socket is the one
  `BMO_MCP_HOST_SOCKET` names, else `bmo/mcp-host.sock` under the user's runtime directory. A host that
  cannot be reached, or refuses,
  leaves the paragraph out and the launch logs one sentence saying so.

Each surface is a crate of its own repository, declared in the workspace manifest and allowed by
`tests/dependencies.rs`; every other crate of another repository, a downstream's client among them,
stays outside the boundary.

It calls no downstream but the job service's lease route and the receiver's instruction route: the lease
returned and the report handed over as a job ends, and a chat session told of a server it lacks. The gate and the receiver reach it through the hook point
above for everything else.

- **A test's ground reaches no peer.** A server a test stands is handed an environment built for it, with
  no deployment's peer address or token inherited from the worker that runs the suite; it names a peer
  only where the test itself names one, and it marks itself a test's. Every call in this section — the
  job service's lease, the receiver's instruction, notice and handoff, the job service's queue reads — is then not made, and the line saying so is logged. A test
  that wants a stand-in calls the call itself with a map of its own, never through a test server's
  ground.

## What the redesign took out

`GET/POST /digests`, `GET/POST /searches`, `GET /searches/{searchId}` and `GET/PATCH /research-settings`,
with the tools `browse`, `remote_browser`, `research_branch`, `unread_pages`, `song` and `news_due`, are
gone: each only wrapped a call into an upstream the caller reaches directly. A digest, a research, a
browse and the morning news are ordinary sessions started with `POST /sessions` and the instructions
their caller hands in, and the session calls the upstreams it needs through their own MCP servers.

The agency's job client and its lease taking are gone; the harness takes the lease and hands the item's
content to `POST /jobs`. The hooks the job service and the brain registered here are gone; agency calls
the brain's client itself. The word-chain procedure and its mention among the agent documents are
retired.
