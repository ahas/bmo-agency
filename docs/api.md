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
  manifest is this boundary's version; the current line is `0.1.0`.
- **A change to this document and a change to the boundary go in one commit.** A change that makes a
  route, an event, a hook or a shared table's contract different repairs this document in the same
  commit, stating what is true now.
- **The version moves by what changed:** the major on a change to a route, an event, a hook or a
  table's contract that an existing consumer reads; the minor on an addition of one; the patch on
  anything else.

## Agents

- **An agent is a place agency stands and controls.** A consumer names an agent by its stable name;
  agency holds where it stands — the herdr it is in and the node, if any — its one standing pane, and
  its lifecycle (`docs/agent-control.md`, `docs/agents.md`).
- **The routes** register an agent and its home, stand and retire its place, read its place's state,
  and list the agents and their panes. A place's home is read and used together with the agent, since a
  remote agent's pane and ids are its node's (`docs/work-nodes.md`, 「The bot's side of a remote
  agent」).
- **An agent's role** is the tools and the policy its runs are shown, named by the consumer and
  resolved by agency (`docs/agent-roles.md`). The role a co-worker place holds is agency's policy's
  (`docs/coworker-pool.md`, 「Standing branch checkouts and their list」).
- **An agent's documents** are the files agency composes a run's launch from, read from the deploy
  tree and the prompt tree (`docs/agent-documents.md`, `docs/base-prompts.md`, `docs/launch-lines.md`);
  a consumer hands in the documents it owns and agency hands them to the harness as they stand.
- **Usage is filed under the agent that spent it,** never added to another's; a run names the agent it
  belongs to and the run it came from (`docs/agent-ownership.md`). A consumer reads an agent's usage
  across the usage boundary described there.
- **The event** agency publishes for an agent is its place standing, its pane changing, and its
  retirement.

## Sessions

- **The session contract is seven operations,** declared by the session crate and reached by every
  caller through it: start a session, send a turn into it, read its state, interrupt it, fold it, end
  it, and list sessions. herdr is one backend of that contract; a caller names no backend
  (`docs/sessions.md`, `docs/agent-control.md`).
