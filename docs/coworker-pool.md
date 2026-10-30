# The common co-worker pool

Agency keeps one pool through which every co-worker takes a place before its model process launches,
bounds how many of each tier run at once, lends and reclaims leases, holds reservations and warms the
prefixes they name, and stands the checkouts that branch co-workers work in. What a co-work work
order plans and builds is the job unit's; the pool itself, with its places, leases, reservations,
warming and standing checkouts, is agency's.

## Who takes a place

- **Who takes a place first:** co-work branches, branches the search and browse searchers open, and
  the co-workers a job worker line calls with Claude Code's native `Agent` (also called `Task`) all
  take a place from the same store pool before the model process launches.
- A co-work work order's builder turn also launches a job after taking a place in the same pool.
- **Branches:** the caller gives a role and instruction to the common dispatcher, which reads the
  provider order and the `role:<name>` tier assignment and picks the actual `(provider, model,
  effort)` independently of the parent. A native `Agent` call is the exception: its provider is the
  parent's where the parent stands on a provider that lends co-workers, and the pool's order decides
  for every other branch.
- **A branch's source and work key are the caller's.** A seat is asked under the source label its
  caller sends — `cowork` for a co-work branch, a research run's own label for its branches — and a
  builder turn's place under `turn`; the label is kept as sent and only `turn` is a builder turn's. A
  seat is resolved under the work key its caller names, such as a research line's, and under
  `role:<name>` where it names none. The key a request or a reservation was resolved under is the one
  kept on it, and every later reading of it — the request asked again, a reservation's seats — reads
  that key rather than building it again.
- **A worker line's native `Agent`:** the child runs inside the worker's own program, so the worker
  settings' hook puts the request into the provider that program stands on and the tier the
  `role:<name>` mapping gives on it before the call, and sends the call only after taking a place
  (「Native co-worker places」 below).
- **Refused:** a type not in a stored role (a CLI default type like `general-purpose`) follows the
  parent model, and a call re-invoked inside a co-worker spreads one more level outside the pool, so
  both are refused. A native call on a line that is not a worker line is refused too.
- **Codex collaboration events** are only observation data of children the provider already launched,
  not a run road of this pool, and are not used to assign new work.

## Concurrency limits

- **Co-workers are borrowed at Reliable, Fine, Light or Simple.** Supervisor, the name a stored key of the
  tier tables, is the chat agents' tier, and Frontier the job worker's. The builder stage is the
  worker itself, so neither is a tier a co-worker is borrowed at: the pool refuses a child request or a
  reservation at either one, and a plan's side task naming either one falls back to its role's tier.
- **The limit** is the `(provider, tier)` row of the co-worker pool tier-limit table, one row for
  every provider and co-worker tier. Only a provider turned on that lends co-workers is counted,
  admitted on and shown (`docs/model-tiers.md`, 「Common co-worker capacity」). A native call's seat
  counts against the row of the provider its parent stands on, so the registry's rows are what hold
  the native co-workers of a worker on a registration.
- **The limit attaches to the tier** regardless of which model the tier currently resolves to, so
  changing a tier mapping or a role's tier does not lose the limit row, and two tiers resolving to one
  model still count their own limits.
- There is no operation to delete a row, and a limit of zero blocks that tier on that provider.
- **Builder turns have no limit of their own.** A worker's turn waits only on its checkout, so as many
  builder turns run at once as there are workers.
- Role, tier assignment, mapping and capacity are all store rows, so a value the owner changed stays
  across restart and deployment, and the code seed does not revive it.

## Requests and leases

- A request goes into the pool's request table before the model launches.
- Within one provider and tier, only a waiting request with a smaller global increasing sequence can
  take a place, and one immediate transaction checks the current active count plus the reserved places
  and the FIFO head, and grants a lease (「Reserving places and keepalive」 below).
- A lease preserves a hard-to-guess token, a stable owner and request key, the role and work key, the
  chosen provider, model, effort and tier, the capacity then, the generation and the time.
- Changing settings does not change this snapshot for an already-waiting or active request; it reads
  the new settings from the next request.
