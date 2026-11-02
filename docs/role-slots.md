# Role slots

A role is a concurrency cap for its runs: each run of a role occupies one slot of it while it goes, and a
run beyond the cap waits in the role's line until a slot frees. This document states how a run asks for
its slot, how the line orders the asks, how slots are reserved ahead, and the standing branch checkouts a
co-work branch works in. What a role is, is `docs/agent-roles.md`; which provider a run goes out on is
`docs/provider-order.md`.

## Who takes a slot

- **Every run of a role takes a slot of that role before its program launches:** a job's run, a worker
  ask, a delegated task, a co-work order's builder stage, a co-work branch, a search's or a browse's run and
  its branches, a translation, a native co-worker a job's program calls, and an upkeep run. Nothing runs a
  role's program outside a slot.
- **A supervisor's turn takes none.** A supervisor is no role; its turns are bounded by its one program
  (`docs/supervisors.md`, 「Where a supervisor's turn runs」).
- **A slot is asked under a role the store holds,** and is refused 422 under any other: an unknown role is
  an error, never a run on some default.
- **A slot's source and owner are the caller's.** A slot is asked under the source label its caller sends —
  `job`, `ask`, `delegate`, `cowork`, a runner line's name for one the research unit asks, `native` for a
  native call — and under the owner and request key its caller names; the label is kept as sent.
- **A slot is let in on its role's order unless its ask names a road.** An ask may name a provider and a model
  together, the road it then goes out on and waits for; or a model alone, whose provider the role's order
  gives: the first entry of the order naming that model, else the role's active provider as the ask is made,
  else the order's first. The entry the provider has in the role's order gives the effort and the thinking.
- **A slot of a role whose order is empty goes out on the road of the supervisor that asks it,** the one its
  ask names as `askedBy` (「A delegated task」 below). Asked under no supervisor the store holds, it is
  refused 422 with why, and nothing starts.

## The cap

- **A role's cap is the most of its runs that are let in at once,** the row's `max_concurrent`. A run beyond
  it waits for a slot and none is refused, but for a double's own ask where delegated runs hold the whole cap
  (「A delegated task」 below).
- **The cap is the role's alone.** It is counted over every run of the role whatever its provider, its model
  or its path: a role's runs all go out on its active provider (`docs/provider-order.md`, 「Choosing the
  provider」), and no provider, model or path carries a limit of its own.
- **Runs do not take turns at a path.** A run occupies a slot of its role wherever it works; two runs at
  two paths, or two runs a caller sends to one folder, are two slots. What keeps two jobs out of one
  checkout is the repository's checkout, not the slot (`docs/worker-slots.md`).
- **The cap is read as every run asks to be let in,** so a changed cap holds from the next admission: a
  raised cap lets in at once the waiting runs it frees, and a lowered one takes no slot away from a run
  already let in.
- **A cap of zero closes the role:** its runs wait until the owner raises it.

## The line

- **An ask goes into the slot table before the program launches.** One immediate transaction checks the
  role's active slots plus its reserved ones, who stands ahead of the asker, and whether the run's provider
  has quota (`docs/provider-order.md`, 「When every provider is out」), and grants a lease.
- **Within one role, waiting asks stand in the order they were asked,** by a global increasing sequence, and
  a slot goes to the first of them whose owner is asking now.
  - **An owner asks for its waiting request by asking for its admission,** again and again while it waits
    (`docs/api.md`, 「slots, reservations and places」). Every ask is numbered in one increasing order of
    the line's own, which counts no time.
  - **A request ahead of the asker stands in its way while its owner is asking:** a reservation's slot
    always does, and any other request does where its owner has asked since the asker's own last ask.
  - **A request whose owner is not asking keeps its place and is passed over, never dropped.** It leaves the
    line only by its token's release or cancel, or where the evidence below shows its owner ended.
  - **A request whose provider has no quota is passed over** while it has none, keeping its place, so a run
    waiting for quota never holds up a run of the same role that could go; it is next again once its
    provider has quota.
- **Request keys:** the key a caller asks under names one request for good. An ask repeated with that key
  while its request stands — waiting, reserved or active — is answered with that request, unchanged, so a
  caller that lost an answer asks again safely. Once the request is given back, released or cancelled, the
  key is spent: an ask under it is refused 422 and opens nothing, and the caller's next run asks under a key
  of its own.
