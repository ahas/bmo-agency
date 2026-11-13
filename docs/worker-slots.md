# A repository's checkouts

- **What this says:** how one repository runs several jobs at once — its base checkout, the support
  checkouts beside it, the worktree each is, the branch each job works on, how a job's work lands on
  main, and what a job may write outside its own checkout. Which role a job runs as and how many of a
  role's runs go at once are `docs/role-slots.md`'s; a checkout bounds only how many jobs of one repository
  stand in its tree at once.

## The base checkout and the support checkouts

- **The base checkout:** the path the repository's row names (`docs/repositories.md`). Between jobs it
  stands on main; while a job of a repository with support checkouts is at work in it, it stands on that
  job's item branch (「A job in the base checkout」).
- **Support checkouts:** further worktrees of the same repository, numbered from 2, which exist so that
  the queue can run more than one item of one repository at a time. They are the repository's, never rows
  of their own.
- **The support count is the repository's setting,** from 0 to 8 (`policy/workers.toml`): how many support
  checkouts stand beside the base. Zero is none, and the repository then runs one job at a time.
- **Where it is changed:** the repository's settings in the console's repository tab, agency's repository
  routes (`docs/api.md`, 「repositories, prefix-checks and room-holdings」), and `bmo-agency agent --set
  <repository> --support <n>`. A change settles the checkouts at once: a missing one is made at its first
  launch, and one beyond the count is removed once no job holds it. Agency settles every repository's
  checkouts again as its server starts, so a setting written while it was down takes effect then.
- **A TODO item names the repository, never a checkout:** the repository's queue is every one of its
  checkouts' queue.

## Names and places