- **Release and cancel** can be done only by the holder of the token, and finish once however many
  times called.
  - They finish at once when success, failure, a launch refusal or a cancel is confirmed, and after a
    restart reclaim only when there is evidence that the task, job or execution record and the real
    unit and pane have all ended.
  - An active lease is not taken away for age alone. So the same place cannot be given to two runs on
    the clock while a process is still alive.
  - **A lease bound to its launched child is given back once that child has ended,** whatever said or
    did not say so: from the server's start and every minute after, a lease bound ten minutes or more
    ago whose child no systemd unit, herdr pane or open execution holds any longer is finished as
    ended. Where what runs on the machine cannot be asked, nothing is given back on that pass. A native
    call's lease is read by its caller's process instead (「Native co-worker places」 below).

## Prior requests and yielding requests

- **A request is a prior request or a yielding request.** The caller decides which, and the pool does
  not read the reason.
- **Every request and reserved place now in this pool is a prior request** — yielding is a rule the
  pool holds, a place used when there is something that can defer its own work behind another's.
- **A prior request's turn** is the smallest sequence among the prior requests waiting on the same
  provider and tier. A yielding request is skipped as if not there in that line — even having queued
  first, it does not block a prior request's turn.
- **A yielding request enters only** when no prior request is waiting on the same provider and tier,
  active plus reserved is below the limit, and it is the FIFO head among yielding requests. A reserved
  place waiting to become reserved is a prior request too, so a yielding request does not enter while
  such a place exists.
- **An already-active yielding request is not taken away.** A prior request waits for its lease to
  end. So a prior request waits on a yielding request only until an already-entered one ends, and never
  stands behind a yielding request that entered while it was waiting.
- **The caller can raise a waiting yielding request to a prior request.** The raised request keeps its
  sequence and gets its place in the prior-request line.
- **A place key:** a request may hold one. Only one active request per key exists, so while a key is in
  use, that key's request does not enter even in its turn.
  - A request blocked by a key does not block another request behind it — one worker's turn waiting
    does not stall every branch of the same tier.
  - If a prior request of the same key is waiting on any provider and tier, that key's yielding request
    does not enter. The key too is text the pool does not read the meaning of.
- **A builder turn takes a place** keyed by the worker checkout directory, on the provider launching
  that turn, and records the `builder` role's tier as the model it runs. A checkout has one
  session-continuing place and one job lock, so the key too is the checkout.
  - A builder turn is the worker's own turn, not a borrowed co-worker: it stands in no tier's line and
    counts against no tier's limit. It waits only while another turn holds its checkout, the earliest
    waiting turn of that checkout first.
  - The runner takes a place before launching the job, and while waiting for a place the unit does not
    launch. The place is released when the job row closes and its unit is no longer alive, and the
    pool's own pass gives it back once the unit has ended where that release never came.
  - A provider switch within one builder turn uses the place it got — a worker job's provider fallback
    is not subject to the branch limit (「Native co-worker places」 below).
  - The checkout's job lock stands as is: the key sets what launches first, and the lock keeps what
    launched from overlapping in one checkout. An ordinary job does not go through the pool.

## A project's own session

- **A project's own turn asks for its earlier session through its place.** The request that stands for a
  project's own work is the builder turn, whose place key is the project's checkout. The worker standing
  in that checkout is the project's, and the session it holds on the model the turn resolves to and the
  login the job line is walked on first is the project's session.
- **The pool carries it on the request** where both of these hold, and only these:
  - the provider's declared cache lifetime for that model has not passed since the session last answered;
  - the session's last answered turn stayed under the model's context limit.
  A provider that declares no cache lifetime leaves a held session no clock to outlive, so it is warm.
- **Where either fails,** the request carries no session and the turn opens a new one as it does today,
  carrying the note the replacement rules already leave (`docs/sessions.md`, 「Session replacement」). The
  session that failed is dropped with its record by the rules those sessions already stand on.
- **A borrowed child's seat carries no session of its own.** A co-work or research branch keeps no session,
  and a native call keeps its parent's, so their seats are unchanged.
- **Reuse changes which session a seat runs in, never how many run.** A request's session is no part of
  admission: the tier's limit, the line, the place and the yielding rule count exactly as they did, and a
  builder turn still stands in no tier's line.

## Reserving places and keepalive

- **Reserving:** whoever will soon launch co-workers reserves in the pool. A reservation is one line,
  "this owner will soon use so many places, as this role, at this tier", and stands in the pool's
  reservation table.
  - A reservation resolves provider, model, effort and tier the same road as a request and freezes it
    as a snapshot, and a task naming a tier other than the role's reserves at that tier.
  - Reserving again with the same key makes no new reservation and only changes the desired count —
    raising queues only the missing places, lowering releases the latest-queued places first.
  - Reserving with the same key on an already-released reservation stands that reservation again with
    the current mapping, and the places queue anew at the back.
