# Provider order

How a run gets its provider and its model. Every supervisor and every role carries a provider order of
its own; a run goes out on the first provider of its order that has quota and can answer now, moves down
the order on a limit or a failure, and comes back to the top at its next run. There is no order shared by
everything, no level of work standing between a role and its model, and no limit on how many runs go out on
one provider at once. How the Claude Code provider walks its own accounts is `docs/cache-fallback.md`; how
quota is read is `docs/usage-windows.md`.

## What an order is

- **An order is a list of providers, each once,** held on its supervisor's or its role's row
  (`docs/supervisors.md`, `docs/agent-roles.md`). Its first entry is where the row's runs go while it has
  quota; every later entry is where they go when the ones above it have none.
- **Each entry names the provider and what the row runs on there:**
  - the model, one the provider holds (「Models a provider holds」 below);
  - the effort, a level of the one scale `low`, `medium`, `high`, `xhigh` and `max`, or none for the
    provider's own default; the provider turns a level into its own word (「Effort and thinking」 below);
  - whether the run thinks, where the provider takes the switch;
  - on the registry's provider, the answer ceiling, or none for the registration's own.
- **An order holds a provider once, never a provider twice with two models.** A provider holds one quota,
  so a second entry of it could only ever be reached with that quota already spent.
- **Beside the entries, the quota share:** the remaining quota at or below which a provider is passed over,
  a fraction of its quota, 1% where the row sets none.
- **An order names only providers agency carries and whose program carries the row's line** (the chat line
  for a supervisor, the role's own line for a role); a write naming another is refused with the provider and
  why. An order is never empty.

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
- **What a provider has left is read afresh as each run starts,** never kept on the order.

## Choosing the provider

- **A run goes out on the first provider of its order that has quota above the share and can answer now.**
  Can answer now is the provider's own answer (`docs/cache-fallback.md`, 「Availability: whether a provider
  can answer now」): the owner's switch on it is on, its credential stands on this machine, and, for a
  provider that holds logins, one of them is open.
- **It goes out on the model, the effort and the thinking its entry names there.**
- **That provider is the row's active provider.** Every run of a role goes out on the role's active provider
  at that moment; a role's runs never spread over several providers by count, since no provider carries a
  concurrency limit of its own and the role's cap is the only bound (`docs/role-slots.md`).
- **The choice is made as the run starts and kept for the run.** A run already going finishes where it went
  out; a change of the order, the share or the quota holds from the next run.

## Falling over and coming back

- **A run that fails for its provider moves to the next provider of the order.** A failure for the
  provider is a limit met, a quota or balance refused, a credential refused, the last open login closing
  under it, or the provider found unable to answer as the turn ends; it is read off the run's ending by the
  one failure reading every run asks (`docs/cache-fallback.md`, 「Automatic walks only on providers, in
  order」). A failure of the work itself moves nothing.
- **A job moved this way is the same job:** launched again on the next provider, with the same item and
  lease, in the same checkout, at most once on each provider in one job, its prompt opened with a line
  saying it continues work already in the checkout. It keeps each provider it went out on and why it moved,
  and its report says so.
- **The next run begins at the top of the order again.** A provider passed over for its quota or a failure
  carries the next run the moment it can, so a row returns to its first provider at its next run, never
  in the middle of one.
- **A blocked road stays passed over for a while:** a provider and model the harness blocks after it did not
  answer is passed over by the next runs — a minute for an outage, five for a refused access, fifteen or
  until its reset for a limit (`docs/api.md`, 「road-blocks and launch-room」). A block is the harness's memory
  of a failure, not a reason a provider cannot answer.

## When every provider is out

- **A run none of whose providers has quota waits; it is never refused.** It is held with the reason — each
  provider's, and the moment the first of them reopens where a window names one — and goes out as soon as a
  provider of its order has quota above the share again, on the first such provider in the order.
- **A waiting run holds no slot.** It waits in its role's line for its slot and for its provider together
  (`docs/role-slots.md`, 「The line」), so a run waiting for quota never keeps a slot from a run that could go.
