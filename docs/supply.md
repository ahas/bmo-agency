# Supply

Who pays for a run, and on which credential it goes out. A road names the provider and the model a run goes
out on (`docs/provider-order.md`); a supply source is an axis inside that road: the same provider and the same
model, on a credential and at a payer of its own. A run never changes provider or model for its supply. Agency
measures what a run costs in US dollars at the provider's list price and knows nothing of plans or credits: a
consumer above it answers, through the supply hook, which sources a run may take and whether a source that the
platform pays for takes it.

## The sources

- **Personal supply:** the person's own credential, a Claude login or an API key of theirs, paid by them to the
  provider directly. It runs out when the provider refuses it: a window full, a key refused or a
  balance spent.
- **Organisation supply:** the credential of an organisation unit on the person's path, the keys and Claude
  accounts the unit registers (「Organisation supply」), paid by the unit to the provider and counted as
  organisation credits by the consumer. Its source word is `organisation`.
- **A platform source:** the platform's credential, its Claude logins lent for the purpose and the gateway
  members and registrations no person and no organisation owns (「Owned supply」), paid as the consumer answers:
  the plan's included supply, an organisation's credits or the person's own credits are each a platform source
  of the consumer's naming. Every platform source goes out on the same credentials, so moving between them
  changes only who pays.
- **Stop:** nothing left. A run waits or ends saying so (「Running out mid-turn」).
- **The owner's and the deployment's own runs** — a run of no account or of the administrator's account
  (`docs/account-isolation.md`, 「The scope」) — go out as every run went before there were sources: on the
  platform's credentials, their cost recorded and reserved nowhere. Every rule below holds for the runs of the
  other accounts alone.

## What a person brings

| Road | A login | A key |
|---|---|---|
| Claude Code | a subscription login of the person's own, signed in on their node | an Anthropic API key, an account of the kind `api-key` |
| Codex | none | an OpenAI key, a personal member of the gateway |
| DeepSeek | none | a DeepSeek key, a personal member of the gateway |
| OpenRouter | none | an OpenRouter key, a personal member of the gateway |
| The registry's provider | none | none: a registration is the platform's, or its owner's (「Owned supply」) |

- **A key is the person's secret in the console's secret store,** the file `users/<user>/<name>` under the data
  root's `env/`; agency is handed the name alone and reads the file only where a run or the gateway hands its key
  on. Its value never enters the store, a log line, a response or a command line.
- **A Claude login or an `api-key` account of a person is a Claude account whose `user_id` is that person**
  (`docs/cache-fallback.md`, 「The Claude Code road is a list of accounts」). A login is signed in on the person's
  node (「Signing a person's login in」 below) and kept in a configuration directory there.
- **A gateway key of a person is a row of personal supply:** the person, the road it serves (`codex`,
  `deepseek` or `openrouter`), the secret's name, its place among that person's keys of the road, an on/off and
  the moment it was found refused. The gateway serves it as a personal member of its own (`docs/own-llm.md`,
  「Personal members」).
- **Several on one road are taken in their order,** as a person's logins and keys were placed; one refused waits
  for its person, and the next is taken.
- **Only the person makes, sees, changes and removes their own,** through the routes the console calls with the
  person's own headers (`docs/api.md`, 「Who a caller acts for」 and 「personal supply」). The administrator sees
  of another person's supply only which roads it serves, and the Claude accounts list the administrator reads
  holds the platform's accounts alone.
- **A person off-boarded has their personal supply switched off:** each of their Claude accounts and keys is
  turned off and their gateway call key is revoked. Their secrets stay sealed where the console keeps them.

### Signing a person's login in

- **The sign-in runs on the person's node,** in the node's herdr session as the node's own user: the CLI's
  subscription sign-in with a configuration directory under the node's data root, named after the account. The
  address it prints and the code the person pastes travel as they do for the owner's (`docs/cache-fallback.md`,
  「Signing an account in from the console」), and the login it writes stays on the node.
- **The account names that directory and its node.** A run of the person on that node is handed the directory
  as its configuration home and no token; the account goes out nowhere else, and its usage windows are read off
  its turns' answers as every account's are.
- **The pane is read as herdr joins its soft-wrapped lines,** so an address wider than the pane reads whole, and
  the login is taken once the CLI says it took (`Login successful`), refused once it says it refused, and given up
  at the policy's bound otherwise.
- **Its credentials file stands on the node, not here,** so a login of a person's found waiting for them is
  opened again by their next sign-in, not by a file this machine reads.
