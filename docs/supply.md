# Supply

Who pays for a run, and on which credential it goes out. A road names the provider and the model a run goes
out on (`docs/provider-order.md`); a supply source is an axis inside that road: the same provider and the same
model, on a credential and at a payer of its own. A run never changes provider or model for its supply. Agency
measures what a run costs in US dollars at the provider's list price and knows nothing of plans or credits: a
consumer above it answers, through the supply hook, which sources a run may take and whether a source that the
platform pays for takes it.

## The sources

- **Personal supply:** the person's own credential, a Claude login or an API key of theirs, paid by them to the
  provider directly. It runs out when the provider refuses it: a window at its ceiling, a key refused or a
  balance spent.
- **A platform source:** the platform's credential, its Claude logins lent for the purpose and its gateway
  members, paid as the consumer answers: the plan's included supply, an organisation's credits or the person's
  own credits are each a platform source of the consumer's naming. Every platform source goes out on the same
  credentials, so moving between them changes only who pays.
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
| The registry's provider | none | none: a registration is the platform's |

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

## The lendable accounts

- **A platform Claude account goes out for another person's run only where it is marked lendable.** None is
  marked by default; the owner marks one. An account of a person is never lent.
- **Whether the rule holds while no supply hook is registered** is the switch `lendable_without_hook` in
  `policy/supply.toml`. On, a person's Claude run with no hook registered goes out on their own login or a
  lendable account, and on no other; off, it walks the platform's accounts as every run did before the rule.
- **An `api-key` account of the platform is never sent to a person's node,** as an endpoint account is not: its
  key stays on this machine.

## Choosing a source as a run starts

- **A run whose account is a person other than the administrator** asks the supply hook for its order as it
  starts (「The supply hook」), then walks the order within its road:
  - **personal supply** is taken where the person holds an open credential for the road's provider: a Claude
    login or `api-key` account that can answer now for a Claude Code road, an open key of the road for a gateway
    road, none for the registry's provider. It goes out on that credential.
  - **a platform source** is taken where the hook accepts a reservation for it first (「Reservations」). It goes
    out on the platform's credential: a lendable Claude account, or the gateway's platform member under the
    person's call key.
  - **with nothing left,** the run is stopped (「Running out mid-turn」, the same sentence).
- **The order is asked afresh for every run,** so a personal login that reopens is taken again from the next
  run, and the platform's rule of using the login in use to its end does not hold for a person's run.
- **With no hook registered,** the order is personal supply, then the platform unreserved: the run's cost is
  recorded, and nothing is reserved.
- **With a hook registered that does not answer,** the order is personal supply alone: a person's run never goes
  out on the platform's supply without a reservation.
- **What the run went out on is its source:** `personal`, the platform source the hook named, or `platform` for
  a run that reserved nothing. It is kept on the run and its job (「The ledger」).

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
| sources and reservations | the hook, the walk at a run's start, reservations, extension, the settlement outbox, the capture after a restart | — |
| running out | the personal pass-over, the platform hand-over, the grace unit, the job's wait and the supervisor's sentence | — |
