# The boundary API

This document is agency's boundary: every route it exposes, every event it publishes, every hook a
consumer registers, and every table whose shape a consumer reads or writes. A consumer reaches agency
only across this boundary and never into a crate or a module behind it; agency reaches a consumer only
by publishing an event or calling a hook the consumer registered. What a turn is told, whom an agent
answers, which role and model a piece of work needs, and whether an answer may go out are the
consumer's, decided above this boundary and handed in; agency supplies, carries and controls, and
judges none of it.

## The shapes the boundary is made of

- **A route** is an operation agency's server answers: a verb and a path, a request body and a
  response body, and the status codes and kebab-case outcome codes it answers with. A route's contract
  is its shape, not its wording.
- **An event** is something agency publishes as it happens, which a consumer subscribes to. Agency
  does not wait on a subscriber and reads nothing back from one.
- **A hook** is a decision agency hands up: a consumer registers one, and agency calls it at the point
  named. Every hook has a default that does nothing — agency runs with no consumer registered, and an
  unregistered hook is that default, never a failure.
- **A table** whose contract is shared is one a consumer reads or writes directly beside agency. Its
  columns, their meaning and the rows' keys are the contract; a column a consumer does not read is
  agency's own.
- **A tier, a role and a work key** are the words a consumer names a piece of work with; agency
  resolves each to a provider, a model and an effort (「Models, registry, tiers, roads and fallback」
  below).

## Versioning

- **The contract carries the repository's one version, in semver.** The version in the workspace
  manifest is this boundary's version; the current line is `0.25.0`.
- **A change to this document and a change to the boundary go in one commit.** A change that makes a
  route, an event, a hook or a shared table's contract different repairs this document in the same
  commit, stating what is true now.
- **The version moves by what changed:** the major on a change to a route, an event, a hook or a
  table's contract that an existing consumer reads; the minor on an addition of one; the patch on
  anything else.

## The control port

- **Every route is answered by agency's server,** the deploy tree's `bin/agency-server`, on the loopback,
  on the port `AGENCY_CONTROL_PORT` names (3220 where it names none), under `Authorization: Bearer
  <token>` with the token `AGENCY_CONTROL_TOKEN` holds; the server does not stand without one. A request
  without the token is answered 401.
- **Both variables stand in the data root's `env/agency.env`,** the file the server's unit takes its
  environment from. A process of agency's whose environment holds neither — a placed tool, which the
  tool host hands a clean one (`docs/tools.md`) — reads them there, a variable its own environment holds
  standing first.
- **A route takes and answers JSON,** field names in camelCase, but for a run of the run ledger, whose
  fields are its columns' names (「Run log and run records」 below); a GET reads its fields from the
  query, and a path's `{id}` names the row it reads. A refusal is answered with its status and a body of `code`, the status's kebab-case reason, and `error`,
  an English sentence; a session operation's fault carries its own outcome code (「Sessions」 below).
- **A verb outside the server tells it what it changed** by a POST to the route concerned, under the same
  port and token; the verb is never failed by it, and a word that did not land is said once on its error
  stream.
- **The routes:**

| Method | Path | What it answers | Section |
| --- | --- | --- | --- |
| GET | `/agents` | every agent, its place and its configured and answered roads | 「Agents」 |
| GET | `/rulebooks` | the shared rulebooks named, by their text | 「Agents」 |
| POST | `/agent-gate` | one of an agent's gates switched on or off | 「Agents」 |
| POST | `/agent-reasoning` | an agent's own thinking and effort set | 「Agents」 |
| POST | `/agent-role` | the role an agent holds from now on | 「Agents」 |
| GET, POST | `/agent-sampling` | a worker's own sampling, read and saved | 「Agents」 |
| GET, POST | `/base-prompts` | the base prompts and their choosers, read and changed | 「Agents」 |
| GET, POST | `/companions` | the companions' settings, read and saved | 「Agents」 |
| GET, POST | `/companions/reset` | a companion's reset, counted and carried out | 「Agents」 |
| POST | `/companions/add`, `/companions/remove` | a companion's agent row stood up and taken out | 「Agents」 |
| GET | `/reset-point` | an agent's reset point | 「Agents」 |
| POST | `/session/start` | a session started for an agent at a scope | 「Sessions」 |
| POST | `/session/turn` | one turn carried into a session | 「Sessions」 |
| POST | `/session/push` | words put into the turn a session is working | 「Sessions」 |
| GET | `/session/state` | a session's state, by `session` | 「Sessions」 |
| POST | `/session/interrupt`, `/session/fold`, `/session/end` | the three operations on one session | 「Sessions」 |
| GET | `/session/list` | every session the backend keeps | 「Sessions」 |
| POST | `/session/answer` | the caller's answer to the question a session's turn stands on | 「Sessions」 |
| GET | `/session/output` | what a session's latest turn has written so far | 「Sessions」 |
| GET | `/sessions` | every session an agent holds, as a reset reads them | 「Sessions」 |
| POST | `/session-reset` | an ask to clear one session | 「Sessions」 |
| POST | `/rooms/forget` | a room's rows forgotten, or counted | 「Sessions」 |
| POST | `/documents/reload` | documents a session relies on, moved | 「Sessions」 |
| GET | `/events` | the stream of agency's events | 「Events」 |
| POST | `/events/publish` | an event a verb's own process published, published on the server's streams | 「Events」 |
| GET, POST | `/hooks` | the points a consumer registers at, and a registration | 「Hooks」 |
| POST | `/hooks/remove` | a registration taken away | 「Hooks」 |
| POST | `/typed-line` | a typed line's composition, as the submit hook asks it | 「Hooks」 |
| GET | `/road-verbs` | the verbs a room's road performs, as the consumer answers them | 「Hooks」 |
| GET | `/coworker-pool` | what holds each place of the pool now | 「Co-worker pool」 |
| POST | `/pool/enqueue`, `/pool/acquire`, `/pool/bind`, `/pool/finish` | a seat or a place asked, let in, bound and given back | 「Co-worker pool」 |
| POST | `/pool/reserve`, `/pool/reservations/release` | an owner's seats reserved and given back | 「Co-worker pool」 |
| GET | `/pool/reservations` | the reservations an owner holds | 「Co-worker pool」 |
| POST | `/places/take`, `/places/prepare`, `/places/remember`, `/places/retire` | a branch's standing checkout | 「Co-worker pool」 |
| GET | `/run-log`, `/run-records` | a run's lines and its records | 「Run log and run records」 |
| POST, GET | `/runs`, `/runs/{id}` | a run of the ledger opened, listed by the search it carried, and read by its id | 「Run log and run records」 |
| POST | `/runs/{id}/close`, `/runs/{id}/launch` | a run of the ledger closed, and its runner started in its agent's host | 「Run log and run records」 |
| GET, POST | `/models`, `/registry`, `/model-tiers`, `/fallback` | the catalogue, the registry, the tiers and the roads | 「Models, registry, tiers, roads and fallback」 |
| GET | `/stand-in` | whether a stand-in answers an agent while the rest closes Claude Code's road | 「Models, registry, tiers, roads and fallback」 |
| GET | `/model-tiers/models`, `/fallback/codex-models`, `/fallback/openrouter-models` | a provider's catalogue | 「Models, registry, tiers, roads and fallback」 |
| POST | `/worker`, `/worker/node`, `/workers/stand` | a worker added, changed, placed or stood again | 「Workers」 |
| GET, POST | `/jobs/stop-mark`, `/jobs/stop-mark/clear` | a job's stop mark | 「Workers」 |
| POST, GET | `/roads/block`, `/roads/open` | a road's block, and whether a road is open | 「Workers」 |
| POST | `/runs/stop`, `/runs/alive` | runs stopped, and which names are alive | 「Workers」 |
| GET | `/runs/clock` | the clock a run stands on, by its name or the name it runs under | 「Workers」 |
| GET, POST | `/runs/overtime` | the asks on runs past their clock, and a verdict on one | 「Workers」 |
| GET | `/editors/cap` | the bound an editor work key's run carries for an agent | 「Workers」 |
| GET | `/machine/room` | this machine's room for another job's launch | 「Workers」 |
| POST | `/machine/launch-held` | the launcher's notice that launches are held, or its withdrawal | 「Workers」 |
| GET, POST | `/rest` | the deployment's rest, read and written | 「Accounts」 |
| GET | `/rest/passed` | why the rest that has just passed silenced the deployment, the passed rests cleared | 「Accounts」 |
| GET | `/health` | agency's health and readiness | 「Health」 |

## Agents

- **An agent is a place agency stands and controls.** A consumer names an agent by its stable name;
  agency holds where it stands — the herdr it is in and the node, if any — its one standing pane, and
  its lifecycle (`docs/agent-control.md`, `docs/agents.md`).