- **A person with no node attached cannot sign in yet:** the start is refused 409 `node-not-attached`.

## The sources a person's chain leaves open

- **Three settings of the account service's delegation chain** (the web's `docs/accounts.md`, 「The delegation
  chain」), each a flag that narrows down the chain, so a link only turns a source off for everyone below it and
  never turns one on that a link above turned off:
  - `supply.platform`: whether the person may run on the platform's supply. On by default; held at the
    platform's link, a unit's and a person's.
  - `supply.organisation`: whether the person may run on their organisation's own supply. Off by default, and
    a unit's word alone never opens it: it stands open only where the platform's own link says it is open for
    the person — its value, or a rule naming them, a group of theirs or a unit on their path, as the platform
    administrator opens it for one company — and no link below closes it. Held at the platform's link and a
    unit's.
  - `supply.personal`: whether the person may run on their own login or key. On by default; held at the
    platform's link, a unit's and a person's.
  A head writes a unit's link and a person's, with rules for a person, a group or a unit; a lock keeps the
  links below from narrowing further. Agency declares the three as its server stands, with its other kinds
  (`docs/prompt-layers.md`, 「The settings agency declares」).
- **Resolved for the person every run serves,** afresh as the run starts, beside the hook's order. Where the
  account service does not answer, what this process last read for the person stands, else the defaults; where
  it says nothing — none named, a person it does not hold — the defaults stand. The defaults leave every order
  as the hook answers it.
- **The order a run walks keeps a fixed shape:** personal supply, the organisation's, the platform's.
  - personal supply is left out of the hook's order where `supply.personal` is off;
  - every platform source is left out where `supply.platform` is off;
  - `organisation` is placed before every platform source — so after personal supply wherever the order puts
    that first — where `supply.organisation` stands open on a unit of the person's path; a hook that names it
    itself is not followed.
  Nothing else of the hook's order moves, so a person's own reordering of the platform's sources stands.