- **Each place of a reservation** is a request-table row carrying that reservation's token.
  - A place row queues in the same FIFO as an ordinary request, and when its turn comes and there is
    room under the limit, becomes reserved, not active.
  - The limit counts active and reserved together, so a reserved place cannot be taken by another
    request, and the reserving owner's actual request does not requeue.
  - If the owner has a reserved place on the same provider and tier, one transaction releases that
    place and admits the actual request as active. If the owner's not-yet-reserved place is that tier's
    FIFO head, the actual request takes over that place's turn.
  - When a place frees, when the limit changes, and on the pool's own clock every five seconds, the pool
    raises a head place to reserved. An already-reserved place is not taken away by lowering the limit — the same promise as
    an admitted request's snapshot.
- **Release:** a reservation releases only when the owner releases it or there is evidence the owner
  ended, not for time passing.
  - Releasing a reservation cancels the waiting place rows and releases the reserved ones.
  - What proves the owner's end is known by the owner's side: the co-work runner by the work order
    record, the research runner by the run row and unit, a native reservation by the CLI process that
    set it. The pool reads no owner's name and judges nothing; each owner releases its own.
- **No reservation, no warming.** Only while a reservation stands does the pool keep alive, up to that
  reservation's desired count, the prefix that reservation will use, before the cache cools.
  - Not one ping and done. A ping restarts the prefix's clock, so the next ping goes after the idle
    threshold again, continuing until the reservation releases.

The warming judgement is split three ways.

- **The kind of place** names candidates. Each kind answers for the reservations of its own sources:
  where this provider and model's prefix is, when it was last used, whether a run is working in it
  now, and what run a ping becomes. Agency's pool holds two kinds.
  - A co-work branch's place is a line in the standing checkouts list (「Standing branch checkouts and
    their list」 below). Which pool those lines stand in, and which places the owner's tasks sit in now,
    the consumer answers for the reservation's owner through the co-work places hook (`docs/api.md`,
    「Hooks」); where it answers none, the reservation's places are not warmed.
  - A native co-worker is a kind that names no place to warm (「Native co-worker places」 below).
  - A reservation whose source neither kind answers — a search or browse run's, whose place is the
    research branch line's prefix (`research-runs.md`) — holds its places, and agency's pool warms
    nothing for it. What a branch last wrote that line with is kept all the same, by the place key its
    consumer names: the provider, model, effort and upstream, and the latest moment it was written,
    which a later write never moves back (`docs/api.md`, 「Co-worker pool」).
- **The provider** names the lifetime and the ping method. The lifetime is the value that provider
  announced and its idle threshold, and a ping is running the same run line one turn on that provider's
  program. Each provider's implementation holds that answer, and the pool does not branch by provider
  name. A reservation on a provider turned off, or on one that lends no co-workers, is not warmed.
- **The pool** chooses. For each reservation it picks, up to the desired count, from candidates not
  currently working with a matching fingerprint that the keepalive schedule finds due — past the idle
  threshold and within the usable lifetime, the announced lifetime less a short margin
  (`docs/keepalive-schedule.md`, 「When a prefix is worth a ping」) — closest to cooling first.
  - An already-cooled prefix is left — its value is the same whether a ping or a task uses it, and if
    the reserved work never comes, only what the ping used is wasted.
- **The record:** each ping writes the reservation token, place, provider, model, time and outcome to
  the pool's ping table. Since the reservation and this record survive a bot restart, no ping goes
  again to a place whose last ping is younger than the idle threshold, and a failed ping keeps its
  reason too.

Per provider:

- **Claude Code:** warms. Runs the place's run line once there and keeps no session.
- **Codex:** warms if the model it runs announced a cache lifetime. Codex's prefix is built by Codex
  from its own documents, so a ping too runs Codex once with the same run description, keeping no
  thread. Even a new run keeping no thread opens the default instruction, skills list, the checkout's
  own instructions and turn settings in the same text for the same place and run description, and the
  only thing changing per run is a recording id. A date in the turn settings makes the first run after
  a day change rewrite that prefix. A Codex place has no fingerprint, so it is judged by clock alone.
