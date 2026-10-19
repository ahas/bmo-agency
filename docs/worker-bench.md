# The worker's workbench

## What a worker holds outside its jobs

Each job worker holds three things beside the work of its jobs:

1. the **preamble**, which stands ahead of the job prompt
2. the **workbench folder**, which remains regardless of sessions
3. the **dev server** its row names, which agency stands for each of its jobs and which a job does not start
   beside the pane but restarts when it needs to

- **Where they come from:** all three follow from what the worker's row says, and differ per worker.
- **None of them is part of the worker session's prefix.**

## Preamble

### What it is

- **A field of the worker row.**
  - It is written where the worker line is edited in the console's agent management screen
    through agency's worker routes (`docs/api.md`, 「Workers」); `bmo-agency agent` edits the same
    field.
  - Left empty, there is none.
- **The owner's text, and the owner's choice.** For example: which port the dev server stands on,
  what its outside address is, and to check it rather than start a new one.

### It stands at the very front of the prompt that opens a job

- **Read fresh:** every time the runner hands a job's text to the worker, it reads the preamble from the
  worker row at that moment and attaches it.
- **Every job gets it:**
  - ordinary jobs
  - each stage of a work order, since each is its own job — builder turns continuing the same session,
    and branch roles running their own session in a copy
- **The order of the prompt received:**
  1. the handed-over summary, on a turn that opens a new session
  2. the preamble bundle
  3. the impact grounds its item was filed with, where it carries any and the run is not a co-work
     branch (the job unit's `todo.md`, 「Impact grounds」)
  4. the job's text
  5. the knowledge its item calls for, where the knowledge gate picked any (the reference unit's
     `reference-selection.md`, 「Work beyond a turn」)
  6. the trailer
- **Reshape requests:** a request to reshape repeats the whole job prompt, so the preamble goes along
  inside it.
- **Editing the field:** the new text stands from the next job prompt handed over.

### What it is not attached to

- **Two pings:** the keepalive ping and the ping that warms a place the pool reserved are fixed
  one-word texts.
- **Two kinds of added words:** added words pushed into a running turn, and added words asked after an
  answer, are said to a session that already has the job prompt.
- **So none of these four** carries the preamble.

### It is not the prefix

- **Where the prefix comes from:** the command line and the files it points at — the CLAUDE.md chain,
  settings, MCP config, plugins and skills (`docs/launch-lines.md`).
- **The preamble is in none of them,** nor in the run line's arguments or environment.
- **So editing the field** leaves agency's `worker_prefix_digest` unchanged: no session fold, no cold cache.

### It is attached outside sanitising

- **Why:** the preamble is agency's setting, not something someone sent.
- **When:** like the trailer, it is attached after the job's text has been sanitised.
- **No cross-effects:**
  - A verdict on the job's text never alters this field's text.
  - Editing this field never revokes a verdict remembered for the job's text.

### The bundle is agency's header plus the owner's text

- **The header:** if the worker has a preamble or a dev-server command, or works in a directory that is
  not a git checkout, the job's prompt opens with a header:
  - the path of this worker's workbench folder, and where in it what one item alone needs goes
    (「An item's scratch」 below)
  - if the directory is not a checkout, that there is nothing to commit, push or deploy, so the closing
    steps that need git are skipped and no repository is made
  - if there is a dev-server command, that the dev server stands in a pane outside the job, the path of
    its log file, and the exact line that restarts it — with the rule that a job restarts a dev server
    that is down, failing or serving stale code and carries on, rather than leaving it as unfinished
- **Below it** stands the owner's text as written.
- **None present:** a worker with none of the three has no bundle at all in its job prompt.
- **Language:** the header, like the trailer, is the deployment speaking to the worker, so it is
  written in English.
- **Its lines** are the job unit's, which composes the job's prompt; agency keeps what it is composed
  from: the worker's preamble and dev-server command on its row, and its workbench folder
  (「Workbench folder」 below).

## Workbench folder

### What it is

- **Where:** `workbench/<worker name>/` under the runtime data root.
- **What goes there:** things a job has no reason to remake each time, such as a check copy of the
  data, check logins and the dev-server log.
- **Not the session scratchpad:** that is made fresh per session and never reused, so it is no place for
  these.

### How it is kept

- **Agency creates it:**
  - for every worker when it starts
  - for a worker when that worker is added
  - for a job's worker when the job starts, which the job's runner asks of agency with the rest of the
    bench: what a job before it asked of the dev server taken back, and the dev pane brought up where
    the worker has a dev command (`docs/api.md`, 「Workers」)
  - leaving it as is if it already exists
- **A co-work branch has none:** its name lives one task and stands in no agents row
  (`docs/coworker-pool.md`, 「Standing branch checkouts and their list」), so a branch's job makes no workbench and its prompt
  names none.
- **Deleting a worker does not delete what the folder holds,** since agency does not know what is inside.
  A workbench no agent's row names and that holds nothing goes with the item scratch sweep below.
- **Inside no checkout,** for the same reason the data root is outside every checkout.
- **Left out of deployment backups:** its contents can be remade, and one data copy exceeds a hundred
  megabytes.
- **How a job learns the path:** only from the header line of the preamble bundle. It is not passed as an
  environment variable, since the job worker's run-line environment is fixed.

### An item's scratch

- **What one item alone needs goes apart:** a rehearsal's data root, a copy of the data taken for
  that item's check, a check's build or download. It stands in `items/<item number>/` inside the
  workbench folder; a PostgreSQL database made for it on this machine's cluster is named
  `bmo_item_<item number>_<anything>`. The header tells a job both, and the job reads its item number
  off its own item.
- **It is one of the job's own places,** so a confirmation on a command that stays inside it is
  answered by the job's runner, never put to the owner (`docs/agent-control.md`, 「A command the runner allows
  itself」).
- **It goes once no job of the item is open.** Agency's minute watch
  (`docs/agent-control.md`, 「Room on the machine」) looks whenever the set of items with an open job
  changes, and once as its process starts: every `items/<n>/` folder of every workbench whose item has no open job is removed, and
  every `bmo_item_<n>_…` database likewise dropped. The same look removes every empty workbench folder no
  agent's row names. A database the account cannot drop stays, and the
  watching process's journal names it.
- **What outlives the item stays beside it,** in the workbench folder itself: the dev server's data
  copy, logins, a tool's environment several items reuse. Nothing outside `items/` is removed by the
  watch.
- **Why:** a data copy or a rehearsal is gigabytes, and one left behind by each finished item is
  what fills the disk.

## Working directory

- **Named or made.** A worker is added with a directory, or with none; a worker added with none works in
  `worker-homes/<worker name>/` under the runtime data root, made when the worker is accepted.
  - A worker name holds no path separator and cannot start with a dot, so the directory cannot reach
    outside that folder.
  - A directory left there by a removed worker of the same name is taken over as it stands.
  - Removing the worker does not remove the directory, for the same reason as the workbench.
- **A checkout or not.** Any existing directory is accepted; whether it holds `.git` (a directory, or the
  file a worktree carries) decides what the worker's jobs do.
  - **A checkout:** everything a job does, co-work included.
  - **Not a checkout:** ordinary jobs only — in effect a chat worker.
    - A launch that asks for co-work (the item's flag, `--cowork`, or the sentinel in the text) runs as an
      ordinary job, and the launcher says so; agency refuses a co-work place in such a directory.
    - The job's steps that read git — the work-tree baseline, the commits a job made, the graph check —
      find nothing and count as nothing, not as a failure.
    - The preamble header tells the job to skip committing, pushing and deploying.
  - The worker screen marks such a worker as not a checkout.
- **The lock stays the directory's.** A made directory is one per worker, so such a worker never waits
  for another.

## Dev server

### The command

- **Another field of the worker row: the dev-server command.**
  - One shell line; left empty, that worker has no dev server.
  - The command decides the port, bind address and base path. Agency does not pick ports.
- **The outside address** is the machine's own web server's, which splits paths onto ports; the command
  must listen on the port that site expects.

### It stands only while a job of the worker runs

- **Every worker alike:** a base worker's, a support worker's and a worker of any other repository.
- **And only once that job has asked for it.** A job's start stands no server; a job that is about to
  look at what the server serves starts it (「A job starts, restarts and stops it」), and a job that
  never does holds none.
  - **The request** is a file in the workbench folder, `dev-server.wanted`. The start and restart
    verbs write it; the pane removes it whenever it takes the server down for good — the job over,
    the server idle, its lifetime past, or a stop asked for — and the job's runner removes one left
    from before as the job starts.
  - **Why:** a console dev server holds two to three gigabytes once its pages have been opened, and
    one stood up for every job, the many that never open the console included, ran the machine out
    of memory with five of them standing.
- **What counts as a job running:** the job hold, a lock of the checkout's own that the job's runner
  holds beside the checkout's job lock for the whole job. Nothing but a job's runner takes it, and it
  goes with the runner's process, so a runner that died holds nothing.
- **Why not the job lock:** the keepalive ping, the fold and the standing pass take the checkout's job
  lock too, for as long as each runs. A dev server read off that lock would come up for a keepalive and
  live for as long as the keepalive kept the worker's session warm. The keepalive keeps the session's
  cache and nothing else.
- **The job ends, the server ends:** once no job holds the hold, the pane stops the server, removes the
  request and closes itself; the next job starts it again where it needs it.
- **Its lifetime:** a server that has run longer than the settings row `dev_server_max_age_minutes`
  (180 where there is no row) is stopped and the pane closes, job or no job. The clock starts again with
  every start, and a job that still needs the server starts it again (「A job starts, restarts and stops
  it」).

### It goes down when nobody uses it

- **What counts as use:**
  - the start or the restart verb, each of which leaves the request anew;
  - the server's own use mark: the pane names a file in `BMO_DEV_USED_FILE`, and a server that can
    tell when it was asked something touches it. The console's development build touches it on every
    request its server answers, at most once a minute; a server that never touches it is judged by
    the other two;
  - a client outside the server's process group holding a TCP connection to a port the group listens
    on: a browser with a page open holds the page's hot-reload socket. A connection between two
    processes of the group — the console's server reading its stand-in bot — is not use. The pane
    asks `lsof` for the group's connections every thirty seconds, since one asking costs a third of a
    second of CPU.
- **Idle for too long, it goes down:** a server nothing has used for the settings row
  `dev_server_idle_minutes` (20 where there is no row) is stopped, the request removed and the pane
  closed. A job that needs it again starts it again.
- **When memory is short, the idle go first:** while the machine is short of memory by its launch
  limits (`docs/agent-control.md`, 「Room on the machine」) — less memory available than
  `machine_limit:launch_memory_available_gb`, or the last minute's memory pressure at or above
  `machine_limit:launch_memory_full_pct` — a server unused for two minutes goes down. One in use
  stays: a browser on its page, or requests within those two minutes.

### It stands in that worker's fixed workspace

- **Where:** the workspace where the head pane stands (`docs/agent-control.md`) has one dev-server tab,
  and that tab's pane runs `bmo-agency dev-server pane` with the worker's name.
- **Why not in the head tab:** the head tab's one pane is the worker's resident program itself — the job
  turns, the keepalive and the fold all go into it and no run opens a pane beside it — so the dev
  server stands in a tab of its own and never shares that pane.

### What the script in the pane does

- **Starts the command.**
  - It runs the row's command with zsh, with the worker's checkout as the working directory.
  - The command gets its own session and process group, and that group number is written to
    `dev-server.pid` in the workbench folder.
- **Clears a leftover first.**
  - On a fresh start, the script first ends the group that file points at if it is still alive.
  - Why: when the pane closed and the script died without cleaning up, a dev server left holding the port
    must not block the new one.
- **Its own environment.**
  - Agency's `BMO_` variables are removed from the dev server's environment, and the workbench folder
    path is added as `BMO_WORKBENCH_DIR`, its use mark as `BMO_DEV_USED_FILE` (「It goes down when
    nobody uses it」).
  - This keeps agency's secrets from reaching any dev server; if a dev server needs a `BMO_` variable,
    the command writes it itself.
  - This environment is the dev server's, not the job's.
- **Logs.**
  - Output flows into the pane as it is and is also written to `dev-server.log` in the workbench folder.
  - The log rotates at a size limit.
  - A job reads the dev server's state from this file, not through herdr.
- **Restarts.**
  - When the command ends, it starts it again after a few seconds.
  - If it keeps ending right after starting, it widens the interval and says so in the pane.
- **Follows the row.**
  - It rereads the row every few seconds.
  - If the command changed, it ends the running command with its process group and starts the new one.
  - If the command is empty or the worker is gone, it ends it and ends itself; herdr collects a pane
    whose command has ended.
  - So editing the field alone changes or brings down the dev server.
- **Follows the job and its use.**
  - On every look it also asks whether a job holds the checkout's job hold, whether the request still
    stands, how long the server has run and how long since it was last used, and whether the machine
    is short of memory.
  - No job, no request, a server past its lifetime, or one idle past its allowance stops the server,
    removes the request and ends the script, which takes the pane with it.

### Agency sets it up

- **Idempotent:** setting it up ends in the same result however many times it is called.
- **A pane stands only where the request stands** (「It stands only while a job of the worker runs」);
  everywhere else setting it up closes any pane it finds.
- **Called at five moments:**
  1. When agency's server starts, for every worker with a dev-server command whose checkout a job holds.
  2. When the herdr subscription is reattached. If herdr died and came back, the pane front's process is
     dead.
  3. Right after agency's worker routes change a worker's dev-server command, for that worker.
  4. When that worker's job starts, before the runner hands over the first prompt, with the request
     left from before removed first: the pane a job that died left standing is closed.
  5. When a job starts or restarts the server, by the verb.

#### The setup order

1. **Find the workspace:** find or create the fixed workspace and set up the head.
2. **Find the pane:** a pane in that workspace with no run token, which `layout.export` says runs the
   dev-server script under this worker's name, is the dev-server pane.
3. **Otherwise start one:** in a new tab with `layout.apply` given only `workspace_id`.
4. **No `tab.focus`:** it would steal the screen a person was looking at.

#### The rules around it

- **The binary's version rides in the command arguments.**
  - The version is a hash of the agency binary.
  - A dev-server pane with a different version is closed and started anew.
  - A running pane does not reread its own code, so when a deployment restarts agency, moment 1
    replaces panes of the old version.
- **One setup at a time.**
  - Agency's server, a job's runner and the verb are separate processes and may try to set up the same
    worker's dev server at once.
  - So setup holds a file lock, one per worker, in the data root's lock directory; whoever comes later
    finds the pane the earlier one started.
  - If two or more dev-server panes of the same worker are still seen (say, a person started another),
    only the first in the snapshot is kept and the rest are closed.
  - Inside the lock only one side closes, so there is no disagreement about which to keep.
- **Not closed when the head is set up anew.**
  - Whoever resets the head closes panes with no run token, but keeps panes whose command is the
    dev-server script.
  - A pane herdr revived has no command, so it is still closed, and moment 2 starts a new one.

### It is not a run

- **No run token:** the dev-server pane has none.
- **So it is outside the run bookkeeping:** it is not in the picture that tracks liveness, not in the run
  list the head pane writes, and takes no lock a job waits on; it only reads the job hold.
- **Tied to the job by the hold alone:** it outlives no job; a job never starts a second server beside it
  and never ends the pane, but starts, restarts and stops the server through the verb (below).

### A job starts, restarts and stops it

- **The lines:** `<agency binary> dev-server start|restart|stop <worker>`, given in full
  in the preamble header, since a worker's checkout may be any repository.
- **`start`:**
  1. Refuses a name that is not a worker, or a worker with no dev-server command — a support
     worker's being its base's (`docs/worker-slots.md`) — or a worker no job of which is running.
  2. Leaves the request in the workbench folder, which counts as use as well.
  3. Brings the dev-server pane to the row, as agency does, so a pane that is missing is started.
  4. Where a live server stands already, says so at once. Otherwise waits, up to a minute, until the
     pid file names a live process group, then prints it and the log's path; on no new server it
     prints the log's tail and fails.
- **`restart`:** the same, with a restart request left beside the request, and the wait is for a
  process group other than the one standing.
- **`stop`:** removes the request, then waits, up to half a minute, until the server's group has gone,
  the pane having stopped it on its next look; a worker with nothing running is told so. It refuses
  only a name that is not a worker.
- **What the pane does with the request:** on its next look it takes the request away; a running server
  is stopped and started again at once, and a server waiting out the delay after failures is started
  at once with the failure count cleared. A pane that starts fresh drops a request left from before,
  since it starts the server anyway.
- **When a job uses them:** it starts the server just before it first looks at what the server serves —
  it takes about a minute to come up — and restarts it where the server is down, answers with errors,
  or holds code the job has changed in a way hot reload does not take; a dev server it could start or
  restart is never reported as unfinished work. Done looking, it stops it rather than leaving it to the
  idle rule.
- **What stays off limits:** the system's own standing services are not a dev server,
  and a job does not restart them; the deployment does.

## Servers a job leaves listening

- **What they are:** a process listening on a TCP port whose environment names a worker's agent place
  (`BMO_AGENT_PLACE`), which is to say something the worker's program started: a `nuxi dev` a job ran
  itself, in the background or detached.
- **What they are not:** the worker's dev server carries no `BMO_` variable (「What the script in the
  pane does」); a server a person started in a shell of their own and the owner's units name no place.
  None of them is ever a candidate.
- **Why they outlive the job:** the worker's resident program stays up between jobs, kept warm by the
  keepalive, and whatever a job's turn left running stays with it.
- **When they are swept:**
  - The job's runner sweeps its own worker's as the job ends, before it hands the job over, by asking
    agency across the boundary (`docs/api.md`, 「Workers」).
  - Agency sweeps every worker's every ten minutes, which catches a runner that died. It takes a server
    whose worker has no job holding its checkout's job hold, and one that has run past the dev
    server's lifetime even while a job runs.
  - An agency server on a copy of the data root sweeps nothing: the machine is not the copy's.
- **What is ended:** the server's process group, TERM and then KILL after five seconds, and only where
  every process in that group names the same place. The worker's program shares its group with its
  head, which names no place, so a group holding the program is never signalled; a server in such a
  group is reported and left.
- **What is recorded:** each server ended is written to the runner's or agency's journal with the
  worker, the checkout it ran in, its age, its memory and its command. Which job's run it started in
  is the job service's to read off its own jobs' times, so agency's line names no job.
- **Not swept:** co-worker places and the places of agents that are not workers.
- **Reading them:** `bmo-agency dev-server list` lists every worker's dev server and every server a job
  left, with the memory each holds and what the sweep would do with it; `--sweep` does it.

## A repository's own bench

- **What a repository's dev-server command stands up is that repository's own:** its console, its
  stand-in services, its ports, its address and its authentication, and the tools a job uses to
  measure or picture a screen there. That repository's documents state them.
- **What agency holds to of any of them** is what this document says: the command is one field of
  the worker row, it runs in the worker's checkout from its workbench folder, it carries no `BMO_`
  variable but its workbench path and its use mark, a support worker is told its number in `BMO_WORKER_SLOT`
  (`docs/worker-slots.md`, 「Dev servers」), and it stands only while a job of the worker runs and has
  asked for it.