- **`GET /supply-sources`** answers the signed-in person what their chain leaves open — `personal`, `platform`,
  `organisation` — whether a unit of their path holds supply of its own that is on, and `personalOnly`, whether
  their own login or key is all that can carry their runs, which the console reads to ask a new person for one
  (the web's `docs/web.md`, 「The join page」).

## Organisation supply

- **An organisation unit owns supply as a person does:** a key for a gateway road, a row of the store's
  `personal_supply` naming the unit in place of a person, its secret in the unit's space of the console's secret
  store, `units/<unit>/<name>` under the data root's `env/`; and a Claude account naming the unit in place of a
  person, an `api-key` account its heads register on its Anthropic API key, or any account of no person's the
  owner gives the unit (`docs/api.md`, 「accounts, rest and usage」).
- **Only the people of its path take it.** A run of a person whose chain leaves organisation supply open takes
  the supply of the nearest unit of their path that holds an open credential for the road's provider: a Claude
  account of the unit's that can answer now on Claude Code, an open key of the unit's on a gateway road; the
  registry's provider has none. Nobody else's run goes out on it, the platform's own runs among them.
- **It is reserved through the hook as a platform source is,** under the source `organisation`, the
  reservation naming the unit, so the consumer counts its cost and the gateway lets the person's call key reach
  that unit's members while it is open (`docs/own-llm.md`, 「Personal members」). A Claude account of the unit's
  goes out on the person's node with the run, as a person's `api-key` account does.
- **Running out:** a key of the unit's its provider refuses waits for the unit's heads, as a person's waits for
  them, and the run goes on to the next source of its order in a new session; a reservation of it whose extension
  is refused hands the run to no platform source, since that goes out on another credential, and the run takes
  its unit of grace and ends there. The next run asks its order afresh.
- **Its heads manage it** through `/organisation-supply/{unit}` (`docs/api.md`, 「accounts, rest and usage」): a
  person is taken as a head where the chain answers them the unit's link of `supply.organisation`, which it
  answers its writers alone, and a key is added only while organisation supply stands open for the unit.

## The lendable accounts

- **A platform Claude account goes out for another person's run only where it is marked lendable.** None is
  marked by default; the owner marks one. An account of a person, the administrator's among them, is never
  lent, and marking one lendable is refused.
- **An `api-key` account of the platform is never sent to a person's node,** as an endpoint account is not: its
  key stays on this machine.

## Owned supply

- **Each credential a road goes out on belongs to a person, to an organisation unit or to no one,** and the
  record of the credential says which:
  - a Claude account by its `userId`, or its `unitId` (`docs/cache-fallback.md`, 「The Claude Code road is a list
    of accounts」);
  - a gateway member by the `account` the members file names for it (`docs/own-llm.md`, 「The gateway's
    members」);
  - a registration by its own `account` (`docs/own-llm.md`, 「What a registration holds」).
- **What no person and no organisation owns is the platform's,** and is the only supply a platform source goes
  out on. What a unit owns is its organisation supply alone (「Organisation supply」).
- **What a person owns is that person's personal supply:** their run takes it as personal supply, and nobody
  else's run goes out on it, as a platform source or any other. The gateway holds to the same line on every
  request (`docs/own-llm.md`, 「The gateway」).
- **What the administrator owns is the owner's own supply,** and the deployment's own runs go out on it as on
  the platform's: a run of no account or of the administrator's walks the administrator's Claude accounts with
  the platform's, and is sent on the gateway's own key, which reaches the administrator's members and
  registrations. Another person's run never does, and a deployment run naming a model another person owns is
  refused by the gateway as any stranger's request is.
- **A model a person owns that refuses their run** is passed over as a refused personal key is (「Running out
  mid-turn」): the run goes out on the next source of its order after personal supply, and no key of theirs is
  marked refused for it, since none was used. A job that finds no source left waits for supply, and on the next
  beat of its wait asks the model it owns again.
- **Who owns a member or a registration is set by the owner** through its route (`docs/api.md`, 「providers,
  models and model files」), and taken away with null, which gives it back to the platform.

## Choosing a source as a run starts

- **A run whose account is a person other than the administrator** asks the supply hook for its order as it
  starts (「The supply hook」), shapes it by what the person's chain leaves open (「The sources a person's chain
  leaves open」), then walks the order within its road:
  - **personal supply** is taken where the person holds an open credential for the road's provider: a Claude
    login or `api-key` account that can answer now for a Claude Code road, an open key of the road or a member
    of the road they own for a gateway road, a registration they own for the registry's provider. It goes out
    on that credential.
  - **organisation supply** is taken where a unit of the person's path that the chain leaves open holds an open
    credential for the road and the hook accepts a reservation for it first. It goes out on that credential.
  - **a platform source** is taken where the hook accepts a reservation for it first (「Reservations」). It goes
    out on the platform's credential: a lendable Claude account, the gateway's member no person owns, or a
    registration no person owns, under the person's call key. A model another person owns offers no platform
    source.
  - **with nothing left,** the run is stopped (「Running out mid-turn」, the same sentence).
- **The order is asked afresh for every run,** so a personal login that reopens is taken again from the next
  run, and the platform's rule of using the login in use to its end does not hold for a person's run.
- **With no hook registered, or one that does not answer,** the order is personal supply alone: the platform's
  supply is off for other people until the consumer answers for it, and a person's run never goes out on it
  without a reservation.
- **What the run went out on is its source:** `personal`, `organisation`, the platform source the hook named, or
  `platform` for a run that reserved nothing. It is kept on the run and its job (「The ledger」).

## The supply hook

- **The point is `supply`,** registered by the consumer with `PUT /hooks/supply` (`docs/api.md`, 「hooks」 and
  「Hook points」). Its registration outlives agency's restart: its address is kept in the store's settings and
  its token in a file under the data root's `env/`, never in the store.
- **Five asks,** each posted as `{ point, ask }` with the ask's `op`, answered under `answer`:
  - `order`: the run, its account, its provider and model, whether it is a job's or a supervisor's turn, and
    the job, the supervisor and the room where they stand. Answered with `sources`, in order, each `personal`
    or a platform source the consumer names.
  - `reserve`: the run, the step, the account, the source and the idempotency key. Answered with whether it is
    `accepted`, the `reservation` it opened, its `amountUsd` and `expiresAt`. The size is the consumer's.
  - `extend`: the run, the step, the reservation, the source, what the run has spent so far as `spentUsd` and
    the key. Answered with `accepted`, the reservation's whole `amountUsd` now and its `expiresAt`.
  - `capture`: the run, the step, the reservation, the source, the actual cost as `costUsd` and the key.
    Answered with `taken`. It may exceed the reservation; how far is the consumer's, at most one reservation
    unit. A reservation the consumer already let go, past its expiry, is taken as a charge of that amount.
  - `release`: the run, the step, the reservation and the key. Answered with `taken`.