- **OpenRouter:** warms only when the upstream that took the prefix is recorded and that upstream
  announced a lifetime. The ping goes to that one upstream without fallback (`docs/cache-fallback.md`).
  - A co-work branch has the answering upstream recorded in the job record and held by the standing
    checkouts list line, so it warms on that upstream's clock.
  - A search or browse branch launches only on a provider carrying the research run line, and
    OpenRouter's program, dsh, does not carry that line, so OpenRouter has no branch place to warm
    (`research-runs.md`).
- **A registered model:** does not warm. The service announces no cache lifetime and a request with no
  tools reports no cache read or write, so there is no clock to keep alive. A registered-model
  reservation only holds the place.

- **The gate** is agency's own keepalive schedule (`docs/keepalive-schedule.md`). Before a
  reservation's places are judged, the pool asks it whether the pool's pings go on that reservation's
  provider, and asks again before each ping goes; while the owner's warming switch stands it says no,
  and no ping goes and nothing is written down. None goes to a turned-off provider either, and none
  during a rest window on a provider whose pings rest with the account — Claude Code's, following cache-fallback's
  rule that Codex and OpenRouter warm separately on their own clocks (`docs/cache-fallback.md`). The
  reservation stands regardless of this gate and keeps holding the place.
- **Auto-progress stop** is the reserver's rule: the co-work runner sets no reservation while stopped
  and releases those it set.
- **Timing:** the pool's warming runs on agency's own clock at the keepalive pass's period, after the
  same start wait, never beside itself; raising places runs on the more frequent pool tick.

## Native co-worker places

The worker settings' hook takes the place for one native `Agent` call.

- **The provider a call stands on** is read off the environment the program hands its hook: the
  provider that owns the program, and Claude Code where none does.
- **A run on a provider that lends none:** its call is refused at once, taking no place, and the
  program does that part itself (`docs/model-tiers.md`, 「Common co-worker capacity」). The
  reservation command below refuses such a program the same way.
- **Before the call:** the pre-call hook reads the type as a role, sets model and tier by the
  `role:<type>` mapping on the provider the program stands on — the registry's for a worker on a
  registration, Claude Code's for one on Claude Code — puts in a request on that provider keyed by the
  session and tool-use id, and waits for a place FIFO. The wait is a few minutes, shorter than the
  hook limit, and if no place comes in it, the request is cancelled and the call refused with a reason.
  The refused call's work is done by the caller with its own tools. A call that got a place is allowed while its input is edited: the
  caller's prompt is appended the way to read the code dependency graph, so whatever role it is called
  as, and whether the caller mentioned the graph or not, the co-worker receives that text.
- **The call ends:** a call that received its result releases the lease. If it answers that it handed
  off to a co-worker running in the background, that agent id is written to the lease and it is not
  released.
- **The call fails:** releases the lease.
- **The co-worker stops:** releases the lease written to that agent id. If it stops before the id is
  written, the stop is recorded first, and the following end sees it and releases at once.

- **Binding:** a waiting request binds to the waiting hook process, an active lease to the calling CLI
  process, both with pid and start time. Only with evidence the process ended does a later hook or the
  pool's own clock, every five seconds, reclaim — cancelling a waiting request and releasing an active
  lease. Not reclaimed for time passing.
- **Pre-reserving:** a job can pre-reserve places for native calls it will soon make.
  `bmo-agency coworker pool reserve --role <role> [--tier <tier>] --count <count>` sets a reservation
  on the provider the calling program stands on, as the pre-call hook reads it, and the same command's
  `release` releases it.
  - The reservation's owner is the CLI process that called that command, with pid and start time. The
    pre-call hook, if the calling CLI is that process, takes over that reservation's places without
    requeuing. With evidence the CLI process ended, a later hook or co-work runner tick releases the
    remaining reservation.
  - A native reservation only holds places and does not warm. A native co-worker's prefix is assembled
    by the parent CLI within its own session from the role definition, tools and what it inherits from
    the parent, and sending those bytes from outside the CLI would need opening a new Claude Code
    session, which the live-claude guard blocks (`docs/live-claude-guard.md`).
- **Inside a branch job:** if one provider is blocked it does not cross straight to another provider —
  doing so would run a model different from the provider and model the first lease recorded. That job
  is closed as failed, the outer runner releases the existing lease, takes a fresh FIFO request on the
  next provider's same tier, and launches a separate attempt. An ordinary worker job's provider
  fallback is not subject to this limit.