- **The routes** list the agents with their places and panes (`GET /agents`), and change what an agent's
  row holds: its gates, its reasoning, its role, a worker's sampling, its base prompt, a companion's
  settings, and a worker added, changed, placed or stood again (「Workers」). An agent's place is
  agency's own to stand and retire: the server stands every agent's place as it starts and again as a
  change asks for it, and takes it down as the agent goes (`docs/agent-control.md`, 「One standing pane per
  agent」), so no route stands or retires one. A place's home is read and used together with the agent,
  since a remote agent's pane and ids are its node's (`docs/work-nodes.md`, 「The bot's side of a remote
  agent」).
- **The agent list.** `GET /agents` answers `agents`, every agent of the store, the resting among them,
  each with its row's fields and roads as the screen shows them, and beside them what a recollector
  reads: `awake`, false while the agent rests; `brainFolder`, the folder of the brain its long-term memory
  is in, relative to the brain root, empty for the root itself, or null for an agent that keeps none;
  and `recollectorRegistration`, the registration its recollectors run on, or null for their work key's
  own road (`docs/agent-ownership.md`, 「Long-term memory」). It answers `roles` beside them, every role
  an agent may hold.
- **The shared rulebooks.** A consumer that composes a run's whole prompt itself reads the rulebooks
  more than one writer shares from agency, which keeps one copy of each (`docs/agent-documents.md`):
  `GET /rulebooks?names=<name>,<name>` answers `rulebooks`, each named rulebook's `name` and its `text`,
  trimmed, in the order asked, as the deploy tree holds it. A name that is no rulebook of agency's is
  refused 404 with `not-found`, and a name that is not one plain word 400.
- **An agent's role** is the tools and the policy its runs are shown, named by the consumer and
  resolved by agency (`docs/agent-roles.md`). The role a co-worker place holds is agency's policy's
  (`docs/coworker-pool.md`, 「Standing branch checkouts and their list」).
- **An agent's documents** are the files agency composes a run's launch from, read from the deploy
  tree and the prompt tree (`docs/agent-documents.md`, `docs/base-prompts.md`, `docs/launch-lines.md`);
  a consumer hands in the documents it owns and agency hands them to the harness as they stand. An
  agent may be registered with a prompt folder, one plain name under `prompts/agents/`, and a
  companion's system prompt and persona are read from the folder its row names, never from its name
  (`docs/store.md`, 「Its tables」).
- **A companion's reset** is a route, a dry run and a write, for the companion, the ranges asked, and
  the rooms it answers alone and those it shares, which the caller names since a room's answerers are
  its owner's. Agency takes its own rows, asks the brain for the person range, and publishes the rest
  for its owners (`docs/companions.md`, 「Starting it afresh」).
- **A companion's agent half.** The consumer holding the console's rooms adds a companion with a room
  of its own and takes it out again; it writes the room's half and agency the agent's
  (`docs/companions.md`, 「Where a companion and its rooms come from」).
  - `POST /companions/add` takes `name`, `label`, `provider`, `model` and `worker`, writes the agent's
    row and answers `ok`. Every field is checked before anything is written, and a field that does not
    pass is refused 400 with `field`, the field's name, and `error`, the sentence saying why. A field
    beyond these is not read.
  - `POST /companions/remove` takes `agent`, `rooms`, the chat ids of the rooms it answers, and `by`,
    who asked, and answers `ok` once everything of the companion's own is gone. An agent that is no
    companion, or one the deployment seeds, is refused 400 with `field` and `error`.
- **An agent's reset point.** `GET /reset-point?agent=<name>` answers `point`, the moment the agent's
  last reset cut it off at, in epoch milliseconds, or null where it was never reset; a turn begun at or
  before it is overtaken (`docs/companions.md`, 「What the reset cuts off」). A call naming no agent is
  refused 400.
- **Usage is filed under the agent that spent it,** never added to another's; a run names the agent it
  belongs to and the run it came from (`docs/agent-ownership.md`). A consumer reads an agent's usage
  across the usage boundary described there.
- **No event is published for an agent's place;** a consumer showing the places reads `GET /agents`
  again, and `nodes-moved` tells it when the nodes under them move (「Events」 below).

## Sessions

- **The session contract is ten operations,** declared by the session crate and reached by every
  caller through it: start a session, send a turn into it, push words into the turn it is working, read
  its state, answer the question its turn stands on, read what its latest turn has written so far,
  interrupt it, fold it, end it, and list sessions. herdr is one backend of that contract; a caller
  names no backend (`docs/sessions.md`, `docs/agent-control.md`).