- **Every ask that moves money carries its idempotency key,** `<run>:<step>:<op>`, the step a number that
  rises with each reservation, extension, capture and release of the run, so an ask sent again is taken once.
- **Agency never calls the account service by its own address:** the hook is the consumer's, and its default
  does nothing.

## Reservations

- **A platform source is reserved before the run goes out on it,** at the size the hook answers, and kept in the
  store: the run, the step, the account, the source, the reservation, its amount, what it covers so far, its
  expiry and its state.
- **The running cost is read while the run goes,** on the meter's beat (`policy/supply.toml`,
  `meter_seconds`): a gateway road's from the gateway's request records of the run's program, by the run name its
  launch sends, since the run began, a Claude Code road's from the status line's cumulative cost of the turn.
- **At 80% of the amount, or as its expiry comes within two beats, the reservation is extended** (`extend_at`,
  the fraction). An accepted reservation names its amount; one that names none is taken as refused. An extension refused hands the run over (「Running out mid-turn」).
- **As the run ends,** its reservation is captured with the actual cost and the rest released.
- **Capture and release go through an outbox,** the store's `supply_settlements`, one row an ask keyed by its
  idempotency key, sent until the hook takes it, as it is written and then every few seconds, the wait between
  tries doubling up to the policy's ceiling. A row the hook takes is let go.
- **A reservation that expired while agency was down** is captured as agency starts, through the same outbox,
  with the cost the store last recorded it covered: the meter writes what the run has spent on each beat.

## Running out mid-turn

- **Personal supply the provider refuses** — a 429, a 401 or a 402, read off the run's ending by the one failure
  reading (`docs/provider-order.md`, 「A road that fails」) — is passed over as a refused Claude account is: the
  key or login waits for its person, who is told once, and the same run goes out on the next source of its
  order, in a new session that opens with the line that it continues the work already in the checkout. A failure
  of personal supply blocks no road.
- **A platform source whose extension is refused** hands the run to the next platform source of the order that
  accepts a reservation. The credential does not change, so the session goes on; what it covered so far is
  captured to the source it left, and the next pays from there.
- **Where none accepts,** the run goes on for one more reservation unit, the amount its last reservation
  answered, and then its turn is interrupted through the session contract:
  - a job waits for supply, as it waits for a road, holding no slot, and asks its order again on each beat of
    its wait (`policy/supply.toml`, `wait_seconds`); it goes out once a source takes it;
  - a supervisor's turn ends with one sentence: the source that ran out, when it reopens where the hook names a
    moment, and what the person can do — top up, or open their supply settings.

## The ledger

- **A run's and a job's cost and source are kept:** `cost_micros`, the cost in millionths of a US dollar summed
  over the models that answered, and `supply_source`. A Claude Code turn's cost is the status line's
  `total_cost_usd` the turn added; a gateway road's is the sum of its request records.
- **The gateway records every request it carries:** the account its call key is, none for the platform's key,
  the model, the run name and the program's session its request named, its tokens and LiteLLM's cost (`docs/own-llm.md`, 「What a request
  costs」).
- **Usage groups by person and by source:** `GET /usage` with `groupBy` `account` or `supply` (`docs/api.md`,
  「accounts, rest and usage」).

## The facts told

- **`supply.changed`** carries a person's run moving to another source (`supply-moved`) or finding none left
  (`supply-closed`), published once each after agency has acted (`docs/api.md`, 「Events」).
- **The person's own supervisor room is told once,** through the receiver's instruction route by the outbox:
  the room the run answers, for its supervisor. The owner is not told.

## What stands and what is left

| Part | Stands | Left |
|---|---|---|
| call keys and request records | a call key per account on its nodes, the platform key here alone, each request recorded with its account, model, run and cost | — |
| personal supply | personal members, `api-key` accounts, a person's own accounts and keys through their headers, the sign-in on their node, the off-boarding switch | — |
| owned supply | a Claude account's, a gateway member's and a registration's owner, the walk and the gateway holding to it, the administrator's owned supply carrying the deployment's own runs | — |
| organisation supply | the three source settings on the chain, the order's shape, a unit's keys and Claude accounts, the walk to the nearest unit, its reservation and the gateway's members of it | a Claude subscription login of a unit's own, signed in from its screen |
| sources and reservations | the hook, the walk at a run's start, reservations, extension, the settlement outbox, the capture after a restart | — |
| running out | the personal pass-over, the platform hand-over, the grace unit, the job's wait and the supervisor's sentence | — |