- **A plan holds as many stages, and a stage as many tasks, as its design asks for.** How wide a stage
  is belongs to the design, and how many run at once belongs to the pool. When the width exceeds the
  pool, tasks all remain in the plan and FIFO, and only those given a place become running. The rest
  wait as pending, having started neither the model process nor the run-limit clock. When one ends and
  releases a lease, the next tick launches the front task of the same provider and tier.
- **A standing branch checkout is only a place to reuse a prompt cache, not model capacity,** so the
  list below and the pool lease do not stand in for each other.
- **While waiting for work,** that work order's builder turn does not run, and only the existing worker
  keepalive policy applies to the worker's session in the meantime. An auxiliary role has no session,
  so there is none to continue or warm.
  - Instead, while a stage runs, the co-work runner reserves in the pool, per provider and tier, that
    many of the places the next stage's branches will use, up to the most places a limit can hold. A
    task naming a tier reserves at that tier. When the next stage starts, those requests take over the
    places, and if no task remains to wait on that provider and tier, the remaining reservation is
    released.
  - When a work order stops, fails or ends, all its reservations are released, and on a bot restart a
    tick releases that work order's reservations not matching the work order record. The runner does
    not warm; the pool does (「Reserving places and keepalive」 above).

## Standing branch checkouts and their list

- **The places branches work in** are the standing checkouts under a runtime directory keyed by the
  pool and the place, and one place takes one task at a time.
- **Which places are taken is the consumer's to say.** A consumer asking for a place, or ending an order,
  hands in the places its tasks sit in now — each the checkout its order works in and the place's
  number — and agency counts those as busy; it reads none of the consumer's orders or jobs. The ask
  names the order's checkout, the task's selection, its role and the tier it asks for, and is answered
  with the place's number and its folder.
- **A branch's turn reaches its place as a thrown-away session** whose folder is the place's, naming
  the task's item and role (`docs/api.md`, 「Sessions」): agency finds the place by that folder and
  puts the turn into the place's program under the label below.
- **A pool is one repository:** the work order checkout's git common directory sets the pool, and the
  pool name is that repository's name plus a short hash of the common directory path.
  - Places stand only as that repository's worktrees, so they never check out another repository's
    commit, and place numbers, busy places, list lines, the place-count limit and clearing are all
    counted within the pool.
  - No other repository takes over and remakes a place one worker's repository used, so installed
    dependencies and a warmed prefix stay for that repository's next task.
  - A place numbered directly under the root, not under a pool, belongs to no pool and is borrowed by
    nobody, so when picking a place, if nothing runs in it, it is detached from its repository, cleared,
    and its line deleted.
  - **A pool nothing can reach any more is swept on the same pass.** The live pools are the ones the
    workers' working directories name, together with the pool a place is being handed out of at that
    moment; every other pool standing under the root, and every pool the list still holds lines for, is
    an orphan. A checkout that moves makes one: its common directory changes, so the pool it names
    changes with it and the old pool is left standing with its places, its lines and its agents while
    nothing will ever borrow from it again.
    - Each place of an orphan is taken off whichever repository it is a worktree of, cleared, its lines
      deleted and its agent retired — the same disposal a place of a live pool gets — and the pool's own
      directory goes once nothing of it is left.
    - A place a task is still sitting in keeps its pool standing until that task has ended, and the
      sweep comes back on the next pick.
- **On taking work,** the place is rewound to the work order's HEAD — tracked files rewound, untracked
  files deleted, ignored files kept, so installed dependencies stand with the place, its submodules
  forced onto the HEAD's pins — and on top of it the worker checkout's uncommitted changes and new
  files are laid.
  - If the repository the place points at differs from the work order's or cannot be read, it is not
    rewound but detached and remade as a worktree of the work order's repository.
  - A place left only registered but gone is remade, and a place long idle with no live line is cleared.