- **A turn is sent with its prompt alone;** the session keeps the conversation, and agency computes
  only what is new (`docs/sessions.md`, `docs/agents.md`, 「The session's scope」). What the prompt
  carries is the consumer's, composed above this boundary (「Hooks」 below).
- **A session's scope** — one session, a room's session, a worker's session — is the consumer's
  declaration; agency keeps one harness session per scope and resumes, folds or hands it over as the
  scope's provider declares (`docs/agents.md`, 「What a provider declares for a conversation」).
- **Reading state** answers the session's id, the provider and model it stands on, its place, its
  generation, and the context its last request carried against the registration's window
  (`docs/own-llm.md`, 「How requests go」). A consumer reads it to show a session; it builds no run from
  it.
- **Folding and handing over** are the provider's way of carrying a full session past the window; a
  consumer that keeps a conversation reads the handoff the provider declares rather than agency's fold
  (`docs/companions.md`, 「Its limit」).
- **A reset of a session** is an ask a consumer writes and the next turn carries out; agency takes the
  program down at once and opens a new generation (`docs/session-resets.md`, `docs/companions.md`,
  「Clearing it」).
- **The events** are the turn's lifecycle (「Events」 below).

## Events

- **Agency publishes a turn's lifecycle:** the turn started, waiting for its engine with what holds it,
  the answer being written, sent, failed with its reason, or cut short by a limit its provider declared
  (`docs/companions.md`, 「What a turn shows」, 「A turn that goes nowhere」). A consumer reads them to
  show a turn and to wake what a turn's end should wake; agency waits on no reader.
- **A run's log lines** are published as the run writes them, so a consumer shows a long turn as it
  goes rather than as a silence (`docs/agent-control.md`, 「A job gone quiet」).
- **A node's state** is published as it changes — ready, dropped, come back, its load and pressure
  (`docs/work-nodes.md`, 「When a node drops」, 「In herdr」).
- **The gateway's settling** is published as a new table starts serving and an old one drains
  (`docs/own-llm.md`, 「The gateway」).
- **The pool's grants and releases** are published for a consumer that shows the pool's state
  (`docs/coworker-pool.md`).
- **A session's system-prompt change** is published as the turn that sent a changed prompt records it
  (`docs/companions.md`, 「The session」).

## Hooks

Each hook has a default that does nothing, so agency runs with none registered.

- **Turn assembly.** Before a turn is sent, agency calls the consumer to compose what the turn carries
  — the prompt, and the material it chose to stand at its head. Agency reads none of it and hands it to
  the harness as it stands (`docs/sessions.md`). The default sends the turn's words alone.
- **Reference selection.** A consumer chooses what a turn is given beside its words; agency attaches
  what the hook returns. The default attaches nothing.
- **Permission and rule judgement.** A consumer judges whether a run may take an action its harness
  asks to confirm; agency carries the judgement to the harness (`docs/agent-control.md`, 「Confirming」).
  The default confirms nothing and leaves the harness's own policy to stand.
- **Output check.** Before a sent answer leaves, a consumer reads it — a sanitiser, a repeat check —
  and agency sends what the hook hands back (`docs/companions.md`, 「What a turn shows」). The default
  sends the answer as it was written.
- **The submit hook.** A line a person types into an agent's pane is composed by the consumer that owns
  the agent, not by agency or the gateway (`docs/own-llm.md`, 「Claude Code in the pane」). The default
  sends the line as typed.
- **The conversation limits.** A provider declares the limits a conversational turn is watched against;
  a consumer registers what counts as a turn going nowhere, and agency cuts the turn at the first
  (`docs/companions.md`, 「A turn that goes nowhere」, `docs/agents.md`, 「What a provider declares for a
  conversation」). The default watches nothing.
- **The owner's end, for a reservation.** A reserver registers what proves its owner ended; agency's
  pool releases a reservation only on that proof (`docs/coworker-pool.md`, 「Reserving places and
  keepalive」). The default never releases for time passing.

## Co-worker pool

- **The pool is a module of agency's server,** sharing the request, the tiers, the roads and the routes
  with the control routes; its whole mechanism is `docs/coworker-pool.md`.
- **The routes** take a place as a prior or a yielding request, raise a yielding request to a prior
  one, release and cancel a lease by its token, and set and release a reservation. A builder turn takes
  its place keyed by its checkout. The pre-reserve and release verbs a job runs are routes of this same
  surface (`docs/coworker-pool.md`, 「Native co-worker places」).
- **The tables whose contract is shared:** the request table, the leases on it, the reservation table,
  the ping table, the per-provider-and-tier limit table, and the standing checkouts list. A consumer
  reads them to show the pool and writes a limit; the lease token, the owner proof and the snapshot are
  the pool's own.
- **The hook** is the owner-end proof (「Hooks」 above). The pool reads no owner's name and judges
  nothing.

## Run log and run records

- **The run log** is every line a run wrote, under the run's name, written as the run streams and read
  by a consumer to show it (`docs/run-records.md`, `docs/agent-control.md`). Agency writes it; a
  consumer reads it.
- **The run records** hold one line per road a run walked — work key, room and message, provider and
  model, outcome, time, and the tokens the run reported — and the usage filed under the agent
  (`docs/run-records.md`, `docs/own-llm.md`, 「Which road answered is recorded」). A consumer reads them
  to show a turn's figures and its gates'; agency builds no run from them.
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

## Health

- **Agency answers its own health across the boundary** so a consumer shows the system's state without
  reaching inside it.
- **The routes** read each node's state, load and link (`docs/work-nodes.md`), the gateway's settling
  and which table it serves (`docs/own-llm.md`, 「The gateway」), the registry's and the accounts'
  usability, the pool's active, reserved and waiting counts (`docs/coworker-pool.md`), and the rotation
  — where the next launch and the next heavy run go (`docs/work-nodes.md`, 「The rotation」).
- **The events** are a node's state changing, the gateway settling, and an account becoming unusable,
  so a consumer's health view follows without polling.
- **Agency reports faithfully:** a node not ready says what is missing, a gateway still settling says
  so, and a key that is not usable says it is not; nothing is shown ready that is not.