- **Name:** `<repository>-w<n>`, with `n` counting from 2 — the base is the first of its repository.
- **Label:** the repository's label followed by `#<n>`.
- **Directory:** a git worktree of the repository at the base checkout's path followed by `-w<n>` — `<base
  path>-w2` for a repository's first support checkout.
- **The path is fixed for the life of the support checkout.** A job does not get a directory of its
  own: the directory is part of every Claude session's prefix, and a session is kept under its
  directory's name, so a path that moved per job would cold-start every session and could resume
  none of them.
- **The worktree is made at the launch, before the run's program is stood, the first time a job
  lands on it** — on whichever road the launch takes. It is detached at the base's main branch, read
  off the base checkout, and the repository's own setup is then run in it: the script named
  `worktree:setup` in its `package.json`, where there is one. What it installs and links is that
  repository's own; the operations repository's installs its workspace from its lockfiles and links
  its env files, the way the base checkout has them.
  - **The submodules come first,** in every repository and before its script: each submodule the
    commit pins is cloned where it is missing and put on the pinned commit, so a project kept in a
    repository of its own stands in every checkout as it does in the base. A folder an older commit
    tracked where a submodule now stands keeps only what git ignored in it, installs and builds,
    which would refuse the clone, so it is emptied first. What is open inside a submodule is left
    as it stands, and a submodule git will not move over it fails the setup. A repository's own
    script may bring them the same way before it installs, so a checkout set up by hand has them too.
- **A support checkout whose worktree is missing gets it made again at its next launch,** by the same
  cut: detached at the base's main, the submodules first and the repository's setup after them. A
  directory holding anything that is not a checkout is somebody's and is refused rather than taken
  over, and a worktree whose setup failed is taken away so that the next launch starts it afresh. The
  setup runs until its script exits, however long a cold install takes, and a second launch of the same
  support checkout waits for the first to finish making the worktree, then finds it standing.
- **The setup is run again every time a checkout's files may have moved under its installs:**
  whenever the runner, a work order or a borrowing puts a checkout on a job's branch — a support
  worktree or a base checkout alike — and whenever a close has rebased a branch onto main. Main
  moves under every checkout as other jobs land, and a lockfile it brought along leaves the
  installs behind the code, so a suite would fail on something the job never touched.
- **So a repository's setup is cheap when nothing moved,** and deciding what moved is the
  repository's own: a setup may hash its lockfiles and install only where that hash differs from the
  one written beside its installs when they were last made, or where nothing is installed. A setup that fails stops the job from starting, or fails the close,
  and says why.
- **What is shared and what is not:**
  1. The package installs share Bun's install cache, which hard-links the files, so a support
     worktree's `node_modules` costs links rather than copies and is still right for its own branch.
  2. Build output is each worktree's own, because several dev servers write it at once.
  3. Each support checkout has its own workbench folder under its own name, so its dev server reads
     a data copy of its own and never the live data.

## A job on a support checkout

- **Every job works on a branch of its item,** made from main in that worktree: `work/<item>`, or
  `work/job-<job id>` for a job with no item. The worktree never has main checked out.
- **Main is the branch the base checkout stands on,** read off the base checkout — the one the
  repository's row names — and never off the support worktree, which stands on a `work/` branch. A base
  standing on a branch other than `main` has its support checkouts' jobs made from that branch, and
  the job unit's landing verb and the close land them back on it.
  - Where the base checkout itself stands on a `work/` branch — its own job's — main is the branch
    that `work/` branch was put on from. Putting a checkout on a job's branch writes that down in
    the repository's git config, beside the branch, and deleting the branch takes it away.
  - Where nothing is written down, it is the branch the remote's own head names, and failing that
    `main`, then `master`.
- **Making a job's branch also writes down the branch it was made from,** beside it in the same
  config, once, as it is made. Taking the branch up again later leaves that alone, whatever main
  then reads, and it is what the landing checks the branch against (「Landing on main」).
- **Before the job's turn starts, the runner puts the worktree on that branch:**
  1. Whatever the last job left uncommitted there is committed onto the branch that job was on,
     which is how a job is parked. Where the worktree is already on the job's own branch, what is
     open is that item's own and is left open for the job.
  2. A branch already fully merged into main is deleted once the worktree has moved off it.
  3. The item's branch is checked out where it exists — a rerun takes up where the last run
     stopped — and made from main where it does not.
  4. The repository's setup is run on what the branch holds.
- **A job launched again on another road stands in the same checkout.** Where a job's road stops
  answering and the job goes out again on the next one (`docs/cache-fallback.md`, 「The provider order's choice」), its turn is put on the same branch of the same worktree, which is already the job's
  own: what the first road left open there stays open, and the new road's turn is told it continues
  work already in the checkout.
- **The job commits on its branch and does not push it.** The close lands it.
- **A verb run inside the worktree works on the worktree.** The job carries agency's
  environment, whose `BMO_SOURCE_DIR` names the base checkout, so every verb that reads or writes
  the checkout's own files asks for the working checkout instead: the worktree the
  command was started inside, where git registered it under the base checkout's repository, and the
  source root everywhere else. The deploy tree is such a worktree too and never counts. The graph
  tool and the worker's own graph tools (`docs/worker-mcp.md`) read and write the worktree's graph through it, which is
  what keeps a job from leaving an uncommitted graph edit in the base checkout that would refuse
  every later landing.

## A job in the base checkout

- **A repository with support checkouts keeps main out of every job's hands.** An ordinary job the
  repository runs in the base checkout works on its item's branch there too, the same `work/`
  branch a support checkout would take, and lands the same way. A repository whose support count is
  zero has nobody to land beside it, and its base job works on main as it stands.
- **Why:** a landing moves main, and a checkout standing on main with a job's uncommitted files open
  in it is one main cannot be moved in without either refusing or overwriting that job's work. With
  the base job on a branch, main stands checked out nowhere while it works, and every landing moves
  the ref alone and goes through at once, whichever files the base job has open.
- **As the job starts, the runner puts the base checkout on the branch:**
  1. From main, the branch is made or taken up, and anything already open on main — a person's own
     edit — is carried across as it stands rather than committed; it stays somebody else's to the
     job, as it always is in a shared checkout.
  2. From a `work/` branch an earlier job was left on, what is open is parked on that branch first,
     as a support checkout parks.
  3. The repository's setup is run on what the branch holds, as in a support worktree.
- **As the job ends — before its report goes out — the runner puts the base checkout back on
  main:** what the job itself left uncommitted is committed onto its branch, which is parked work
  exactly as on a support checkout; main is checked out as the landings left it, so the checkout is
  current the moment the job is over; and the branch is deleted where main already holds all of it.
  What was open before the job began is carried back as it stands. A switch that cannot be made is
  named in the report, and the next job's start parks and moves off the branch.
- **A work order in the base checkout works on its item's branch too.** The order puts the checkout
  on the branch when it takes it and back on main when it lets it go, and its stages run on that
  branch without moving it; the close of its last stage lands the branch.
  - It lets go by stopping or by finishing. Either way the change its own turns wrote there — read by
    their time windows, since others may write in a base checkout — is committed onto the branch,
    unchecked, since the branch is not main; what anybody else left open goes back to main as it
    stands, and a file mixing the two is left open and named.
  - An order that parked onto main or onto `wip/<item>` before it worked on branches has that parking
    brought back as before, onto its branch.
- **A base checkout another repository's job borrows** is put on that job's branch and back the same
  way, where its repository has support checkouts.

## A repository whose base is a worktree of another's

- **A repository's path may be a linked worktree of another repository's checkout** — a repository kept
  for one pinned model beside the code tree's own row, `<checkout path>-<name>`. It is a repository of its
  own: its own name, label, pin, rules and queue, and no other repository's settings count it.
- **A repository renamed with its checkout has the worktree moved by agency,** as its server starts and on
  the heads' clock, the rename read off agency's policy (`policy/agent-renames.toml`): git moves it to the
  directory the rename names while agency holds the checkout's job lock, so no job is at work in it, and the
  row is pointed at the new directory only once it stands there. A job holding the lock puts the move off to
  the next pass.
- **Its checkout is its own:** its jobs hold that worktree's lock alone, so a job of the other repository
  never holds it up nor it the other's.
- **It works the way a support checkout does:**
  1. Main is read off the repository's main worktree — the checkout git keeps the repository in — and never
     off the worktree, which cannot have main checked out while that checkout does.
  2. Every job works on its item's branch made from that main, put on and parked as on a support checkout
     (「A job on a support checkout」), and the worktree is never put back on main.
  3. Nothing else writes in the worktree, so everything a job leaves open there is its own.
  4. The close lands the branch on that main, and a work order and a borrowing take the worktree the way
     they take a support checkout.
- **The worktree is made once, by hand:** `git worktree add --detach <path> main` in the main checkout, the
  repository's setup (`worktree:setup`) run in it, and the row then pointed at it with `bmo-agency agent
  --set <repository> --path <path>`. Nothing makes or removes it for the row.

## Landing on main

- **The job unit's close does it.** Agency states it here because every checkout it keeps is landed
  by it. The close, run in a checkout standing on a
  `work/` branch — a support worktree, or the base checkout during its job — refuses a tree with
  uncommitted changes to tracked files, then:
  1. waits its turn in the repository's close line (the deploy unit's `deploy.md`, 「The close line」), so no other
     close of the repository on this machine checks or lands until it has landed;
  2. rebases the branch onto main as it stands now; a conflict aborts the rebase and is refused
     with the files it stopped on, and resolving it — rebase, fix, continue, close again — is the
     job's own work; the repository's setup is then run on the rebased tree, and a setup that fails
     refuses the close;
  3. runs the static check and the tests the rebased commit selects (the repository's own rule for which tests a change runs), or skips the tests with `--skip-suite` as any close may;
  4. lands the commit, in this order:
     - pushes each submodule commit the landed commits pin anew to its own repository's main where
       that does not hold it yet. A submodule commit that would not fast-forward its repository's
       main refuses the landing with nothing moved, and rebasing it there and moving the pin again
       is the job's;
     - where main is checked out, makes sure fast-forwarding that checkout would overwrite none of
       the files open there;
     - pushes the commit to the remote's main as a plain fast-forward;
     - only once the remote has taken it, fast-forwards main here, writes down what the job landed
       and sets the checkout main stands in up again;
  5. writes the landed commit down as waiting to go live, and lets the line go.
- **The remote's main is what a landing wins.** A push the remote refuses (a commit pushed there from
  elsewhere first, a rule of the remote's own) leaves main here and the record of what the job
  landed untouched, and the landing takes another round: the remote's main is brought in where the
  close follows it, and the branch rebased again. A refusal on every round refuses the landing with
  what the remote said.
  - Where main here does not move once the push went through, the next round follows the remote's
    main and moves it there; a main that still will not move refuses the landing as one that could
    not be fast-forwarded, the commit already on the remote.
- **Where main is checked out, fast-forwarding it moves that checkout.** That is the base checkout
  between jobs, fast-forwarded in place; what is open there stays where it is, and where the
  landing would overwrite one of those files the landing is refused before the push, naming the
  checkout and the files, with nothing moved. While a base job is at work main is checked out
  nowhere, and the landing moves the ref alone, guarded by the commit it expects main to be at.
- **A checkout moved by a landing is set up again where its installs moved.** Where the commits
  main moved over changed a path the close names as an install's input, the repository's setup runs in the checkout main stands in once the push went
  through and main is fast-forwarded there, so its installed packages match what it now stands on.
  - Which paths call for it is the close's to say: the landing itself knows no repository's
    install, and the landing verb names none.
  - A setup that fails there does not undo the landing, which has already happened; the close says
    it failed and names the checkout, and the next setup in that checkout brings it level.
  - Where no setup is called for, that checkout's submodules are still brought to the pins main
    now holds, as the setup brings them first, and a failure is said the same way.
- **The units linked into the checkout main stands in follow its files.** An installed user unit
  may be a link into that checkout rather than into the deploy tree — the gate's long runs, herdr —
  and a commit moving the unit's file would leave the link dangling, so the next reload makes the
  unit vanish. The close and the landing verb keep them by the deployment's own rule
  (the deploy unit's `deploy.md`, 「The unit links」):
  - Before main moves, every such link whose file the landed commit lacks is matched to the
    commit's one tracked file of the unit's own name. None, or several, refuses the landing with
    nothing moved, naming the unit.
  - A unit linked there that is running in a folder the landed commit no longer has — its working
    directory — holds the landing: it is refused, naming the unit, until the unit stands idle, since
    a running process whose folder is removed under it fails on its next relative path.
  - Once main has moved there, each link is pointed at its file and the user units are reloaded.
  - A base checkout put back on main as its job ends relinks the same way whatever its job's own
    commits left dangling; nothing holds there, since those commits already moved the files.
- **A branch lands only on the branch it was made from.** Before anything moves (the close's first
  rebase, and every round of a landing) the branch it was made from is read: the one written down
  as it was made, or, for a branch made before that was written, each branch it could then have been
  made from: the one written down for the checkout's return, the remote's head, `main`, `master`.
  Where one of those shares a commit with the job's branch that the branch being landed on lacks,
  the landing is refused and names both, since the rebase would carry that branch's commits across
  and the push would publish them.
  - Which branch the work belongs on is the owner's call, and the refusal says how to land it
    either way: move only the job's own commits across with `git rebase --onto <landing branch>
    <the branch it was made from>` and close again, or stand the base checkout on the branch it was
    made from.
  - A branch made from the branch it lands on passes however far that branch has moved since, as
    does one whose own commits were moved across that way.
- **Whoever lands second rebases.** Two jobs that changed the same file each land on their own
  close: the first moves main, and the second's rebase, once its turn in the close line has come,
  meets the first's change and resolves it.
- **Two landings do not race.** Landing holds the repository's landing lock, and waits for another
  landing to let it go, however long that landing's own check again takes; a landing whose process
  has ended holds nothing. The
  closes of this machine already land one after another through the close line; the lock is what
  keeps a close on a work node, which lands from outside it, from racing one of theirs.
- **A commit lands only on the main it was checked against.** Inside the close line main moves only
  by a landing from outside it. Where main moved while the close checked, the landing rebases the
  branch onto it once more and, before fast-forwarding, the close
  checks the rebased commit again: the repository's setup, the static check, and the tests selected
  over the range from the main it last checked to HEAD — what main brought meanwhile and the job's
  own commits together. A commit written against files main has since moved or renamed is refused
  there rather than landed.
  - The check again runs inside the landing lock and inside the landing's three rounds: a main that
    moves again while it runs is taken in by the next round, checked from the main the last check
    covered.
  - A failure refuses the landing with what failed, as a failing close does; the branch stays
    rebased, and the job mends it and closes again.
  - Tests the close skipped with `--skip-suite` stay skipped; the static check always runs.
  - The backlog deployment runs the tests at the head for any batch of more than one commit,
    which is where two commits that each passed are checked together.
- **A repository without such a close lands with the job unit's landing verb,** out of the deploy
  tree's binary, run in the checkout standing on the job's `work/` branch after the job's own checks:
  the same rebase and the same landing, without a suite of its own.
- **A commit holding Hangul never lands, in any repository.** Before it pushes, every landing — the
  close's, the landing verb's and one a work node asks of the operations machine — reads the job's own
  commits on the branch past main and refuses where a subject or body holds Hangul outside the names
  of address the close's policy sets aside, naming each commit by its short hash and subject, with
  nothing moved (the worker's standing rules, 「Language」). Rewording them and
  landing again is the job's.
- **What the runner counts as the job's own commits** on a branch is what the landing recorded it
  landed and what is still on the branch past main — not everything between where the branch began
  and where main now stands, which holds other jobs' landings too.

## Checkouts on the nodes

- **A repository may also have one checkout on each work node that takes jobs,** named `<repository>@<node>`
  with no number, set in its settings as the list of its node checkouts and never by the support count
  (`docs/api.md`, 「repositories, prefix-checks and room-holdings」; `docs/work-nodes.md`, 「Which nodes take
  jobs」). Its base stands on this machine. Its path is the one the setting names, written from the node's home
  or absolute, else the base's folder name followed by the node's beside the base; one removed from the list
  while no job holds it goes from the row, and its checkout on the node stays there.
- **Its jobs work on their item's branch as every support checkout's do,** made from the remote's main,
  which the runner brings up in that checkout first.
- **Its landing keeps the rules above:** the node's close or the landing verb holds the repository's landing
  lock here through the home bridge, takes the remote's main in before each round and pushes it, and
  the base checkout here follows. A landing here takes the remote's main in first wherever the
  repository has a checkout on a node (`docs/work-nodes.md`, 「Ending a job on a node」).

## Parking

- **Parking is leaving the branch.** A job that ends without landing leaves its branch where it is,
  and the next job on that worktree commits what it left open onto it before moving off.
- **A stopped work order on a support checkout** commits everything open in its worktree onto its
  item's branch and lets the worktree go. Nothing else writes in that worktree, so all of it is the
  order's; no check runs and nothing is sorted by who wrote it. On resume it takes that worktree
  again once it is free, and checks its branch out there.
- **A base checkout parks by time windows:** a stopped work order sorts out its own change there by
  its turns' time windows. Where the repository has support checkouts the change goes onto the order's
  branch (「A job in the base checkout」); where it has none it is checked and goes onto main or
  `wip/<item>`, as the job unit's `work-orders.md` states.

## The queue with support checkouts

- **A repository takes as many jobs as it has free checkouts** — the base's, and each support checkout's
  within the count — and each of them waits for a slot of the role it runs as as well
  (`docs/role-slots.md`). The fact that tells the supervisor the queue is waiting counts the free checkouts
  of each repository, and one item is launched per free checkout.
- **The launcher picks the checkout.** It takes, in this order: a free support checkout already on the
  item's branch; the base checkout; a free support checkout by number. A launch naming a support checkout
  directly takes that one or is refused. Where the repository has checkouts on the nodes, the launch first
  takes its machine from the rotation and then the checkouts there in that order (`docs/work-nodes.md`, 「The
  rotation」).
- **A stopped work order at the head of the queue is its runner's to resume** in the checkout it holds; the
  items behind it are still told as waiting where the repository has another checkout.
- **An item already at work is not launched a second time,** on any checkout of its repository; the refusal
  names the job that has it.
- **A launch meeting a held lock tries the next free checkout** before it refuses.
- **Free checkouts are not room on the machine.** A launch with a free checkout and a free slot is still
  held while the machine is short of memory, threads or job places (`docs/agent-control.md`, 「Room on the
  machine」), and the queue is told again once it has room.

## Dev servers

- **A support checkout's dev server stands only while a job in it runs and has asked for it,** as
  every checkout's does (`docs/worker-bench.md`, 「It stands only while a job runs」). It runs the
  repository's dev command in the support worktree, from its own workbench, and stops once the job
  lets the job hold go or leaves it unused; a fold holding the checkout's job lock does not stand it.
- **It is told its number:** the pane runs it with `BMO_WORKER_SLOT=<n>` in its environment, and a
  dev server that binds ports moves them by that number. The operations repository's bench does
  (`docs/worker-bench.md`); the base's dev server runs with none and keeps its ports.
- **The job's preamble says which checkout it is:** a job in a support checkout opens with the
  repository's preamble under a header saying it is support checkout `n` of the repository, in its own worktree and on
  its item's branch, that its dev server stands only once the job starts it, the lines that start,
  restart and stop that dev server, and that where the owner's text names the base's
  workbench, console or ports, it means the support checkout's own. A base job on its
  item's branch is told the branch, and that the close lands it rather than a push.

## A job writes only in its own checkout

- **Every repository's checkout is someone's.** A job may write in its own checkout and in a checkout it
  has borrowed, and in no other checkout of any repository — base or support — whoever holds it. A place no
  repository names is not guarded by this.
- **The job guard refuses the write,** from the file tools and from the shell alike, and the
  refusal names whose checkout it is and the verb that borrows one.
- **Borrowing:** `bmo job borrow <repository>`, run inside a job, takes a free checkout of that
  repository for the rest of the job — a support worktree first, the base checkout where the repository
  has no support checkout free, on the job's own machine — and prints its path. It is an assignment held in
  the job's name (`docs/api.md`, 「assignments and jobs」), so the borrowed checkout reads held everywhere a
  held checkout is read, and nothing is launched into it. The job commits, pushes and closes there the way
  that repository closes a job. A job's own repository is refused, and so is a repository whose every
  checkout is at work: it says so, and asks nothing to wait.
- **A job borrows with what its program holds.** A job's program is handed no control token, so the verb
  calls agency's server with the job's session token, and the server lends only to the job running in the
  checkout that token was issued for (`docs/api.md`, 「The control port」). A process holding neither token
  is refused, naming both.
- **When the job ends the loan ends.** The next read of the assignments releases one its job no longer runs
  under, whichever way the job ended. What it left open stays in the checkout, for its report to name.
- **Why:** a stopped job parks everything open in its checkout as its own, so a write from any
  other job would leave with it. With every job in a checkout of its own, what is open in a checkout
  is its holder's, and parking needs to sort nothing.