- **A place key:** a request may hold one, and only one active request per key exists. A job's run is keyed
  by its checkout, so one checkout carries one run at a time; a request blocked by its key does not block
  another request behind it.
- **A lease** preserves a hard-to-guess token, a stable owner and request key, the role, the path, the
  chosen provider, model and effort, the cap then, and the time. Changing a role's row does not change this
  snapshot for a request already waiting or let in.
- **Release and cancel** are done by the holder of the token, and by the owner for everything it holds
  (「An owner's release」 below), and finish once however many times called. They finish at once when
  success, failure, a launch refusal or a cancel is confirmed. Each given-back request keeps who gave it
  back: `holder`, the token's holder; `owner`, its owner's release; or `run-ended`, the line on evidence its
  run ended.
- **Giving back a lease bound to a role session ends that session,** whether its holder finishes it or its
  owner releases it: a turn running in the session is cut, ending `stopped` with the reason `released`
  (`docs/api.md`, 「sessions」), and the session is ended once that turn has ended. Nothing goes on running on
  a slot given back.
- **A lease is bound to what its run stands as:** a systemd unit, a herdr pane or an open execution by its
  name, or a role session by its id. A role's start that names the slot binds the lease to the session it
  opens, whether or not the lease was bound before (`docs/api.md`, 「sessions」), and a binding naming the id
  of a session agency holds is bound to that session.
- **A slot carries several sessions in turn.** Each start naming the slot moves its lease onto the session
  that start opens, and the session the lease was bound to before is bound to it no longer. A consumer that
  runs sessions one after another on one slot — a first answer, its rounds, a review — starts the next
  session on the slot before it ends the one before; ending the session the lease is still bound to gives
  the lease back.
- **An admitted lease bound to nothing is given back only by its holder or its owner:** the line reads no end
  of a run that never named itself, so such a lease stands until its holder finishes it or its owner's release
  gives it back, however long that is.
- **A bound lease is given back only on an observed end, never for a span of time:**
  - a lease bound to a session, once that session ends while the lease is still bound to it: ended by its
    caller, by its holder's finish or by an owner's release, or gone with the agency process that held it,
    since a role session is held in memory from its start to its end and a restart ends every one. A
    session the lease has moved off ends without touching the lease;
  - a lease bound to a unit, a pane or an execution, once the line's pass has seen that run standing and then
    sees no systemd unit, herdr pane or open execution holding it. A run not yet seen standing is never read
    as ended, however long it takes to come up. The pass reads from the server's start and each minute after;
    it is a reading, not a bound on any lease;
  - the token holder's release, or the owner's.
- **An active lease is never taken away for its age:** only these ends give one back, and a native call's the
  evidence its own process ended (「Native co-worker slots」).
- **An admission answers what the request is now:** `waiting` while it waits in the line, `active` once let
  in, and `released` or `cancelled` once given back, with who gave it back and why. A caller asks again only
  while it waits; a request given back by anyone stays given back.

## An owner's release

- **An owner gives back everything it holds in one ask** (`docs/api.md`, 「slots, reservations and places」):
  every request it asked that still waits, stands reserved or is let in — a waiting or reserved one
  cancelled, an active one released — every reservation it holds, and every role session bound to one of its
  leases, ended, a turn running in it cut first (「The line」). A consumer asks it as it starts and as its run
  ends, so it keeps no list of its own holds to give back, and a lease let in whose run never started a
  session is given back with the rest.
- **The release is the owner's word, read as given:** agency judges nothing of why, and a request asked under
  another owner is not touched.

## Prior and yielding requests

- **A request is a prior request or a yielding request.** The caller decides which; the line does not read
  the reason. Every request now asked is a prior request; yielding is a rule the line holds for work that
  can defer behind another's.
- **A yielding request enters only** where no prior request of the same role stands in its way and the
  role has a free slot; an already-active yielding request is not taken away. A waiting yielding request
  may be raised to a prior one, keeping its sequence.

## Reserving slots

- **Whoever will soon launch runs of a role reserves slots of it:** "this owner will soon use so many slots
  of this role". A co-work runner reserves, while a stage runs, the slots the next stage's branches will
  use; a job pre-reserves the native calls it will soon make.
- **Each reserved slot is a request carrying its reservation's token,** queued in the role's line, and
  reserved — not active — when its turn comes and the cap has room. The cap counts active and reserved
  slots together, so a reserved slot cannot be taken by another request, and the owner's actual request
  takes it over in one transaction without queuing again.
- **Reserving again with the same key** only changes the count: raising queues the missing slots, lowering
  releases the latest-queued first.
- **A reservation releases only** when its owner releases it or there is evidence the owner ended — the
  co-work runner by its order record, the research unit by its own run, a native reservation by the CLI
  process that set it — never for time passing.
- **A reservation holds slots and warms nothing.** No role is warmed: a run's prefix is read again by the
  next run while it is still cached, and nothing pings it in between (`docs/keepalive-schedule.md`).
- **The line's own pass** runs when a slot frees, when a cap changes, and every five seconds, raising to
  reserved every reserved slot no request stands in the way of while there is room.

## A delegated task

- **A task a supervisor hands off through `bmo/delegate`** (`docs/tools.md`, 「The delegate」) asks a slot of
  the role its ask names — `delegator` where it names none — as the source `delegate`, keyed by a request of
  its own, and waits in that role's line however long it takes.
- **A task naming no role is the supervisor's double:** it holds a slot of the `delegator` role. The delegator
  role is seeded with an empty order, so its lease carries the asking supervisor's provider, model and effort
  as they stand when it is asked — the provider, model and effort its last turn went out on, else its order's
  first — and it waits, never moves, while that provider has no quota. An order the owner sets on the role
  stands over that, and the slot is then let in on that order like any run of the role. The delegator role's
  cap counts every supervisor's double together.
- **A task naming a role** holds a slot of that role and goes out on that role's own order, like any run of
  it, or on the asking supervisor's road where that role's order is empty.
- **A double's own ask never waits on doubles.** Every delegated run is a double, and a double may delegate in
  turn; its ask says it comes from a double. Such an ask is refused, before it enters the line, where slots
  asked under the source `delegate` and let in already number the role's cap or more, the cap being above
  zero. Waiting there could have no end: each of those runs may be a double waiting on a slot of its own, and
  in any ring of such waits the last double to ask found every slot it waited for held by delegated runs, so
  refusing that ask leaves no ring to form. Any other ask, a supervisor's among them, waits as the line says.
- **Once let in, its run goes out at the task's folder** under the role's prompt and bundles, with the asking
  supervisor's tools beside them (`docs/tools.md`, 「The delegate」), resuming the
  session kept for that role at that folder on that provider and model where it is valid
  (`docs/sessions.md`, 「A role's session」).
- **Its stage stands on its slot:** waiting while its slot waits, running once let in and bound to its run,
  and ended once the slot is finished, with what it came to. `GET /delegations` lists them per asking
  supervisor (`docs/api.md`, 「slots, reservations and places」).

## Native co-worker slots

The worker settings' hook takes the slot for one native `Agent` call a job's program makes.

- **The type a call names is the role,** and the hook reads it as one: a role the store does not hold, or one
  not marked callable, is refused, and the caller does that part with its own tools.
- **The provider is the caller's program's,** read off the environment the program hands its hook — the
  provider that owns the program, and Claude Code where none does — since a native child runs inside that
  program. The model and effort are the co-worker role's order's entry for that provider; a role whose order
  holds no entry for it is refused, and the caller does that part itself.
- **Before the call** the hook puts in a request under the role, keyed by the session and tool-use id, and
  waits in the role's line, however long that takes. A call that got its slot is allowed while its input is
  edited: the caller's prompt is appended the way to read the code dependency graph.
- **The call ends:** a call that received its result releases the lease; one that handed off to a co-worker
  running in the background writes that agent id to the lease, which its stop releases. A failed call
  releases the lease.
- **Binding:** a waiting request binds to the waiting hook process, an active lease to the calling CLI
  process, both with pid and start time; only evidence the process ended reclaims either.
- **Pre-reserving:** `bmo-agency coworker pool reserve --role <role> --count <count>` sets a reservation of the
  role for the native calls the calling program will soon make, and the same command's `release` releases
  it. Its owner is the CLI process that called the command.

## A co-work branch

- **A plan holds as many stages, and a stage as many tasks, as its design asks for.** How wide a stage is
  belongs to the design, and how many run at once belongs to each task's role. Tasks a slot is not free for
  remain pending, having started neither the model process nor any clock; when one ends and releases its
  lease, the next tick launches the front task of the same role.
- **A branch's provider is its role's active provider,** never its parent's: the dispatcher takes the role a
  task names and the role's order (`docs/provider-order.md`).
- **Inside a branch, a provider that fails does not cross straight to another:** the branch is closed as
  failed, its lease released, and a fresh request taken for the same role, which goes out on the role's next
  provider as a separate attempt.

## Standing branch checkouts and their list

- **The places branches work in** are standing checkouts under a runtime directory keyed by the pool and the
  place, and one place takes one task at a time. A standing checkout is only a place to reuse a prompt
  cache, never a slot: the list below and the role's line do not stand in for each other.
- **Which places are taken is the consumer's to say.** A consumer asking for a place, or ending an order,
  hands in the places its tasks sit in now, and agency counts those as busy; it reads none of the consumer's
  orders or jobs. The ask names the order's checkout, the task's selection and its role, and is answered
  with the place's number and its folder.
- **A branch's turn reaches its place as a run whose folder is the place's,** naming the task's item and role
  (`docs/api.md`, 「sessions」).
- **A pool is one repository:** the order checkout's git common directory sets it, and its name is that
  repository's name plus a short hash of the common directory path. Places stand only as that repository's
  worktrees; place numbers, busy places, list lines and clearing are counted within it.
  - A place numbered directly under the root, not under a pool, belongs to no pool and is cleared when
    picked.
  - A pool nothing can reach any more — none of the repositories' checkouts names it — is swept on the same
    pass: each place is taken off its repository, cleared and its lines deleted, and the pool's directory
    goes once nothing of it is left. A place a task still sits in keeps its pool until that task ends.
- **On taking work,** the place is rewound to the order's HEAD — tracked files rewound, untracked files
  deleted, ignored files kept, submodules forced onto the HEAD's pins — and the order checkout's uncommitted
  changes and new files are laid on top. A place whose repository differs or cannot be read is detached and
  remade; a place registered but gone is remade.
- **A branch's run in a place is a run like any other:** a pane of its own labelled `#<item> <role> <nn>`,
  closed with the run. A place keeps no program between branches.

### Why a place is reused: the prefix

- The prompt cache attaches only to a prefix with not one character different, and that prefix holds the
  absolute path of the place the checkout's own instructions were read from. Reusing the same place turns
  that part of the prefix into a read; what a provider holds is the prefix, so the next branch that takes
  the place opens a new session yet does not rewrite it.

### The list

- **One line per pool, place, provider and model,** holding the provider, the role that last answered
  there, the last answer time and the prefix fingerprint then; an OpenRouter line holds the answering
  upstream too.
- **Liveness** is the provider's declared cache lifetime for that model (for OpenRouter, the answering
  upstream's) less one margin subtracted in one place (`policy/pool-warm.toml`). A line past its lifetime
  is deleted, and so is a Claude Code line whose fingerprint differs from what the place would now open; a
  Codex or OpenRouter line has no fingerprint and dies by clock alone, and a provider announcing no lifetime
  leaves no line.
- **When a stage launches several branches of one role,** the list answers which places to use: an empty
  place within its lifetime and with a matching fingerprint first, the shortfall opened as cooled or new
  places, up to the most branches one stage launches at once.

## On a registration the arbiter stands

- **A slot on a registration whose engine the arbiter stands** asks its demand once the line lets it in
  (`docs/own-llm.md`, 「An engine the arbiter stands」). The role's cap is the ceiling and the master's
  grants are what run: a run waits for its grant as it waits for its slot, and gives its demand back with
  its lease. A slot asked from a run on such a registration asks within that run's demand, and a slot whose
  caller hands in a granted demand of its own stands on that demand and asks none.