- **A supervisor's turn waits the same way,** its room told by the receiver, which reads the wait off the
  turn's state.
- **What a person must do is said where only a person can reopen a provider:** an account waiting for a
  person, with the sign-in command and that account's own configuration directory; a spent balance, where
  its provider's member says it is topped up. A provider that only waits for a window names the moment it
  reopens.

## Runs that do not walk their own role's order

- **A repository's pinned model** carries every job in the repository instead of the role's order, and its
  work waits while the pin has no quota or cannot answer; nothing falls back (`docs/repositories.md`, 「A
  pinned model」).
- **A delegated task as the supervisor's double** goes out on the supervisor's own provider, model and
  effort as they stand when it is asked, and waits, never moves, while that provider has no quota
  (`docs/role-slots.md`, 「A delegated task」).
- **A native co-worker** stands inside its caller's program, so it goes out on the provider that program
  stands on, at the model and effort the co-worker role's order names for that provider; where the role's
  order holds no entry for that provider, the call is refused and the caller does that part itself
  (`docs/role-slots.md`, 「Native co-worker slots」).
- **A session a caller starts naming a provider and a model** goes out on exactly that, as a named road: the
  provider must carry the line and hold the model, else it cannot answer, and the run never moves off it.

## The facts told

- **A move is a fact, told once.** Each supervisor's and each role's last provider is kept beside it, so the
  system acts first and then publishes what it did (`docs/api.md`, 「Events」):
  - `road-moved` as a row's runs first go out on a provider other than its first, or on another one than the
    last move's: the supervisor or role, the provider and model it moved from and to — with the account
    where the provider holds logins — and why each provider passed could not take it;
  - `road-returned` as its runs go out on its first provider again after a move;
  - `road-closed` as no provider of its order has quota or can answer: every provider's reason, and what a
    person does to reopen the first one a person can reopen.
- **The consumer holding the owner's chat tells the owner each once.** A provider that reopens on its own is
  never said as the end of the matter: each fact names what the system moved to, or the moment and what a
  person does.

## Models a provider holds

- **Claude Code:** a model the catalogue the Claude CLI keeps names (「The Claude catalogue」 below).
- **Codex, OpenRouter and DeepSeek:** a member of the provider's own road on agency's gateway, each with its
  window, answer ceiling and thinking default (`docs/cache-fallback.md`, 「The Codex road is a member of the
  gateway」 and the sections after it).
- **The registry's provider:** an offered registration, one with an address or naming a card service
  (`docs/own-llm.md`, 「Registered models」). A registration an order or a repository's pin names is not
  removed, and the refusal names each row that names it.
- **A catalogue that cannot be read at the moment an order is written** keeps the model rather than refusing
  it, since a catalogue that cannot be read says nothing against the name; one that reads and does not name
  the model refuses it.

## Effort and thinking

- **The levels are one scale:** `low`, `medium`, `high`, `xhigh` and `max`. An entry names a level once, and
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

- **A level the entry's model does not take is refused as the entry is written,** and an entry written before
  the model's catalogue named its levels runs at the provider's default for that run.
- **What a run went out with is what its record, its session's key and its prefix digest carry,** so a
  changed model or effort selects another session (`docs/sessions.md`, 「A role's session」).

## The Claude catalogue

- **The list is not written in this repository.** Which Claude models an entry may name is read from the
  catalogue the Claude Code CLI keeps under the configuration directory of the login a run would go out on,
  which the CLI fetches and refreshes on its own as it is used. A model the CLI has begun offering can be
  chosen here without a deployment.
- **The efforts are read from the same entry.** A model's catalogue entry names the efforts it accepts, and
  an entry of that model is validated against those words.
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
  files」). A provider that is off cannot answer now, so every order passes it over and its sessions are not
  warmed; turning it on again returns it to its place in every order that names it.
- **The last provider left on is not turned off,** since nothing would answer.
- **Outside the console,** `bmo-agency agent --provider-on <provider>` and `--provider-off <provider>`.
