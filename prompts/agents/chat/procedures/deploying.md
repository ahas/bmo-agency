# Putting a change live

Read this before a deployment, folding the worker sessions or reloading a document.
All three are owner-only work (ABSOLUTE RULE 2).

## Deploying

- **What a job hands over:** it commits, pushes, runs the whole suite as its own gate and closes
  with `bmo job close`, which writes its commit down as waiting to go live: its item,
  worker and own line of why. A job's shell is refused at every deploy command.
- **What is waiting** stands under 「[올라갈 커밋]」 in `running`, and as `pendingDeploy` on a
  `job-ended` fact. Run it as soon as something waits, whether or not jobs are at work (a job's end
  that brings a commit is the moment), in one call:
  `~/.local/share/bmo/deploy/bin/bmo deploy backlog`.
  - It puts main live: the commits jobs landed, never what a checkout has open. Where more than one
    commit goes together, or main is past the one commit a job vetted, it first runs the whole suite
    once at main in a tree of its own. It deploys once for everything waiting as a round opens.
  - **One run carries what keeps landing.** A round opens once nothing has landed for a short quiet
    window, so the run itself waits for the landings to settle and batches them, or once the oldest
    commit has waited its ceiling, and at once for a 즉시 commit. After a round that went live the same run goes round again while commits it did not carry wait; after a round
    that failed, only where new commits have landed since. Running it while one is at work is never
    refused: the second ends at once, saying the running one carries what waits. A `배포 대기` in
    `running.ts` is a deployment still waiting for the gap, or one that died midway: run it either
    way, and a live one carries what waits while a dead one's hold is let go. A `deployed` fact
    naming commits still waiting needs nothing from you while the run has rounds left; only one that
    says the run went its last round asks you to run it again.
  - **It never runs in your shell.** It starts a unit of its own (`bmo-deploy-backlog-<stamp>`),
    prints the unit's name and the journal line to read it by, and returns at once; the deployment
    goes to its end whatever happens to your call, so no `timeout` around it is needed or reaches
    it. `bmo deploy commit`, `bmo deploy main` and `deploy-web.ts` behave the same.
  - Follow it in `running.ts`, whose 「[배포]」 names every deployment at work, or in the printed
    journal. You are told unasked when there is something to act on: `deploy-batch-failed`,
    `deploy-waiting-gap`, `deploy-gap-missed`, `deploy-hold-released`, `deploy-not-contained` (a
    waiting commit is not in main), `deploy-bot-down` (the bot was down or restarting itself when
    the batch was to go live, so nothing went; the owner was told on his own road — once you are
    up and stayed up, run it again), `deploy-failed` (the deployment itself did not go through;
    the commits stay waiting), `deploy-main-ahead` (nothing waited, but main is past what is
    live: 「A commit you made yourself」 below) or `deploy-releases-pinned` (more than twelve releases
    stand on the disk because live processes still stand in old ones; it names each release with its processes — a job's go when
    the job ends, and any other is the owner's to end, so tell him which). A deployment that went
    live is heard in your own going-down and back lines.
  - Jobs at work and the deployment do not stop each other; a job that ends while the bot is down
    is told to you once it is back. The owner's pause and the deployment leave each other alone.
- **A commit marked 즉시 cannot wait:** it moves the archive's schema, and until it is live every
  script run from the checkout refuses to open the archive. Deploy at once. The mark is computed
  from the commit; a job cannot set it.
  - A move that only adds (a table, a column) goes live at once under jobs at work, whatever
    checkout or repository they run in; the deployment reads that off the settling itself.
  - Only a move that takes away what an older runner names waits for the gap. It holds the queue
    with its own hold (`배포 대기` in `running.ts` and on the console, with the commits, what it
    takes away, the jobs and the time left) and tells you `deploy-waiting-gap`; launch nothing
    while it waits. It goes live the moment they end; after four hours it lets the hold go and tells
    you `deploy-gap-missed`: run it again then.
  - The owner's resume never lets that hold go. Only he does, by hand, from the console or by
    asking you for `~/.local/share/bmo/deploy/bin/bmo todo --release-deploy-hold`; the
    deployment then deploys nothing, tells you `deploy-hold-released`, and the commits wait for the
    next gap.
  - The deploy tree's verbs you run match the archive, so none, `bmo deploy backlog` included, is
    refused in that gap.
- **A deployment cut off midway** leaves the deploy tree on the new commit while the bot runs the
  old code or is down; where that commit moved the schema, every deploy-tree verb refuses the
  archive with `archive schema behind the deployed code`. `running.ts` heads its report with
  「배포가 도중에 끊겼습니다」 and the one command that finishes it: run that at once, before anything
  else.
- **A batch that fails goes live only up to the commit before the first that breaks it.** The run
  searches the batch's commits for the first that fails what failed, and where the commits before it
  pass the whole check, they go live alone and you hear a `deployed` fact for them. You are told
  `deploy-batch-failed` naming that first failing commit with its item, worker and job, the commits
  held behind it and the files that broke; where nothing could go alone, it names every commit with
  its item. That failure is the next thing to fix, before anything else goes: analyse it, file and
  run the repair as todo.md says, then deploy again.
- **A test that passed only when run once more is flaky,** and went live all the same. A `deployed`
  fact's `flaky`, and a job's report naming one out of its close's last line, name each such test:
  file an item to mend each that no open item already covers, as todo.md says.
- **Never deploy what a suite did not pass,** and never run `bmo deploy commit` or `bmo deploy main` by
  hand for the backlog: `bmo deploy backlog` holds the rules.
- **A commit you made yourself** writes no pending row, so the backlog finds nothing waiting and
  tells you `deploy-main-ahead`; `running` lists such commits under 「[올라갈 커밋]」 too. Put it
  live with `~/.local/share/bmo/deploy/bin/bmo deploy main --then '<why>'`: it checks main
  and deploys it, and refuses while commits jobs closed on wait, since `bmo deploy backlog` then
  carries yours with them. What it came to without going live — a refusal, a check or suite that failed — reaches you as `suite-deploy-stopped`, and a refusal
  over waiting commits is your cue to run `bmo deploy backlog`.
- The jobs' reasons become the deployment's purpose. They are English, as everything a job puts
  into git is; you say them in the room's own language, never as written. It backs the runtime data
  up, waits for your reply to go out, and before stopping hands you the purpose as a quiet instruction: say in your
  own voice that you are going down and why; handed it again once back, say that you are back. The
  chat sees your answer, those two lines and nothing of the deploy as written
  (`docs/voiced-notices.md`).

## Folding the worker sessions

- Command: `~/.local/share/bmo/deploy/bin/bmo worker fold`, run it after a deployment or a
  reload that reports the worker's prefix moved; that report is the signal.
- The prefix is everything assembled into a worker session's system prompt when it opens:
  1. the CLI's own instructions
  2. this repository's `CLAUDE.md` and every `CLAUDE.md` above it, and the machine's global one
  3. what the launch line hands over: the skills in `agents/worker/plugin`, the co-worker
     definitions in `agents/worker/coworkers`, `agents/worker/settings.json` and the web MCP server
- An item's instruction and the report trailer are the job's prompt, not its prefix; changing them
  does not count.
- One moved character ends every worker session's cache at once, and folding is then cheaper than
  clearing or keeping the record. The command folds only the sessions whose recorded prefix
  word moved, so it never throws away a sound session; `--all` only when something that word cannot
  see changed, such as a CLI upgrade.
- A job that changed the prefix in its own commit folds its own session as it ends; nothing is
  left for you.
- `docs/deploy.md` says what goes into the prefix.

## Reloading a document

- **What ships with the code is told by the deployment.** The lore, the persona layers and tones,
  the rulebooks, the agent documents and these procedures are read out of the deploy tree. An edit
  in the checkout reaches no session until committed and deployed, and the deployment then tells
  every session concerned itself; a reload of one run from the checkout is refused.
- **What is written while the bot runs is told at once.** A brain document (a knowledge chunk, a
  person document, a room's rulebook) is runtime data. The digest reloads what it writes; a hand
  edit is reloaded by whoever made it, the file named by its path under the brain:
  `~/.local/share/bmo/deploy/bin/bmo reload --path <file> [--path <file>...]`
- What happens depends on the document:
  1. A document a session opens with a tool (a knowledge chunk, a person document, the lore, one
     of these procedures) is one every session concerned is told quietly to re-read.
  2. The shared system prompt (the common chat documents, the three answer rulebooks and the base
     persona) is announced to nobody and rebuilt on the next turn; reload records that its prefix
     moved.
  3. Room rules, owner instructions, tones and room personas travel in the leading room context: a
     change refreshes that block on the room's next turn without replacing the common prefix. Run
     reload for these too.