- **Each place is a resident agent of its own** (`docs/agent-control.md`, 「One standing pane per
  agent」), named after its pool and its number, in a herdr workspace of its own. It has no row in the
  agents table: what it is, is the place.
  - **It holds the role agency's policy names for co-workers,** the worker role, so a task's turn in it
    and every ping that warms it is shown the worker's tools (`docs/agent-roles.md`).
  - **The name is the key and the label is what is in the place.** The agent name is what the place is
    found, stood and retired by, and it never changes. What a person reads in herdr is the workspace
    label, and it follows the run the place holds.
    - A co-work task's turn stands under the run: the item's number, the task's role and the place's
      number as two digits — `#<item> scout 01`.
    - A place with no item's run in it — a ping warming it, or a task's turn that has ended — stands
      under the place itself: `coworker <pool> <place>`, the number again as two digits.
    - A turn renames the workspace, its tab and its head pane to the run as it stands, and back to the
      place as it ends. A workspace herdr revived without its token is found by either label.
  - A branch run in the place, and every ping that warms it, is a turn of the program standing in that
    workspace's pane. A branch keeps no session, so each one opens a fresh session, and a fresh session
    is a new command — the program is started again in the same pane, on the same prefix.
  - A turn in another directory of the place is a turn of the same agent. The working directory is part
    of the command, so the program is started again there, in the same pane.
  - Each branch turn's execution record names its run as its parent and its task as its root, as every
    job turn's does, and says it was a turn of the place's program.
  - A place taken away takes its agent with it: the program is ended and the workspace closed.
  - **An order that ends takes its places' programs down.** When an order is done, or stops, every
    place one of its tasks worked in that no other order's running task is sitting in has its program
    ended and its workspace closed. The place itself — its directory and its list lines — stays, so the
    next task that takes it reads the same prefix and stands the program up again in a new workspace.

### Why a place is reused: the prefix

- The prompt cache attaches only to a prefix with not one character different, and that prefix contains
  the absolute path of the place the checkout's own instructions were read from.
- So opening a new path per task is writing that part of the prefix from scratch per task, and reusing
  the same place turns that into a read.
- What a provider holds is not the conversation but that prefix, so the next task that inherited the
  place opens a new session yet does not rewrite the prefix.

### The list

- One line per pool, place, provider and model, holding which provider it is, which role last answered
  there, the last answer time, and the prefix fingerprint then.
- **Liveness** is measured with a per-provider cache lifetime, but not using the full announced
  lifetime.
  - The announced value is the one each provider's cache policy holds per provider and model (for
    OpenRouter, per answering upstream and model), and the lifetime measured when picking a place is
    that less a short margin.
  - Fitting exactly to the boundary would read a place already cooled on the provider's clock as alive
    and pick it, landing a run promised warm on a cold prefix.
  - The margin is not written per provider but in the one place subtracting from the announced
    lifetime, so the same margin follows when more providers are added.
  - A line past its lifetime is not skipped but deleted.
- **Beyond the clock, a moved prefix kills a line.**
  - A Claude Code line whose fingerprint differs from what the place would now open is a line pointing
    at no cache, so it is deleted.
  - A Codex line has no fingerprint — the fingerprint measures the prefix Claude Code assembles, and
    Codex makes its own text its own way — so a Codex line dies by clock alone.
  - An OpenRouter line dies by clock alone too, without a fingerprint, but that clock is not the
    model's but the upstream that actually took that branch (`docs/cache-fallback.md`). So an
    OpenRouter branch job records the answering upstream in the job record, and the line records that
    upstream too and is measured on the lifetime that upstream announced. One place and model has a
    separate line per upstream.
  - If no answering upstream was recorded, or that upstream announced no lifetime, no line is left, and
    a provider announcing no cache lifetime leaves no line at all — no lifetime is invented where there
    is none to measure, and that provider's branches still borrow the place.
  - A provider whose announced lifetime is shorter than the margin is treated the same — after
    subtracting the margin no trustworthy portion remains, no different from not announcing.
- **When a stage launches several on the same provider and model,** this list answers three things: an
  empty place within lifetime and with a matching fingerprint is used as is, the shortfall is opened as
  cooled or new places, and lines past lifetime are deleted.
  - When answers return, the line of that place, provider and model records the provider and model that
    actually answered, the role, the current fingerprint and the time.
  - Places are grown only up to the maximum branches one stage launches at once — one place is one
    prefix, so growing places is growing cooled prefixes.
- **As a candidate for the pool:** this list is what a co-work branch place's kind supplies to the
  pool. When the co-work runner's reservation stands, the pool picks, among the lines of the reserved
  provider and model, places not currently working, and warms them (「Reserving places and keepalive」
  above).
  - A ping runs the worker's run line once at that place with that provider's program.
  - The place's lock is asked without waiting, so a place with work in it is skipped.
  - A ping writes one turn and touches only the prefix, so it leaves nothing in the session record; on
    success it writes the current fingerprint and time to that line, restarting the prefix's clock, and
    records what it used under that place's name.