- **What each operation takes and answers:**
  - *start* takes the agent's stable name and the scope, and answers the session — its id, its agent
    and its scope — the one already kept for that scope where there is one;
  - *send a turn* takes the session's id, the prompt and the turn's context (below), and answers once the
    turn has ended: whether the program took the words or refused them while it waits on a person,
    which is not a failure, the run the turn went out as, and how it ended (「A turn's end」 below);
  - *push* takes the session's id and the words, and answers what became of them: `delivered`, the
    working turn took them; `waiting`, its program waits on a person; or `refused`, they did not go in.
    A backend that cannot put words into a working turn answers `refused`;
  - *read state* takes the session's id and answers its state (below);
  - *answer* takes the session's id, the id of the question its turn stands on and the caller's answer,
    and answers nothing but its success. A backend whose turns put no question to their caller refuses
    it;
  - *read output* takes the session's id and answers what its turn now running, or its last one, has
    written so far. A backend that keeps no record of a turn refuses it;
  - *interrupt*, *fold* and *end* take the session's id and answer nothing but their success;
  - *list* answers every session the backend keeps, each as *start* answers one.
  - An operation not carried out answers a fault with its outcome code — `unknown-session`,
    `session-busy`, `backend-unreachable` or `backend-refused` — and an English sentence saying why.
- **A turn's context** is what the caller composed around the prompt and hands in with it, every part of
  it optional: `room`, the room the turn answers — its `chatId`, its block's `text` and the `mark` its
  answer carries — which a session across rooms needs and a room's session reads as its own; `via`, the
  road the turn answers its room on where that is not the room's own; `basePersona`, the persona that
  follows the chat line's system prompt where the caller keeps one; `privileged`, whether every new
  message the turn answers is the owner's or no person's message opened it; `answers`, the ids of the
  messages it answers; `began`, the moment it took its words, in epoch milliseconds, which a reset
  begun after it overtakes and which is the moment the turn is sent where none is named; and on a
  worker's turn `job`, the job it is a turn of, and `item`, the item that job works, each a number, which
  agency keeps beside the worker while the turn goes so a place's pressure on the machine names its job
  (「Events」, the machine's pressure). Agency carries each part to the program as its line declares and
  judges none of it: the prompt and the context are the caller's turn assembly (「Hooks」 below).
- **An editor run's parts** are the context an editor session's turn carries, each optional:
  `run`, the name the caller gives the run; `parent`, the unit it runs under, or null; `agent`, whose
  long-term memory it writes; `chatId`, the room it reads; `step` and `steps`, which batch it is of how
  many, or null; `workdir`, the agent's brain folder relative to the brain root, empty for the root;
  `desk`, the fragment desk its batch opened; and `registration`, the registration it runs on, or null
  for its work key's own road. Agency carries them and judges none of them (「Sessions」, the editor's
  session, below).
- **A work turn's parts** are the context a worker's or a thrown-away session's turn carries, by the
  names a job's runner sends them, each in snake case: `provider`, the harness the turn goes out on;
  `model`, the model the worker's session record is kept under; `mapping`, the line the turn is laid on,
  holding `model`, and `effort`, `tier` and `answer_tokens` where it names them; `resume`, which
  session the turn goes on — `{ "kind": "auto" }`, the one kept for that model and login,
  `{ "kind": "fresh" }`, a new one, or `{ "kind": "session", "id": <harness session> }`, the one named;
  `cwd`, the folder its program works in; `name`, the run's name; `title`, what a person watching the
  pane reads over the turn; `thread`, the thread its session is kept under, or null; `job` and `item`, as
  above, none for a research run's turn; `chat`, the room its run's lines stand under; `unit`, the unit
  it runs under, or null; `user`, the user whose logins carry it, or null; `work_key`, the work key its
  logins are offered by; `hears_words`, whether words pushed into the session reach the turn; and
  `variables`, the variables its program is started with beside its own, empty where there are none.
  The program carries `BMO_RUN_NAME`, the turn's `name`; `BMO_TURN_AGENT`, the agent the session is;
  and `BMO_TASK_LOCKED=1` where the turn has a `job`, so the verbs it runs know whose job they work for.
  Its execution is counted under the turn's `name` and is a child of its `unit` where it names one, so a
  stop or an alive asked of either name reaches it. The Claude Code line names no answer ceiling, so
  `answer_tokens` is carried and read by nothing there. A turn that names no `name` goes out as
  `turn:<agent>:<at>`. Agency judges none of the parts: which model, session, login and folder a job's
  turn needs is its runner's decision.
- **The routes and the herdr backend.** herdr's backend knows a session by its agent and its scope:
  `<agent>@worker` for a worker's, `<agent>@<room>` for a companion's in one room, `<agent>` for a
  session across rooms, which only an agent whose row keeps its session at the `agent` scope holds,
  `<key>@editor` for an editor work key's runs, and `<agent>@apart:<n>` for a session thrown away.
  - `POST /session/start` takes `agent` and `scope` — `{ "kind": "worker" }`, `{ "kind": "room", "room":
    <id> }`, `{ "kind": "agent" }`, `{ "kind": "editor" }` or `{ "kind": "apart", "provider": <name>,
    "model": <name>, "cwd": <folder>, "todo": <item or null>, "role": <role or null> }` — and answers
    `id`, `agent` and `scope`. An editor's session is named by its editor work key in place of an agent
    (below); a thrown-away session is a new one at every start.
  - `POST /session/turn` takes `session`, `prompt` and the context's parts by the names above, carries
    the turn until it ends, and answers its end (「A turn's end」 below). A worker's turn goes on its
    session as its work parts name it: on the mapping's model, effort and tier; on the session its
    `resume` names — under `auto` the one kept for the turn's `model` and the login carrying it, where the
    session's lifetime still holds it, under `fresh` none, under `session` the one named; under the
    logins its `work_key` is offered for its `user`, each tried in turn where one fails on the login
    rather than on the work, a work node's token checked before a login is tried there; in its `cwd`,
    under its `thread`, behind the agent lock its worker's jobs share; with a reset the owner asked of
    the worker's sessions carried out, the standby claimed and the held session judged before it goes
    (`docs/sessions.md`, 「A worker's session per model」, `docs/cache-fallback.md`). A session the
    judging replaced goes out fresh on the mapping's model; a kept session that last answered on a lesser
    model goes out on that one. Where the session the turn settles on is not the one its `resume` named
    under `session`, nothing is put in and the turn ends `reprepared` with `continues`, the session it
    would continue, or null for a fresh one, for the caller to send its prompt again composed for it;
    under `auto`, a kept session the program no longer holds is forgotten and the turn asked again once on
    a fresh one. An answered turn's session is recorded for its model and login, judged against the
    model it answered on and folded once it is full, as the worker's own record keeps it. A worker turn
    that names no work parts goes on the model the worker answered on last, under a login that answers
    now. A turn on a provider other than Claude Code is refused `backend-refused`. A companion's goes on its
    room's session, on its pinned provider and model, with the system prompt its settings compose, a lost
    harness session asked again once on a fresh one, and a handoff begun where the turn's context reached
    the mark (`docs/companions.md`). A turn across rooms goes on the chat line: on Claude Code, on the
    model and effort its tier maps to, with the chat line's system prompt and the base persona the turn
    names, its room named in the turn's variables, on the one session its agent's row keeps across rooms
    (`docs/agents.md`, 「The session's scope」); a turn across rooms that names no room is refused. Where
    the agent's key walks onto a stand-in before Claude Code, or every login of Claude Code is closed and
    a stand-in answers, the turn goes out on that stand-in instead, and the session the row keeps is left
    for Claude Code's next turn (`docs/cache-fallback.md`, 「While a fallback stands, the room is not
    silent」).
  - An editor's session, `<key>@editor`, carries one editor run a turn, and several turns at once: the
    turn's prompt put into a run apart of the editor line under that editor work key, on a session of its
    own that nothing resumes, in the brain folder (`AGENCY_BRAIN_ROOT`, else `brain` under the data root)
    or the folder under it the turn's `workdir` names — on the registration the turn names, else on the
    first stand-in the key's walk meets that answers, else on Claude Code with the tier and effort its key
    maps to, on the login the walk of its logins offers the key (`docs/launch-lines.md`, 「Editor runs」).
    The run goes out under the turn's `run`, which its `turn-started` and `turn-ended` carry: its
    execution is counted under that name and is a child of the turn's `parent` where it names one, so a
    stop or an alive asked of either name reaches it; it is handed the turn's `desk` and `chatId` in its record, and it carries the editor
    cap as its clock (「Workers」, the run's clock). A turn whose `run` is already going on the session is
    refused `session-busy`. Its answer is the turn's `turn-ended`. A key that is no editor's is refused, and
    so is a `workdir` that leaves the brain folder; a turn fails where the brain folder is no repository
    yet, where its `workdir` is not there, where the registration it names did not answer, where a walk
    that had to answer met no stand-in that answered, or where the key maps to no model on Claude Code
    when the run falls to it. An editor's session keeps nothing: its state names the model its key maps
    to on Claude Code, an interrupt cuts every run it carries, a fold is refused, an end lets nothing go,
    and a list names none.
  - A thrown-away session, `<agent>@apart:<n>`, is a co-work branch's or a research run's: every turn on
    it goes on a fresh harness session on Claude Code, on the provider, model and folder its scope names
    and the work parts its turn carries, and nothing of it is resumed, recorded or judged as a worker's
    session is. Where its `cwd` is a standing branch checkout (「Co-worker pool」 below), the turn goes
    into that place's pane, under a home labelled by the scope's `todo` and `role`; elsewhere into a pane
    of its own, opened for the turn and closed with it. It carries one turn at a time, a second refused
    `session-busy`. Its state names its provider, its model, its folder as `place` and generation 1;
    an interrupt cuts its turn, a fold is refused, an end lets it go, and a list names every one not
    ended. A scope naming a provider other than Claude Code is refused `backend-refused`.
  - `POST /session/push` takes `session` and `words` and answers `said`: `delivered`, `waiting` or
    `refused`. The words go into the turn the session is working at its next look at the working
    program, as the door types any word (`docs/agent-control.md`, 「Words that arrive while a turn is
    working」), and only where the turn's provider declares that it takes words mid-turn. They are
    `refused` where the session carries no turn, where its turn's `hears_words` is false, where its
    turn's program takes no words mid-turn or is not working yet, where the turn ends before it takes
    them, and where no look takes them within the
    resident turn policy's `push_take_ms`. A refused push changed nothing, so its words are the caller's
    to carry into the next turn. Words a turn took are carried behind its prompt by any later run of the
    same turn, a lost session asked again once on a fresh one.
  - `GET /session/state?session=<id>` answers `session`, `provider`, `model`, `place`, `generation` and
    `context`, and while a turn goes `run`, the run it goes out as, `went_out` once its line is settled,
    and `question` while one stands (below), each null otherwise. A worker's session read with
    `&model=<model>` also answers `continues`: the harness session a turn on that model under `auto`
    would continue now, as the worker's record and the first login its walk offers hold it, or null for
    a fresh one.
  - `POST /session/answer` takes `session`, `question`, the id of the question its turn stands on, and
    `answer` — `{ "kind": "allow" }`, `{ "kind": "leave" }` or `{ "kind": "refuse", "words": <words or
    null> }` — and answers `ok`. A question id that is not the one the session's turn stands on now is
    refused `unknown-session`, and so is a session whose turns put no question to their caller.
  - `GET /session/output?session=<id>` answers `run`, the run of the session's turn now going or of its
    last one, `running`, whether it is still going, and `text`, what its program has written of its
    answer so far, read from the program's own record as it stands, so a turn stopped from outside still
    leaves what it wrote. A worker's or a thrown-away session that has carried no turn is nothing found;
    a session of any other scope is refused `backend-refused`.
  - `POST /session/interrupt`, `POST /session/fold` and `POST /session/end` take `session` and answer
    `ok`. An interrupt with no turn going cuts nothing; a fold or an end refuses a session in a turn. A
    fold asked of a session across rooms is refused, since its program carries it past its window inside
    its own turns; its end lets the program go and the session its row kept with it.
  - `GET /session/list` answers `sessions`, each as a start answers one.
  - A fault is answered with its outcome code and `error`: `unknown-session` under 404,
    `session-busy` under 409, `backend-unreachable` under 503 and `backend-refused` under 422.
- **A turn's end** is what a turn's send answers and its `turn-ended` carries (「Events」 below):
  `delivery`, `taken` or `blocked`; `run`, the run the turn went out as; `outcome`, `answered`, `blocked`,
  `stopped`, `failed` or `reprepared`; `answer`, the reply's text where it answered, and `reason`, the
  failure's words where it failed; `engineError`, as 「Events」 names it; `went_out`, what it went out on
  once its line was settled — `provider`, `model`, `effort` and `tier`, each null where none was named;
  `reply`, where it answered, in camel case as the program's record reported it and carried on unjudged
  — `mode`, `model`, `effort`, `text`, `sessionId` (the harness session it answered on), `account`,
  `contextRatio`, `contextTokens`, `sessionUsage`, `spend` (each `mode`, `model`, `tokens`,
  `inputTokens`, `outputTokens`, `cacheReadTokens`, `cacheWriteTokens` and `cost`), `liveTasks` (each
  `id` and `description`), `workflows` (each `call`, `name` and `status`), `upstream`, the upstream a
  routed model's request went to, and `requestUsages` (each `inputTokens`, `outputTokens`,
  `cacheReadTokens` and `cacheWriteTokens`); `failure`, where it failed or stood blocked, an object
  holding `kind` — `limit` (`resets_at`, `account`, `silenced`), `account` (`failure`: `limit`, `login`,
  `outage` or `node_token`; `detail`), `died` (`ending`, `{ "kind": "exit", "code": <code> }`; `said`;
  `message`), `person`, `untrusted` (`folder`, `config`), `stopped`, `transport` (`message`), `refused`
  (`message`), `unavailable` (`scope`, `mode`, `reason`) or `error` (`name`, `message`); `spans`, where
  the turn's time and spending went, each `kind`, `name`, `seconds`, `times`, `tokens`, `input_tokens`,
  `output_tokens`, `cache_read_tokens` and `cache_write_tokens`; `compacted`, whether the session's
  context was compacted during the turn; and `continues` where it ended `reprepared`. A part a scope's
  turn does not carry is null or empty: `reply`, `failure`, `went_out`, `spans` and `compacted` are a
  worker's and a thrown-away session's turn's, and a companion's, an agent's and an editor's turn answer
  the outcome, `answer`, `reason` and `engineError` alone. A turn refused once its `turn-started` was
  published still ends, `failed` with the refusal as its `reason`.
- **A question in the middle of a turn** is put to the turn's caller on a worker's and a thrown-away
  session's turn, and left standing for a person on every other scope's (`docs/agent-control.md`, 「A
  confirmation in the middle of a turn」). The question is an object of `id` and `kind`: `permission`,
  with `text`, the command or request as the screen shows it, and `amendable`, whether the program
  takes words with a refusal; or `unknown`, a question this reading does not know, with `screen`, the
  last rows the screen shows. It stands on the session's state and is published as `turn-question`
  while the turn waits for it to be answered through `POST /session/answer`: `allow` lets it go on,
  `refuse` refuses it with the words where the program takes them, and `leave` leaves it standing for a
  person in the pane. A question answered in
  the pane or gone from the screen is withdrawn, and a later answer to its id is refused. Agency judges
  no question: which command a job may run of itself, which question goes to the owner and what a
  question nobody answers comes to are the caller's.
- **A turn is sent with its prompt and its context;** the session keeps the conversation, and agency
  computes only what is new (`docs/sessions.md`, `docs/agents.md`, 「The session's scope」). What the
  prompt and its context carry is the consumer's, composed above this boundary.
- **A session's scope** — one session, a room's session, a worker's session — is the consumer's
  declaration; agency keeps one harness session per scope and resumes, folds or hands it over as the
  scope's provider declares (`docs/agents.md`, 「What a provider declares for a conversation」).
- **Reading state** answers the session's id, the provider and model it stands on, its place, its
  generation, and the context its last request carried against the registration's window
  (`docs/own-llm.md`, 「How requests go」). A worker's generation is how many times its session for the
  model it stands on was opened fresh rather than resumed, and its context the tokens its last answered
  turn carried against the window they filled, kept on the session's record. A consumer
  reads it to show a session; it builds no run from it.
- **Folding and handing over** are the provider's way of carrying a full session past the window; a
  consumer that keeps a conversation reads the handoff the provider declares rather than agency's fold
  (`docs/companions.md`, 「Its limit」).
- **A reset of a session** is an ask a consumer writes and the next turn carries out; agency takes the
  program down at once and opens a new generation (`docs/session-resets.md`, `docs/companions.md`,
  「Clearing it」).
- **Forgetting a room.** The consumer that owns a room, removing its registration, asks agency to forget
  it by its chat id: every row of agency's own tables naming the room — its column `chat_id`, a column
  opening `chat_` or one holding `_chat` — goes, but those the code owning a table declares stay with
  their reason: a job's run-log lines, which belong to the job (`docs/run-records.md`, 「Agent log」).
  A dry run answers what would go, table by table, and what stays and why; the forgetting answers what
  went. Nothing of another service's tables is read (`docs/store.md`, 「Its tables」). `POST /rooms/forget`
  takes `chatId` and `dryRun`, and answers `gone`, the rows of each table, and `stayed`, each `table`,
  `rows` and `why`.
- **Documents moved.** `POST /documents/reload` takes `paths` and `now`, whether the notice is due at once,
  and where the caller knows them `agent`, whose documents they are, and `rooms`, the chat ids of the rooms
  they concern, and publishes them as `documents-moved` for the consumer that holds the rooms; agency
  tells no session itself (`docs/sessions.md`, 「Document reload」). It answers `paths`, `now`, `agent`
  and `rooms` as published. The brain calls it for its own documents, its paths relative to the brain
  root, and `bmo-agency reload` for the files it is given.
- **The events** are the turn's lifecycle and the road it walked (「Events」 below).

## Events

- **Read across the boundary,** `GET /events?kinds=<name>,<name>` answers a stream of server-sent events
  of the kinds named, every kind where none is, from the moment it is opened: each event named by its
  kind and its data the event's JSON. An event published while no stream reads its kind is written out
  for nobody.
- **Agency publishes a turn's lifecycle** as the session contract carries it: `turn-started`, the
  `session`, the `run` it goes out as and `at` in epoch milliseconds; and `turn-ended`, the `session`, the
  `run`, its `outcome` — `answered`, `blocked`, `stopped`, `failed`, or `reprepared` where a worker's turn
  was put back to its caller before anything went in — the `answer` where it answered and
  the `reason` where it failed, and `engineError`, `{ "kind": <kind or null> }`, where it failed because
  its engine ended it with an error, null otherwise: the kind as the program names it — `rate_limit`,
  `overloaded`, `authentication_failed`, `billing_error`, `invalid_request`, `max_output_tokens` and the
  rest of its list — and null where it named none. A companion's turn in a room and the chat agent's turn
  carry it alike (`docs/companions.md`, 「What a turn shows」, 「A turn that goes nowhere」). `turn-ended`
  carries the whole of the turn's end, as its send answers it (「Sessions」, 「A turn's end」), and is
  published before the send is answered. `turn-question`, the `session`, the `run` and the `question`,
  is published when a worker's or a thrown-away session's turn stops on a question put to its caller
  (「Sessions」, 「A question in the middle of a turn」).
  A consumer reads them to show a turn and to wake what a turn's end should wake; agency waits on no
  reader.
- **An event no consumer subscribed to** is published to nobody, and nothing else of the turn changes.
- **A run's log lines are read, not published:** a consumer showing a long turn as it goes reads
  `GET /run-log` from the last id it read onward (「Run log and run records」 below).
- **A node's state is the node service's** (「Calls agency makes」 below). `nodes-moved` is published,
  carrying nothing, when the node list's shape moves under the places — a node added, removed, renamed,
  relabelled, reordered or changing state — so a consumer showing the nodes reads them again
  (`docs/work-nodes.md`, 「When a node drops」, 「In herdr」).
- **Pressure on the machine**, `machine-pressure`, is published for each pressure the node service tells
  (「Calls agency makes」 below), as it tells it, with the job each place stands for added, which only
  agency knows: `cause`, `memory` or `disk`; for the memory `fullPct`, `alertPct` and `minutes`, for the
  disk `freeGb` and `limitGb`; `reading`, as the node service gave it, or null; `holders`, what each agent
  place holds, largest first, each its `place`, `megabytes` and `browsers`, the `worker` at work in it and
  the `jobId` and `todoId` its turn named, each null where the place is no worker's at work or its turn
  named none; `largest`, the largest processes, each its `pid`, `name`, `megabytes`, `place` and the
  `jobId` of that place, or null; and `jobs` and `browserJobs`, as the machine's room counts them
  (「Workers」 below). `holders` and `largest` are empty where the node service gave none. Telling the
  chat agent is the consumer's (`docs/agent-control.md`, 「Pressure told as a fact」).
- **The machine's room returned**, `machine-room-returned`, is published once a held launch's notice is
  let go because nothing holds a launch any more: `at`, when the launch was held, and `held`, what held
  it (「Workers」 below). Telling the queue again is the launcher's.
- **The gateway's settling is said in its own log,** not published: the gateway is a process of its own
  beside the server, and a consumer reads the registry's state through the registry's route
  (`docs/own-llm.md`, 「The gateway」).
- **The pool's state is read, not published:** a consumer showing it reads `GET /coworker-pool`, what
  holds each place now (`docs/coworker-pool.md`).
- **A session's system-prompt change**, `system-prompt-changed`, is published as the turn that sent a
  changed prompt records it: the `session` and the `run` (`docs/companions.md`, 「The session」).
- **A road a turn walked**, `road-walked`, is published once for each turn the session contract carries
  on a conversational agent's line, for the unit that keeps the road records: the `stage` (`chat`), the
  `chatId`, the `run`, the `provider` and `model` it went out on, its `outcome`
  (`answered` or `error`), `ms`, how long it held the run, its `inputTokens`, `outputTokens`,
  `cacheReadTokens` and `cacheWriteTokens` summed over the models that answered (each null where the
  road reported none), and its `contextRatio` (or null) (`docs/run-records.md`, 「The roads a turn
  walked」).
- **An account becoming unusable** is told by `account-login-notice` for its login and by
  `deployment-resting` where it was the last login that answered (below); nothing else is published for
  it.
- **A fallback answering**, `fallback-answering`, is published once for each move of the rooms onto a
  stand-in: when a substitute first answers while Claude Code's road is closed, and when it later drops
  to another. It carries the owner (`ownerId`), the substitute that answered before where a fallback
  already stood (`previous`), and the `provider` and `model` answering now (`docs/cache-fallback.md`,
  「While a fallback stands, the room is not silent」). What the owner is told of it is the consumer's.
- **A fallback ended**, `fallback-ended`, is published once Claude Code answers a turn across rooms with a
  session of its own while a fallback stands: the mark of the stand-in answering is cleared, and the event
  carries the owner (`ownerId`) and the stand-in that answered last (`previous`). What the owner is told of
  it is the consumer's.
- **An account's windows read by a turn**, `usage-windows-read`, are published as the turn reads them:
  the mode and the account, the moment they were seen, and each window's `name`, `utilization` (the
  fraction used), `resetsAt` (or null), `durationMinutes` (or null) and `limitName` (or null). The cache
  siren keeps them; agency keeps none and reads them back through 「The cache siren」 below
  (`docs/agent-control.md`, 「The session record and usage are not squeezed out of the screen」).
- **A run come back after a failure**, `run-recovered`, is published once when a run answers again
  while a failure stands: its `fact`, its `intent`, its `kind` and its `ttlMs`, how long it stays worth
  saying (`docs/run-records.md`, 「Why a run died」). Agency voices nothing; saying it in the room is the
  consumer's.
- **What a run spent**, `usage-spent`, is published as a run's answer reports it, for the reader that
  keeps usage: the `agent` it is filed under, the `run` it came from (or null), `at` in epoch seconds,
  and `rows`, one for each model it spent tokens on, each its `account` (empty on a road that holds no
  logins of agency's), `mode`, `model`, `tokens`, `cost`, `inputTokens`, `outputTokens`,
  `cacheReadTokens` and `cacheWriteTokens` (`docs/agent-ownership.md`). Agency keeps no usage rows.
- **A worker's session answered on a lesser model**, `worker-downgraded`, is published as the heal reads
  it: the `worker`, the model it was `launched` on, the models it `answeredOn`, whether its session's
  record was `dropped`, and the `job` it ran for (or null) (`docs/cache-fallback.md`). Telling the owner
  is the consumer's.
- **A session folded**, `session-folded`, is published once a fold has reached, or may have reached, a
  worker's session, naming the `session`: whatever a consumer sent that session to hold — a co-work
  result among it — is folded away with it, and the consumer lets its own record of it go
  (`docs/sessions.md`, 「Session compaction」).
- **What a job's end left in herdr**, `herdr-leftovers`, is published when the sweep after a job's end finds
  workspaces it would not close, once for each picture: `panes`, the count, and `workspaces`, each its
  `workspaceId`, `label`, `panes` and whether it is `marked` as this system's, and `account`, an English
  sentence of it (`docs/agent-control.md`, 「One standing pane per agent」). Telling the owner is the
  consumer's.
- **A worker removed**, `worker-removed`, names the `worker` whose row and sessions went, so a consumer
  lets go what it lent it.
- **An event another process of agency's published** reaches the server's streams too: `bmo-agency`,
  before it ends, posts each event its process published to `POST /events/publish` — the event's `kind`
  and its `body` — and a TypeScript process of agency's posts each as it publishes it, a login's end
  told among them; the server publishes it to every stream reading that kind as it stands. Such a process
  finds the server on the port and with the token its environment names, else those of the data root's
  `env/agency.env`, and posts nothing where neither names a token. A word that did not land is said once
  on the process's error stream, and the process is never failed by it.
- **A login's end told**, `account-login-notice`, is published once for each ending the logins' watch
  finds: the `account`, which `login` it is (`here`, its login on this machine, or `node-token`, its token
  for the work nodes), how it `stands` (`gone`, `ending` or `missing`), `leftMs` (or null), and `notice`, the
  words agency's policy composes of it with the one command the owner runs (`docs/cache-fallback.md`,
  「A login that is about to end」). Saying it in the owner's private chat is the consumer's.
- **A companion's run going in a room**, `companion-run-going`, is published as a run of the companion's
  own starts there and again as it ends: the `agent`, the `run`, the `chatId`, and `going`, true while it
  runs. A consumer that stands cancellations over a room's runs reads which are going from these
  (`docs/companions.md`, 「What the reset cuts off」).
- **The deployment resting**, `deployment-resting`, is published once where a rest closes the last
  login that would answer: the `account` the rest was written under (or null for the service's outage),
  its `kind` (`limit` or `outage`), `until` in epoch seconds, and `notice`, the words agency's policy
  gives it. Saying it in the owner's chat is the consumer's (`docs/cache-fallback.md`, 「Automatic walks
  only on providers, in order」).
- **A companion cut off**, `companion-cut-off`, is published each time a reset moves the companion's
  reset point: the `agent` and the `point`, in epoch milliseconds. A turn begun at or before it is
  overtaken; the consumer holding the companion's rooms lets go unread what waited for its turns there and
  settles its quiet instructions as delivered (`docs/companions.md`, 「What the reset cuts off」).
- **Documents moved**, `documents-moved`, is published as a reload names them: the `paths`, `now`,
  whether the notice is due at once rather than after the hour a chat is told once in, `agent`, whose
  documents they are (or null), and `rooms`, the chat ids of the rooms they concern (empty where the
  reload named none). Which sessions it concerns — the named agent's sessions in the named rooms where
  the reload names them — and what each is told is the consumer's that holds the rooms
  (`docs/sessions.md`, 「Document reload」).
- **A run past its clock asked about**, `clock-asked`, is published as agency opens an ask on a clocked run
  (`docs/agent-control.md`, 「A clock asks before it cuts」): the `ask`, its number; the `agent` the run is
  for and the `run`; `what`, an English sentence of what the run is doing; `startedAt`, `boundMs`,
  `lastActivityAt` and `lastActivity` (each null where the run cannot say); `answerBy`, the moment agency
  cuts the run itself where no verdict has come, in epoch milliseconds; and `verb`, the command a verdict
  is given with. Queuing the order for the chat agent in the owner's private chat, naming the procedure
  `overtime`, is the consumer's that holds the rooms.
- **An ask closed**, `clock-ask-closed`, is published as an ask is settled: the `ask`, the `run`, the
  `verdict` (`keep`, `cut`, or `ended` for a run that ended on its own while asked) and `by` (`agent` for
  a verdict given, `agency` for a cut for want of one, or null), so the consumer withdraws an order not
  yet delivered.
- **A companion reset**, `companion-reset`, is published once a reset has taken agency's part: the
  `agent`, the `ranges` taken (`conversation`, `person`), the rooms it answers `alone` and those it
  answered `shared`, who asked (`by`) and when (`at`, in epoch seconds). Each unit holding rows of the
  companion's — a room's messages and reads, a gate's judgements, its usage, a copy of a store — takes
  its own of the ranges named, and the whole record of each room it answers alone
  (`docs/companions.md`, 「Starting it afresh」).

## Hooks

Each hook has a default that does nothing, so agency runs with none registered.

- **Registered across the boundary.** `POST /hooks` takes a `point`, by its name, a `url` and a `token`
  where the consumer wants one, and registers the consumer's address there in place of any before;
  `POST /hooks/remove` takes a `point` and lets it answer its default again; `GET /hooks` answers
  `points`, each its `point` and whether one is `registered`. Each ask is posted to the address as
  `{ "point", "ask" }`, under `Authorization: Bearer <token>` where a token was given, and the answer is
  read from the `answer` of a 2xx answer. An address that does not answer within ten seconds, answers
  otherwise or answers what does not read as the point's answer is told once on agency's error stream,
  and the point answers its default. The points registered this way are `room-title`, `pane-claims`,
  `cowork-places`, `session-summariser`, `typed-line` and `road-verbs`; a registration lasts as long as
  the server's process.

- **Turn assembly and reference selection are no hook:** the caller composes the prompt and the turn's
  context — the material it chose to stand at the turn's head among them — and hands them in with the
  turn (「Sessions」 above). Agency reads none of it and hands it to the harness as it stands
  (`docs/sessions.md`).
- **Permission and rule judgement are no hook:** a question a program puts to a person in the middle of
  a worker's or a thrown-away session's turn is put to the turn's caller over the session contract, which
  judges it and answers it there, and on every other scope's turn is left standing for a person
  (「Sessions」, 「A question in the middle of a turn」; `docs/agent-control.md`, 「A confirmation in the
  middle of a turn」).
- **The output check is no hook:** agency sends nothing to a room. A turn's answer is published as
  `turn-ended` (「Events」 above), and whatever a consumer checks of it before it leaves is the
  consumer's (`docs/companions.md`, 「What a turn shows」).
- **The submit hook**, `typed-line`. A line a person types into an agent's pane is composed by the
  consumer that owns the agent, not by agency or the gateway (`docs/own-llm.md`, 「Claude Code in the
  pane」). The pane's submit hook asks `POST /typed-line` with the `text` and the `sessionId`; agency asks
  the point with both and answers the `context` the consumer composed, or 409 where it composed none. The
  default composes nothing, and the line goes on as typed.
- **The verbs a room's road performs**, `road-verbs`. A procedure read in a room's turn is held to the
  verbs the road it answers on performs (`docs/tools.md`, 「The procedures」), which the consumer that
  holds the rooms answers. `GET /road-verbs` takes the `chat` and, where the turn names one, the `via`
  road; agency asks the point with both and answers `can`, the verbs. The default answers none, and a
  procedure read in a room keeps none of its marked parts.
- **The conversation limits are no hook:** a provider declares whether a conversational turn is watched,
  and agency's policy holds the limits it is watched against and cuts the turn at the first it crosses
  (`docs/companions.md`, 「A turn that goes nowhere」, `docs/agents.md`, 「What a provider declares for a
  conversation」).
- **The owner's end, for a reservation, is no hook:** each owner releases its own reservation, and the
  pool releases a native one once the CLI process that set it is gone, never for time passing
  (`docs/coworker-pool.md`, 「Reserving places and keepalive」).
- **The co-work places, for a reservation.** The consumer whose co-work reserves answers, for a
  reservation's owner, the checkout its order works in and the places its tasks sit in now, each the
  checkout and the place's number; agency warms that pool's standing checkouts and counts those places
  as busy (`docs/coworker-pool.md`, 「Standing branch checkouts and their list」). The default answers
  none, and that reservation's places are not warmed.
- **The pane claims.** A consumer that keeps panes of its own in agency's herdr — a slot it stands an
  agent in — answers which agents those panes hold, each with its label and whether it may be taken
  down now for standing idle. Agency counts a claimed agent as standing, so its leftovers sweep spares
  it, and on its heads' pass takes down each claim marked idle whose agent nothing is running in
  (`docs/agent-control.md`, 「One standing pane per agent」). The default claims nothing.
- **A room's title.** The tab a room's following pane stands in is named by the room's title, which the
  consumer that owns the room answers for its chat id. The default answers none, and the tab is named
  by the chat id.
- **The job guard.** A job worker's shell commands and file edits are judged before they run by the
  guard of the service that holds the jobs, not by agency (`docs/live-claude-guard.md`, 「First layer: a
  hook that reads shell commands before they run」). The deployment names that guard in agency's
  environment as `BMO_JOB_GUARD`, a command line, and agency hands it to every worker program it
  starts; the worker's `PreToolUse` hook runs it through `sh` with the hook's input on standard input,
  and its output is the hook's answer. With none named, the hook judges nothing and every call goes on.
- **The session summariser.** As a session is replaced, handed over or set aside for another model,
  agency asks the summariser for the note the next run is handed: what kind of session it was (`chat`,
  `worker` or `switch`), the provider it stood on, its session id, its folder, the login it was opened
  under, what was said on a road that keeps no session of its own, the note the worker wrote itself
  where it wrote one, and the room. The summariser answers the whole note, wording and all, or none
  (`docs/sessions.md`, 「Session replacement」). The default answers none, and nothing is carried over.

## Co-worker pool

- **The pool is a module of agency's server,** sharing the request, the tiers, the roads and the routes
  with the control routes; its whole mechanism is `docs/coworker-pool.md`.
- **The routes** take a place as a prior or a yielding request, raise a yielding request to a prior
  one, release and cancel a lease by its token, and set and release a reservation. A builder turn takes
  its place keyed by its checkout. The pre-reserve and release verbs a job runs are routes of this same
  surface (`docs/coworker-pool.md`, 「Native co-worker places」).
  - `POST /pool/enqueue` takes `source` (`cowork`, a branch's seat, or `turn`, a builder turn's place),
    `requestKey`, `owner`, `role`, `selection`, `tier` and `provider` where named, `generation`,
    `yielding`, the `token` of the request it asks again and the `placeKey` of a turn, and answers the
    request; `POST /pool/acquire` takes its `token` and answers the `request` let in, or null while it
    waits; `POST /pool/bind` takes the `token` and the `childId` of the unit it launched; `POST
    /pool/finish` takes the `token`, the `outcome` and `cancel`, and answers whether it was `held`.
  - `POST /pool/reserve` takes `reservationKey`, `owner`, `source`, `role`, `selection`, `tier` and
    `provider` where named, and `count`, and answers the reservation; `POST /pool/reservations/release`
    takes its `token` and the `outcome`; `GET /pool/reservations?owner=` answers the `reservations` held.
  - A request and a reservation are answered as their table holds them, in camelCase; a step the pool
    refuses is answered 422 with the pool's own words.
- **The tables whose contract is shared:** the request table, the leases on it, the reservation table,
  the ping table, the per-provider-and-tier limit table, and the standing checkouts list. A consumer
  reads them to show the pool and writes a limit; the lease token, the owner proof and the snapshot are
  the pool's own.
- **The standing checkouts.** A consumer asks a branch's place with its order's checkout, the task's
  selection, role and tier, and the places its tasks sit in now; agency answers the place's number and
  its folder, which a branch's thrown-away session names as its `cwd` (「Sessions」), lays the
  checkout's open change into it, records what a finished branch left there, and takes down what stands
  in herdr for the places an ended order worked in that no task still sits in (`docs/coworker-pool.md`,
  「Standing branch checkouts and their list」). The labels and keys of the consumer's work arrive with
  each request; agency reads none of its orders. `POST /places/take` takes `cwd`, `selection`, `role`,
  `tier` and `held`, each a `cwd` and a `slot`, and answers the `slot` and the place's folder as `cwd`;
  `POST /places/prepare` takes the order's `source` and the place's `cwd`; `POST /places/remember` takes
  `cwd`, `slot`, `role`, and the `mode`, `model`, `effort` and `upstream` the branch ran on; `POST
  /places/retire` takes `cwd`, the `used` places and `held`.
- **The clocks** are agency's own: every five seconds a reservation's head place is raised and a native
  call whose caller ended is reclaimed, every ten minutes what a job's end left in herdr is swept
  (`docs/agent-control.md`, 「One standing pane per agent」), and on the keepalive pass's period the
  held reservations' places are warmed where the cache warmer's schedule lets pings go (「Calls agency
  makes」, the cache warmer).
- **The hooks** are the owner-end proof and the co-work places (「Hooks」 above). The pool hands an
  owner's name to them as it stands, reads none and judges nothing.

## Run log and run records

- **The run log** is every line a run wrote, under the run's name, written as the run streams and read
  by a consumer to show it (`docs/run-records.md`, `docs/agent-control.md`). Agency writes it; a
  consumer reads it, directly or by `GET /run-log?run=<run>&after=<id>`, which answers the run's `lines`
  after the one given, five hundred at most, each its `id`, `agent`, `run`, `chatId`, `at`, `kind`,
  `name`, `parent`, `ref`, `text`, `jobId`, `state`, `truncated` and `detail`. `GET
  /run-records?run=<run>` answers its `records`, each its `id`, `kind`, `text`, `recordId`, `wakes` and
  `at`.
- **The run records** hold one line per road a run walked — work key, room and message, provider and
  model, outcome, time, and the tokens the run reported — and the usage filed under the agent
  (`docs/run-records.md`, `docs/own-llm.md`, 「Which road answered is recorded」). A consumer reads them
  to show a turn's figures and its gates'; agency builds no run from them.
- **The run ledger** is one row a run of an agent's unit, which the job unit opens as it starts the unit
  and closes as it ends: `runs`, its `id`, `agent`, `chat_id`, `unit`, `depth`, `cap_ms`, `started_at`,
  `finished_at`, `outcome`, `started_by`, `context_tokens` and `search_id`, its times in epoch seconds.
  The routes take and answer it in those column names.
  - `POST /runs` takes an opening, `agent`, `chat_id?`, `unit`, `depth`, `cap_ms?`, `started_at`,
    `started_by?` and `search_id?` (a number or a string, kept as text), and answers 201 with the new
    row's `id`; an opening missing a field it needs is refused 400.
  - `GET /runs/{id}` answers the run: `id`, `agent`, `chat_id`, `unit`, `depth`, `started_at`,
    `finished_at`, `outcome` and `context_tokens`, each null where the row holds none; a run with no row
    is answered 404 with the code `nothing-held`, the base chat library's code for nothing kept at the
    address a path names.
  - `POST /runs/{id}/close` takes `finished_at`, `outcome` and `context_tokens?` and answers 204; a run
    already closed keeps its closing and is refused 409, and a run with no row is answered 404 as above.
  - `GET /runs?search=<search>` answers `runs`, the ids of the runs that carried the search, in id
    order; the search is matched as the text it is kept as, so a caller may name it by its number. A call
    naming no search is refused 400.
  - `POST /runs/{id}/launch` takes `title`, `agent` and `command`, the program the run is: `program`,
    `args`, `cwd` and `environment`, an object of variable names to values. Agency leaves the program in
    the agent's host, its execution rooted on the run's unit, so the unit's name reads as alive while it
    runs and a stop of that name reaches it, and a person watching the agent's place reads it by the
    title. The run closes its own row as it ends. It answers 204 once the program has started; where the
    host does not start it, agency closes the row with the outcome `error`, finished now, and refuses 503
    with the sentence saying why. A run with no row is answered 404 as above, a run already closed is
    refused 409 with nothing started, and a body missing a field, or naming no program, is refused 400.
    The launch is carried to its end whatever becomes of the caller, so a caller that stops waiting
    never leaves a refused run's row open.
- **The judgement a gate produces** is the consumer's record, written beside the road record agency
  keeps; agency keeps which road a verdict's time was spent on, and the consumer keeps the verdict
  (`docs/run-records.md`).
- **A stopped run's record** says it was stopped, not that it had no answer, and agency raises the stop
  to whoever asked (`docs/agent-control.md`, 「Stopping」, `docs/own-llm.md`, 「A gate somebody stopped is
  not a gate with no answer」).

## Models, registry, tiers, roads and fallback

- **The registry** holds the registered models and their addresses; a consumer offers a registration by
  name and agency serves it through its gateway (`docs/own-llm.md`). The routes register, change and
  remove a registration and its addresses, and list them with whether each key is usable; a key is
  written once in the body of the save and never read back (`docs/own-llm.md`, 「The keys」).
- **The tiers** are a consumer's words for a piece of work; agency resolves a tier, a `role:<name>`
  assignment and a work key to a provider, a model and an effort, read afresh for each run
  (`docs/model-tiers.md`). The routes edit the tier mappings and the work-key assignments.
- **The roads and the walk** are the provider order and the automatic walk across Claude Code, Codex,
  OpenRouter and the registry's provider, with the sanitiser gate, the per-key run mode and the account
  judgement (`docs/cache-fallback.md`, `docs/provider-presence.md`). The routes set a key's run mode and
  the provider order, and turn a provider on or off.
- **Whether a stand-in answers** an agent while the rest closes Claude Code's road is answered for the
  consumer that gates rooms on the rest: `GET /stand-in?agent=<name>` answers `answers`, true where the
  agent is pinned to a provider other than Claude Code or its key's walk meets a stand-in now, as the
  cache siren reads the logins' windows (`docs/cache-fallback.md`, 「While a fallback stands, the room is
  not silent」).
- **The catalogue** holds the model files the deployment's own engines serve, and choosing one fetches
  and installs it (`docs/model-catalogue.md`).
- **The gateway** is agency's one translator in front of every registration; its address and key are
  read in one place by every caller (`docs/own-llm.md`, 「The gateway」).
- **The events** are the gateway's settling and a road's outcome as a run records it.

## Accounts

- **An account is a login agency walks and keeps warm.** A consumer names no account; agency chooses
  which answers a run, by the account judgement and the walk (`docs/cache-fallback.md`, 「The Claude
  Code road is a list of accounts」, 「The account judgement」).
- **The routes** list the accounts with each one's window and whether it is usable, set a provider's
  per-key choice of automatic or one provider, and order, enable and disable the accounts and providers
  (`docs/cache-fallback.md`, 「The per-key choice is automatic or one provider」).
- **The tokens for the work nodes** are each account's long-lived token, kept outside the store and
  carried to a node with each run; the command that makes one, and the warning before one expires, are
  agency's (`docs/work-nodes.md`, 「The logins on a node」, `docs/cache-fallback.md`, 「A login that is
  about to end」). No token crosses this boundary: a consumer reads only whether an account is usable.
- **The quota source.** On a loopback port of its own, the one `AGENCY_QUOTA_PORT` names (9200 where it
  names none), and with no token, agency answers `GET /api/claude` for the cache siren: the login the
  deployment's chat runs on first, marked active, then the provider's other logins that are on, each with
  its two headline windows as the siren keeps them. It answers that one path and nothing else.
- **The rest.** Agency answers whether the deployment rests now and until when: while the service's own
  outage rest stands (`rest_until` with `rest_kind` set to `outage`), or while every enabled account's
  own rest stands (`rest_until:<account>`, or a window of its own read full), until the soonest of them
  ends; and why, `outage` or `limit` (`docs/cache-fallback.md`). While it rests, Claude Code's road is
  closed to a job's selection. `GET /rest` answers `resting`, `until` in epoch seconds and `kind`; `POST
  /rest` takes `until`, `kind` (`limit` or `outage`) and the `account` a limit is its login's, and answers
  whether the rest is the one that `silenced` the deployment.
- **A rest that has passed.** `GET /rest/passed` clears every rest whose moment is behind now — the
  service's own and each enabled account's — and answers `kind`, why the deployment rested, where it was
  silent before and rests no longer: `outage` where the service's own rest held it, `limit` otherwise.
  It answers null where no rest passed, where the deployment was not silent, or where it still rests.
  The read takes what it reports, so a second call answers null.
- **The event** agency publishes is an account becoming unusable and coming back.

## Workers

- **A worker is an agent agency keeps a slot and a checkout for.** The routes register a worker, set
  its pin — a provider, or a registration — its sampling and its settings, and list the workers with
  their configured and answered roads (`docs/worker-slots.md`, `docs/own-llm.md`, 「A worker pinned to a
  provider」).
- **A worker's bench** is the standing checkout and workbench agency keeps for it, on this machine or a
  node (`docs/worker-bench.md`, `docs/work-nodes.md`).
- **A worker's tools** are the worker MCP server agency hands its runs (`docs/worker-mcp.md`), and the
  language a worker reads and writes is the worker-language declaration (`docs/worker-language.md`).
- **A worker's jobs** are the consumer's to plan and hand in; agency carries each job's turn into the
  worker's session, on its pin, and reports the turn's lifecycle as an event (「Events」 above). What a
  job is and when it runs is the job unit's.
- **A job's stop mark.** A consumer stopping a job marks it stopped with its reason before the job's
  unit is asked to stop (`job_stop:<job>`); the mark is read by whoever closes the job's record, so it
  closes as stopped, and taken away with that record. Agency keeps the mark and judges nothing of it:
  `POST /jobs/stop-mark` takes the `job` and the `reason`, `GET /jobs/stop-mark?job=` answers the
  `reason` or null, and `POST /jobs/stop-mark/clear` takes the `job`.
- **A road's block.** A failure that closes a road for a while blocks it until a moment, with its
  reason: a provider, or on a provider whose models are services of their own the one model
  (`job_route_block:<provider>` or `job_route_block:<provider>::<model>`, the value `until` in epoch
  milliseconds and `reason`). Agency reads a road blocked while the moment is ahead, besides a provider
  switched off and Claude Code while the deployment rests, and answers whether an item has a road open
  now: some selection its job may go out on, in the order the job walks them, that is not blocked. `POST
  /roads/block` takes a `selection`, `until` and `reason`, or no `until` to open the road again; `GET
  /roads/open?selection=` answers whether that road is `open`, and `GET /roads/open?model=&worker=`
  whether an item's job has one.
- **Stopping runs.** A consumer asks a run to stop by its name, a unit's or an execution's: a
  cancellation stands over it and everything it owns, a run in a pane is asked to end and its pane closed
  after its grace, a resident turn is left to its collector with its provider's interrupt behind it, and
  a unit still standing is stopped by systemd (`docs/agent-control.md`, 「Stopping」). A consumer ending
  several runs at once — a work order changed — has them stopped and confirmed gone first, each pool
  place given back and each cancellation taken back; the refusal names the units still standing, and
  closing the consumer's own records of those runs is the consumer's. `POST /runs/stop` takes one `run`,
  or `runs`, each its `unit` and the `poolRequest` it held, and a `reason`; the refusal of a set whose
  units still stand is 409. Before a unit that holds one of the brain's watches is stopped, by this route
  or any other stop, the brain is told the watch stopped (「Calls agency makes」, the brain), so its watch
  ends as cancelled rather than being launched again; a brain that cannot be told is said once on
  agency's error stream and the unit is stopped all the same.
- **A run's clock.** A run agency holds to a clock — an editor run among them — is asked about past its
  bound rather than cut (`docs/agent-control.md`, 「A clock asks before it cuts」). `GET
  /runs/clock?run=<name>` answers, for the clocked runs standing under that name or as its children,
  `capEnds`, the moment the latest of their bounds runs out, and `keptUntil`, the moment the latest keep
  or open ask on them stands until, in epoch milliseconds, each null where none stands.
- **The asks.** `GET /runs/overtime` answers `asks`, every open ask, each its `ask`, `agent`, `run`,
  `what`, `askedAt` and `answerBy`. `POST /runs/overtime` takes the `ask`, the `verdict` (`keep` or
  `cut`), `minutes` for a keep (the run's own bound where none is named, a hundred and twenty at most)
  and `why`, and answers the ask's `ask`, `run`, `verdict` and `keptUntil` (or null); an ask already
  closed is refused 409 with `ask-closed`, and no ask of that number 404. `bmo-agency overtime` gives the
  same verdicts from a shell.
- **The editor cap.** `GET /editors/cap?workKey=<key>&agent=<name>` answers `capSeconds`, the bound an
  editor run of that key for that agent carries: thirty minutes, or the request wait of the registration
  the agent's recollectors run on where that is longer. A key that is no editor's is refused 422.
- **Which names are alive.** Agency answers which of the names asked are running — held by systemd, named
  by an open execution as its parent or root, or held as a pane's run by herdr — or that nobody could
  say, which is never read as nothing running (`docs/agent-control.md`, 「Asking whether it is alive」).
  `POST /runs/alive` takes `names` and answers `answered` and the `alive` among them, null where nobody
  could say.
- **A support worker removed** publishes `worker-removed` (「Events」 above), so a consumer lets go what
  it lent the worker, such as a checkout lease.
- **The machine's room.** The launcher, the job unit, asks agency before it launches a job on this
  machine and holds the launch while anything is held (`docs/agent-control.md`, 「Room on the machine」).
  `GET /machine/room` answers `held`, every reason a launch is held now, each an English sentence with
  its figure and its limit, empty where there is room: what the node service's machine room holds, then
  agency's own job-count limits met; `jobs`, the jobs at work on this machine; `browserJobs`, those whose
  place runs a browser; and `browsers`, the browser main processes running on it. A node service that
  cannot be asked holds nothing of its part, and the job counts hold all the same.
- **A held launch told.** `POST /machine/launch-held` takes `at`, when the launcher held, in epoch
  milliseconds, and `held`, what it held on, and answers `ok`; agency keeps the notice, and an empty
  `held` withdraws it. While a notice stands agency reads the room every minute, and once nothing is
  held it lets the notice go and publishes `machine-room-returned` (「Events」 above), on which the
  launcher tells the queue again.

## Health

- **Agency answers its own health across the boundary** so a consumer shows the system's state without
  reaching inside it.
- **The routes** read each node's state, load and link (`docs/work-nodes.md`), the gateway's settling
  and which table it serves (`docs/own-llm.md`, 「The gateway」), the registry's and the accounts'
  usability, the pool's active, reserved and waiting counts (`docs/coworker-pool.md`), and the rotation
  — where the next launch and the next heavy run go (`docs/work-nodes.md`, 「The rotation」).
- **The events** are a node's state changing, the gateway settling, and an account becoming unusable,
  so a consumer's health view follows without polling.
- **`GET /health`** answers `pid`, `code` and `ready` (below), `nodes` as the node service keeps them or
  the `error` it answered with, `pool`, the count of its `active`, `reserved` and `waiting` requests, and
  `accounts`, each its `id`, `name`, whether it is `enabled` and whether it is `usable` now.
- **Readiness.** The health route answers the server's process id, the code tree it reads its code
  from, and whether its places have stood once since it started. A deployment that starts agency on a
  pinned release waits for an answer naming its own process and the tree it pinned, and ready; an
  answer from an earlier process or another tree is not yet the release it started.
- **Agency reports faithfully:** a node not ready says what is missing, a gateway still settling says
  so, and a key that is not usable says it is not; nothing is shown ready that is not.

## Calls agency makes

- **Agency calls four services:** the cache siren for an account's usage windows and the cache lifetime
  a session is judged warm by, the cache warmer for whether keepalive pings go, the node service for
  this machine's room and what its places hold, the pressures it tells, a node's link, the nodes it keeps
  and the SSH login to each, and the brain for what a companion's reset
  does to what the agent remembers and to its brain folder, and for a watch whose unit is stopped. Each is reached at the base address the deployment names in its
  variable, with the token named beside it.
- **A service whose address is unset is not called:** each call then answers the default stated beside it
  below, so agency runs with none of the four deployed. An address that does not read as a URL stops the
  service as it starts, naming the variable.
- **Every call is one POST of a JSON body** under the service's base address, field names in camelCase,
  an operation that sends nothing sending an empty object, with `Authorization: Bearer <token>` where a
  token is named. The calls follow the chat wire protocol's conventions (the protocol document of
  `bmo-chat`): an answer whose `answered` is false carries a `refusal` with its `code` and `message` and
  is a refusal; a non-2xx answer carries `code` and `error`; a call is offered again only where the
  connection was refused, within the service's patience, and never once it reached a listener. The node
  service's machine standing and its event stream are the exceptions, GETs as that service answers
  them.

| Service | Address | Token | Patience |
| --- | --- | --- | --- |
| The cache siren | `AGENCY_CACHE_SIREN_URL` | `AGENCY_CACHE_SIREN_TOKEN` | none: a refused connection is a fault at once |
| The cache warmer | `AGENCY_CACHE_WARMER_URL` | `AGENCY_CACHE_WARMER_TOKEN` | none |
| The node service | `AGENCY_NODE_URL` | `AGENCY_NODE_TOKEN` | none |
| The brain | `AGENCY_BRAIN_URL` | `AGENCY_BRAIN_TOKEN` | two minutes, offered every two seconds |

### The cache siren

| Operation | Path | Sends | Answers | With no address |
| --- | --- | --- | --- | --- |
| usage windows | `/usage/windows` | `mode`, `account` | `windows`: each `name`, `utilization` (the fraction used), `resetsAt` (or null), `seenAt`, `durationMinutes` (or null), `limitName` (or null); and `freshSeconds`, how long a reading stands | no windows, and no reading fresh |
| cache lifetime | `/cache/lifetime` | `mode`, `model` | `minutes`: how long after its last answer a session on that provider and model stays warm, or null where the siren judges it by none | none, and agency reads the provider's declared lifetime (`docs/sessions.md`, 「A worker's session per model」) |

### The cache warmer

The warmer holds the keepalive schedule: whether pings go, and on which provider. Agency asks it before
it judges a reservation's places and again before each ping goes, and holds the rest — which places are
due, the ping itself and its record — itself (`docs/coworker-pool.md`, 「Reserving places and keepalive」).

| Operation | Path | Sends | Answers | With no address |
| --- | --- | --- | --- | --- |
| ping schedule | `/pings/schedule` | `kind`, what is warmed (`coworker-pool` for the pool's reserved places); `mode` | `go` (boolean): whether keepalive pings of that kind on that provider go now | no ping goes |

### The node service

| Operation | Path | Sends | Answers | With no address |
| --- | --- | --- | --- | --- |
| machine room | `/machine/room` | nothing | `held`: what this machine is short of for another launch, each named; empty where it has room | room, nothing held |
| link state | `/links/state` | `node` | `up` (boolean): the node's link is connected and was heard from lately | down |
| node rows | `/nodes/list` | nothing | `nodes`, in the node list's order: each `node`, `label` (or null), `kind` (`ssh`, or `jake` for a node reached through a hub), `state` and `detail` (or null) | no nodes |
| node login | `/nodes/login` | `node` | `ssh`: the arguments that open a batch SSH login to the node, ahead of the command, or null where the node has no SSH route | none |
| machine standing | `GET /machine/standing` | nothing | `browsers`, how many browser main processes run, and `places`, what each agent place holds: `place`, `megabytes` and `browsers`; the rest of the standing is not read | no browser, no place |
| pressure | `GET /events?kinds=machine-pressure` | nothing | a stream of server-sent `machine-pressure` events, each as the node service's boundary states it | no event |

- **A node's kind** is how agency tells a node reached through a hub apart: such a node offers no shell,
  so it carries no job and its pane offers no login (`docs/work-nodes.md`, 「In herdr」). Agency keeps no
  record of the hubs.
- **The stream** is read with no wait on its whole, from the server's start for as long as it runs, and
  opened again ten seconds after it drops or cannot be opened. An event that does not read is said once
  on agency's error stream and passed over.

### The brain

| Operation | Path | Sends | Answers | With no address |
| --- | --- | --- | --- | --- |
| fence | `/agents/fence` | `agent` | `ok`; no writer begun before now takes the agent's folder's editing lock again | nothing fenced |
| forget | `/agents/removed` | `agent` | `ok`; the agent's editing lock, its ticket queue and its fence are removed | nothing to remove |
| lock path | `/agents/lock` | `agent` | `path`: the agent's folder's editing lock, relative to the data root, or null where the agent keeps no folder | none |
| book start | `/agents/book-since` | `agent`, `chatId`, `at` | `ok`; the agent's person book of the room reads from `at` on, in seconds | nothing written |
| recollector put-back | `/agents/put-back` | `agent` | `ok`; what the agent's stopped recollector runs left unwritten in its folder is undone | nothing undone |
| reset | `/agents/reset` | `agent`, `preview` (boolean) | `took`: the count of the agent's rows per table and whether its folder stands; with `preview` false, all of it taken, the folder fenced and its lock held meanwhile; and `couldNot`: what it could not do | nothing taken, nothing it could not |
| watch stopped | `/watches/stopped` | `unit`, a unit named `bmo-watch-<id>-<milliseconds>` about to be stopped | `ended` (boolean): the standing watch that unit holds ended as cancelled, its room told nothing more is coming; false where it holds none | nothing ended |
