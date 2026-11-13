# Roads

How a run gets its provider and its model. Every supervisor and every agent of a role stands on one road: a
provider, one model of it, the effort and the thinking its runs go out with there and, on the registry's
provider, the answer ceiling. Its runs go out on that road and on no other. No row holds an order of
providers, nothing is forced over a road, and nothing stands in for a road that cannot answer: a supervisor's
turn on such a road fails and says why, and a job waits for its agent's road. A supervisor's road is set on its
row (`docs/supervisors.md`), an agent's on the agent (`docs/role-agents.md`). There is no road shared by
everything, no level of work standing between a role and its model, and no limit on how many runs go out on
one provider at once. How the Claude Code provider walks its own accounts inside its road is
`docs/cache-fallback.md`; how quota is read is `docs/usage-windows.md`.

## What a road is

- **A road names one provider and what the row runs on there:**
  - the model, one the provider holds (「Models a provider holds」 below);
  - the effort, a level of the one scale `low`, `medium`, `high`, `xhigh` and `max`, or none for the
    provider's own default; the provider turns a level into its own word (「Effort and thinking」 below);
  - whether the run thinks, where the provider takes the switch;
  - on the registry's provider, the answer ceiling, or none for the registration's own.
- **A road names only a provider agency carries and whose program carries the row's line** (the chat line
  for a supervisor, the role's own line for an agent); a write naming another is refused with the provider and
  why. A supervisor always names a road. An agent may name none and follow the supervisor that asks it
  (「Runs on another's road」 below).
- **The quota share is one for every road:** the remaining quota at or below which a road's provider is read
  as having none, a fraction of its quota (`policy/roles.toml`, `quota_share`, 1%).

## Quota: what a provider has left

- **Each provider holds one quota, read for the provider as a whole,** by the provider's own member out of
  what agency keeps (`docs/usage-windows.md`, 「What reads the windows」):
  - **a provider with usage windows** — Claude Code and Codex, each with a five-hour and a weekly window —
    has left what its window with the least left has left: one less the used fraction of that window;
  - **a provider with a balance** — DeepSeek — has quota while the balance its reader last kept is above
    zero, and none once it is at or below zero; a balance never read reads as quota;
  - **the registry's provider** — a registered model on the deployment's own engine — always has quota;
  - **a provider that reports neither** has quota.
- **Claude Code's quota is its accounts':** the provider has quota while one of its open accounts has quota
  above the share by its own least-left window, and its account walk offers that account first
  (`docs/cache-fallback.md`, 「The account judgement」).
- **A window read at its ceiling and not reset** is no quota left, whatever the share.
- **What a provider has left is read afresh as each run starts,** never kept on the road.

## Whether a road can answer

- **A road is judged as each run starts, and the judgement is kept for the run:** it can take the run while its
  provider has quota above the share and can answer now. Can answer now is the provider's own answer
  (`docs/cache-fallback.md`, 「Availability: whether a provider can answer now」): the owner's switch on it is
  on, its credential stands on this machine, and, for a provider that holds logins, one of them is open. A run
  already going finishes where it went out; a change of the road holds from the next run.
- **It goes out on the model, the effort and the thinking the road names.** An effort or a thinking the road
  leaves to the provider is the provider's own: the effort its program runs at by default, and the thinking
  default of the gateway member or the registration the model is, which is read beside the model
  (「Models a provider holds」); a model of the Claude CLI's catalogue states none, and its thinking is the
  line's own.
- **A supervisor's turn on a road that cannot take it fails at once and says why.** The turn ends failed with
  one sentence: the road, its reason, the moment it reopens where a window names one, and what a person does
  where only a person can reopen it — an account waiting for a person with the sign-in command and that
  account's own configuration directory, a spent balance where its provider's member says it is topped up.
  Nothing is stood in its place and nothing waits: the supervisor's next turn judges the road again.
- **A job on its agent's road waits while the road cannot take it,** held with the same sentence, and goes out
  once it can; it is never refused for that, and holds no slot while it waits (`docs/role-slots.md`, 「The
  line」). A job every Claude account of which refused it ends instead, on the login failure naming each account
  a person must sign in to (`docs/cache-fallback.md`, 「Automatic walks only on providers, in order」).

## A road that fails

- **A run that fails for its provider fails on its road.** A failure for the provider is a limit met, a quota
  or balance refused, a credential refused, the last open login closing under it, or the provider found unable
  to answer as the turn ends; it is read off the run's ending by the one failure reading every run asks
  (`docs/cache-fallback.md`, 「Automatic walks only on providers, in order」). A supervisor's turn ends failed
  with it, naming the road and the failure; a job's turn on its agent's road ends, and the job waits for the
  road to answer again before its next turn (`docs/cache-fallback.md`, 「Automatic walks only on providers, in order」). Nothing
  moves to another provider or another model.
- **A request failure the run's program reports is one of these.** A program that gives a turn up on a
  failed request says so itself — Claude Code through its `StopFailure` hook, with the error's kind — and
  the turn ends `errored` on it at once (`docs/agent-control.md`, 「Telling that a turn's program is
  gone」). The failure reading reads it as the provider's where the provider could not answer: a server
  error (a 5xx status, the kinds `server_error` and `overloaded`, under which Claude Code reports a
  connection lost — the model gateway's cut of a streamed answer whose bytes have stopped among them,
  `docs/own-llm.md`, 「The gateway」), the connection to the provider or the model gateway refused, too
  many requests or a spent window (a 429, `rate_limit`), and a refused credential or balance (a 401 or 402,
  `authentication_failed`, `billing_error`).
- **A request the provider refused for its shape is the work's own:** a 400, 413 or 422, the kinds
  `invalid_request` and `max_output_tokens`. The run ends failed with that error.
- **Inside the Claude Code road the accounts are walked,** never the road left: a run that fails on its account
  moves to the next open account of the same provider and model (`docs/cache-fallback.md`, 「The Claude Code
  road is a list of accounts」).
- **A road a job's turn left for its provider's failure is blocked for a while,** with the failure's kind and
  words: a minute for an outage — a server error, a refused connection, or a failure no kind is read off —,
  five for a refused access — a refused credential or balance —, and fifteen for a limit, or until its reset
  where the failure names one (`policy/roads.toml`, `[blocks]`). A run of a role that names no agent and could
  take any free agent of its role passes a blocked agent's road over while another agent's road can answer; a
  road a run names alone — its agent's, a supervisor's, a pin, a named road — is never passed over for a block.
  A failure of the work itself blocks nothing, and a provider that holds logins is blocked by no failure: a
  failure there is its login's, which its account walk rests. The harness blocks a road the same way through
  `POST /road-blocks` (`docs/api.md`, 「road-blocks and launch-room」). A registration blocked for an outage is
  asked for its health before it is passed over, and one whose engine answers is opened again at once.

## Runs on another's road

- **A job goes out on the road of its agent,** the one its item names or its launch took for its item's model,
  that agent's provider, model and effort (`docs/repositories.md`, 「An agent an item names」). Every job has one
  from its launch, so no repository's pin carries a job.
- **A repository's pinned model** carries a run in the repository that names no agent — its worker's session
  or standby — instead of the role's agents' roads, and that run waits while the pin cannot take it
  (`docs/repositories.md`, 「A pinned model」).
- **A run of an agent that names no road** — the supervisor's double, the `doppelganger` role, as seeded — goes
  out on the asking supervisor's road as it stands when it is asked, and waits while that road cannot take it
  (`docs/role-slots.md`, 「A delegated task」). An agent the owner sets a road goes out on that road instead.
- **A subagent a run's program opens** stands inside that program and goes out on its road; it takes no road
  of its own (`docs/agent-roles.md`, 「Subagents」).
- **A session a caller starts naming a provider and a model** goes out on exactly that, as a named road: the
  provider must carry the line and hold the model, else it cannot answer.

## The facts told

- **A role's runs taking another agent's road is a fact, told once.** Each role's last road is kept beside it,
  so the system acts first and then publishes what it did (`docs/api.md`, 「Events」):
  - `road-moved` as a role's runs go out on another road than the last one's: the role, the provider and model
    it moved from and to — with the account where the provider holds logins — and why each road passed could
    not take it;
  - `road-returned` as its runs go out on its first road again after a move;
  - `road-closed` as no road of its agents can take a run: every road's reason, and what a person does to
    reopen the first one a person can reopen.
- **A supervisor's road never moves,** so none of these is told of a supervisor: its turns that cannot go out
  fail and say why (「Whether a road can answer」 above).
- **The consumer holding the owner's chat tells the owner each once.** A road that reopens on its own is
  never said as the end of the matter: each fact names what the system moved to, or the moment and what a
  person does.

## Models a provider holds

- **Claude Code:** a model the catalogue the Claude CLI keeps names (「The Claude catalogue」 below).
- **Codex, OpenRouter and DeepSeek:** a member of the provider's own road on agency's gateway, each with its
  window, answer ceiling and thinking default (`docs/cache-fallback.md`, 「The Codex road is a member of the
  gateway」 and the sections after it).
- **The registry's provider:** an offered registration, one with an address or naming a card service, each
  with its window, answer ceiling and thinking default (`docs/own-llm.md`, 「Registered models」). A
  registration a supervisor's road, an agent's road or a repository's pin names is not removed, and the
  refusal names each row that names it.
- **A catalogue that cannot be read at the moment a road is written** keeps the model rather than refusing
  it, since a catalogue that cannot be read says nothing against the name; one that reads and does not name
  the model refuses it.

## Effort and thinking

- **The levels are one scale:** `low`, `medium`, `high`, `xhigh` and `max`. A road names a level once, and
  the provider turns it into its own word.
- **Each provider answers for itself,** declared beside its harness, never in a table of providers: which
  levels a run of one model takes, whether the thinking switch reaches its runs, and the effort word a run
  goes out with.

  | Provider | Levels | Thinking off | Thinking on |
  |---|---|---|---|
  | Claude Code | those the model's catalogue entry names, every level where it names none | `MAX_THINKING_TOKENS=0` | the line's own |
  | Codex | `low` to `max`, as the backend's effort through Claude Code's effort word | effort `low` | the line's own |
  | OpenRouter | `low` to `xhigh`, as the backend's effort through Claude Code's effort word | reasoning off | the line's own |
  | DeepSeek | `low` to `max`, as the backend's effort through Claude Code's effort word | effort `none`, reasoning off | the model's own |
  | Registered models | none | effort `no-thinking` | effort `thinking` |

- **A level the road's model does not take is refused as the road is written,** and a road written before
  the model's catalogue named its levels runs at the provider's default for that run.
- **What a run went out with is what its record, its session's key and its prefix digest carry,** so a
  changed model or effort selects another session (`docs/sessions.md`, 「A role's session」).

## The Claude catalogue

- **The list is not written in this repository.** Which Claude models an entry may name is read from the
  catalogue the Claude Code CLI keeps under the configuration directory of the login a run would go out on,
  which the CLI fetches and refreshes on its own as it is used. A model the CLI has begun offering can be
  chosen here without a deployment.
- **The efforts are read from the same entry.** A model's catalogue entry names the efforts it accepts, and
  a road of that model is validated against those words.
- **Absence of evidence refuses nothing.** A model whose entry names no effort and a model the catalogue does
  not name at all both take whichever effort words the harness itself declares.
- **In service and superseded.** The catalogue separates the generation it currently serves from the ones it
  has superseded, and both are offered, marked as which.
- **A stale read is still an answer:** past its stale mark it is served as it stands, and the answer says it
  is the last read and when it was taken.
- **A catalogue that cannot be read refuses,** as Codex, OpenRouter and the own LLM services refuse: no file
  where one is expected, nothing that parses, or a catalogue naming no model is a listing failure rather
  than an empty list.
- **It is read from the login the account walk would take first,** and from the machine's configuration
  directory where that login keeps none of its own.

## The owner's switch on a provider

- **Each provider carries an on/off the owner throws** (`docs/api.md`, 「providers, models and model
  files」). A provider that is off cannot answer now, so no run goes out on a road of it and its sessions are
  not warmed; turning it on again lets every road on it answer again.
- **The last provider left on is not turned off,** since nothing would answer.
- **Outside the console,** `bmo-agency agent --provider-on <provider>` and `--provider-off <provider>`.
