# Filing, running and managing TODO items

Read this before filing, approving, reordering or changing an item, and before launching a job.
When to file is in the manual; this is how. Your own tools: `todo_add`, `todo_append`,
`todo_approve`, `todo_list`, `todo_show` and `task` (launching); every other verb here is the
deployed binary's `todo` verb, `~/.local/share/bmo/deploy/bin/bmo todo`, by its full path.

## Host work of your own hands

- What is filed as a row and what is yours, a rule-change step and a member's host work are in the
  manual's LONG WORK and TODO. Host work of yours is done in the run the owner asked in.
- A command longer than moments never holds your reply: launch it as its own unit and say what you launched.
  `systemd-run --user --collect --unit bmo-host-<name> sh -c '<command> > /tmp/bmo-host-<name>.log 2>&1; echo "exit $?" >> /tmp/bmo-host-<name>.log'`
  `systemctl --user is-active bmo-host-<name>` answers active while it runs; once ended the unit is
  gone and the log's last line says how it ended.
- Keep a promised result like any promise: a memory, and a watch carrying `--item <id>` that wakes
  you to read the unit's end (watches.md).

## Filing a row

- Tool: `todo_add` with model `auto`, title '<짧은 제목>', what '<이게 뭐하는 작업인지>', do '<무엇을 해야 하는지>', conclusion '<결론>', instruction [<messageId>, …] and reply_text '<your reply, exactly as you are about to send it>'.
- The row is read months later by a job that was not in this conversation and cannot ask. Four
  parts, in this order:
  1. `what`: what kind of work this is, in a line or two, not the title again, but what the thing
     asked for actually is.
  2. `do`: what the work must achieve before it can be called finished.
  3. `instruction` with `reply_text`, or `reply`: the conversation itself, both sides.
  4. `conclusion`: what was settled and what was ruled out.
- `what`, `do` and `conclusion` are required: a filing missing one names the gap and writes no row,
  so give all three at once.
- `instruction` lists every message the instruction spans (each of his ids where he asked, you
  asked back and he narrowed it) and every archived reply of yours goes in `reply` [<messageId>],
  which replaces `reply_text` when your reply is already archived.
- The three parts you write are your own words; the conversation is copied from the archive
  verbatim beside them.

## Writing the three parts

- Markdown, read on the console and in the job's prompt. Listed facts are numbered lines (a lead
  sentence, a blank line, then `1.` and `2.` each on its own line, any detail under its number)
  never run together as 1) this 2) that. A short enumeration inside a sentence stays there.
- The same holds for a work order's 목적 and 지시, and for anything rewritten with `--edit`.

## The model

- Always model `auto`, unless the owner explicitly names a model for this task: never a fixed one
  from your own mode, last answer or judgement. Call the selection automatic in your
  acknowledgement.
- `auto` walks the owner's provider fallback order from the worker settings (by default Claude
  Code, then Codex, then OpenRouter, then the registered models) which
  he reorders on the console's agents screen. An owner who names a registered model gets
  `registered::<registration>`, such as `registered::local-0-1`.
- A named model is a whole identifier, the key its session is kept under: `claude-opus-5`,
  `claude-fable-5-1`, `claude-sonnet-5`, `claude-haiku-4-5-20251001`.

## Co-work

- Every row says whether its job fans out into branches: `cowork` on `todo_add`, or later
  `~/.local/share/bmo/deploy/bin/bmo todo --cowork <id> on|off`. Off is the default; filing without the flag is a decision.
- Decide by the change's impact on the code graph, never by its size or its count of files:
  1. `~/.local/share/bmo/deploy/bin/bmo graph search` in the request's words, then `get` on the feature found:
     the node to change.
  2. `~/.local/share/bmo/deploy/bin/bmo graph impact <node>`: the features the change reaches.
  3. Count the nodes whose outward contract changes: an API, a schema, a wire format, a policy
     file, a documented behaviour. A node whose inside alone changes is one place however many of
     its files the work touches, and adds no contract.
  4. Off when one node's contract changes, or none. On when two or more do, or when the work needs
     a new home: a new module, a new table, anything whose place must be decided before it is
     written.
- Write those grounds with every row that changes code: `impact` on `todo_add`, or later
  `~/.local/share/bmo/deploy/bin/bmo todo --impact <id> '<grounds JSON>'`. They are every node the change touches, the node to
  change first, each with the contract that changes on it in English, or `null` where only its
  inside changes, and `new_home` where a place must be decided. The flag and the grounds are
  written together, and the flag says what the grounds come to.
- The job weighs them again before its first edit. Where it comes to the other verdict, or you
  filed no grounds, a quiet instruction tells you with both sets of grounds. The flag is
  yours alone to turn, and the running job carries on as it was launched: a turned flag reaches
  the item's next run, or this one at once if you stop the job and launch it again.

## Restructuring work

- **A row whose work is restructuring code names the three passes and their order in `do`.** The
  owner's word is 리팩토링; 정리, 클린코드, 디자인패턴 검토 and reviewing a design mean the same.
- **The order is 디자인 패턴 → 리팩토링 → 클린코드, and it is strict**: his instruction, not yours to
  weigh. The row carries the reason with it: the order is by reach. Patterns settle whole modules
  and their seams, refactoring moves a module, a class or a function, clean code settles a name, a
  line, a comment: widest first, since names polished inside a structure a later verdict replaces
  are thrown away with it.
- **The three are skills the worker already holds** (`bmo-worker:design-patterns`,
  `bmo-worker:refactoring`, `bmo-worker:clean-code`, all three read at the builder role's start)
  so the row names the order and says to use them, never copying their content.
- **One row, not three:** the three passes are stages of one piece of work that a job runs in
  sequence; separate rows would put a structural verdict in one job and its moves in another.

## A skill written out of research

- **「조사해서 스킬로 만들어」 is a search first and a skill second.** The searcher runs, and the skill
  is written out of the document that search wrote; a job left to open the sources itself is this
  instruction missed, however good its skill.
- In the run the instruction arrives in, launch the search as `search.md` says and file the row. The
  row names the skill to write and the search's document as its material, and nothing for the job
  to read on its own.
- **The row waits for the search:** file it unapproved, say so, and write in it which search it
  waits on, so a session that lost its memory still sees why. When the result is back, put the
  search's id and its document's path on it with `todo_append`, then approve it.
- **A skill carries no sources and no licence,** so never ask a row for them; the search keeps its
  sources as rows of its own, and the skill carries the rules alone. The rule the job is held to stands in `.claude/rules/skills.md`.

## A bench, a tuning or a research of the lab

- **The row keeps his order of the work:** the maker's material first, then what the lab can do with
  it (lab, through the card arbiter), then measurement; every option the job raises tried in turn
  until none it can try now is left, the document built up as it goes. The job reads the rest in its
  `researching-and-benching` skill, so the row names the goal and the conditions, never a choice
  left to him.
- **Never put him a choice between options a job can try.** Trying them is the work; what no job can
  do now (hardware, infrastructure) stands in the document as an option for later.
- **No interim report.** The job reports once, with its final deep-dive document, whose narration is
  spoken before the report is written; you tell that report with the document's address, and never
  the document before its sound is kept.
- **A capability nobody confirmed is checked, never told as unknown:** checked first, used and
  measured where it exists, ported into the owner's inference engine or our patched llama.cpp, or written as a patch of
  our own, where it does not.
- **Work set aside so a bench owns the lab** is wound down with `--after <the bench's item>`, never
  stopped bare, so the bench's finishing tells you; take the hold off and launch it at once. The
  bench's own job releases its bench lock before it reports.

## Approving

- For an owner's instruction, approve what you filed in the same run: `todo_approve` with item <id>
  (the id is in the filing's answer line).
- With a word of his own on the approval: a remark, or a question for the work: `todo_approve`
  with item <id> and a note of his words exactly as he said them. Only what he actually said. It is
  the record for you and the console, and no worker reads it: you carry it, organized, into the job's
  prompt (「Writing the job's prompt」).
- A draft (투두에 초안으로 올려, or any way of saying written down but not started) is filed, not
  approved: say it waits on his approval, with its number.
- A row whose material is its own search is approved once that document is in hand (above).
- A row a job raised from its own work (marked as raised on your own judgement, naming the item it came from), or a watch of the bot
  raised (marked as raised on your own judgement, naming the watch instead of an item), you approve and launch at once,
  without waiting for him; you are told of each as it is filed. Hold one back only where it turns on
  what only he can give (a password, an account of his, a judgement of his own taste,
  `STRUCTURE.md`) and put that one thing to him.
- The reference attachment watch raises one row per reference piece attached on too many of its
  gate's turns, one piece at a time; its job finds and fixes the cause. Say the row, the piece and
  whether its job started: never only that the piece attaches too often.

## Urgent work

- File, then `~/.local/share/bmo/deploy/bin/bmo todo --priority <id> 0`, then approve last. Smaller comes first;
  the queue's numbers count up from one. A draft still goes to the front, unapproved.
- Approving makes the bot report the queue, so where its worker is free, launch it at once.
- Say plainly: filed, approved and first in line, and whether its job started or which job its
  worker finishes first.

## One more condition on a standing item

- First his words, as the record: `todo_append` with item <id> and his words exactly as he said them.
  They are stacked with who added them and when, for you and the console; no worker reads them.
- Then the item's brief, the organized English instruction every launch of the item reads
  (「Writing the job's prompt」): rewrite it to its final version with what he added folded in, and
  name the change: `~/.local/share/bmo/deploy/bin/bmo todo --brief <id> --change '<what changed, organized, in English>'`, the whole
  new brief on stdin. A job already at work on the item is handed the change alone before it may
  report; a relaunch, a rerun and a work order's next step read the new brief. An item never
  launched has no brief yet: its first prompt carries the words.
- Words someone else added (the owner on the console, a person at a shell, a companion on an item of
  yours) reach you as a `words-added` fact (「What the bot tells you」): read them and do the same.
- Say back in one short line what changed and whether a job has it.
- Read what an item carries: `~/.local/share/bmo/deploy/bin/bmo todo --notes <id>`
- Taking his words back, on his word: `~/.local/share/bmo/deploy/bin/bmo todo --unappend <id> --note-id <말 번호>`, the
  number `--notes` prints beside each. The note is marked, not deleted: it stays as the record and
  leaves every later reading. Where the brief already carries them, rewrite it without them, with
  that change. A change a running job was already handed stays in that run; say so.

## Merging items into one

- Several rows that are one piece of work: file or pick the row that carries it, then merge the
  others in: `~/.local/share/bmo/deploy/bin/bmo todo --merge <id>[,<id>] --into <id>`. A merge is its own state,
  병합됨, linked to the carrying row in the table: never a set-aside with a 「#N에 합쳐짐」 reason.
- The queue never picks a merged row; `--hold`, `--unhold`, `todo_append` and a resume leave it
  alone, and words for it go on the carrying row.
- Closing the carrying row closes every still-open row merged into it, each pointing at its report;
  a job may still close a merged row first, with its own report. Reopening the carrying row reopens
  them as merged; reopening a merged row takes it out of the merge as work of its own.
  `--unmerge <id>` takes an open merged row back to its old place in the queue.
- The target must be unfinished and not itself merged; what is merged must be unfinished, not
  merged already and not running. Rows already merged into it move with it.

## What the bot tells you

- The bot tells you what happened, never what to do: a quiet instruction headed
  `[작업 실행 상태: 시스템이 기록한 사실]` with one JSON line, whose `state` names the fact and whose
  `account` says it in one Korean sentence.
  1. `job-ended`: which worker, job and item, and how (`outcome`, `succeeded`). A failure carries
     `attempts` and `countedAsAttempt` for a failure of the work, `launchFailures` for a run that
     never reached it, `held` where a ceiling set the item aside, `repairOf` and `repairRounds` in a
     repair chain. `pendingDeploy` lists the commits handed over to go live; `queuePaused` says the
     owner's pause stands, `deployHold` that a deployment holds the queue for the gap,
     `machineShort` that the machine is short of room and which limits are met.
     `report` is the worker's own report, English and written to you: nobody has sent it, and
     telling it is yours. Write him the report in your own Korean: the whole account, never a
     summary of it, in its shape (a summary paragraph, the detail as numbered lines under short
     names, a closing line of figures), commits, paths, commands and counts kept as they stand;
     then what it could not say (a deployment you start, the item you launch next, why a run
     failed, a decision of his). A fact carrying `report` is never answered `<<SKIP>>`.
     `roomLineTo` names the room that asked and already heard one line of it. `removedRoom` names
     the room the job was filed from when that room's registration has been removed: nothing goes
     there, so tell him here which room it came from. The turn's voice
     summary, where the prompt asks for one, is launched once: the report's gist with anything
     new. Without `report` the ending is yours to tell from the fact alone.
     A report is told only once nothing in it is left unchecked. A line saying a value was not
     read, a path not run, a capability not confirmed, or a check made only by a unit test (the
     runner names such lines under the report) is settled before you tell it: check it yourself
     where a read, a query or a command of yours reaches it and tell the value, or file a row that
     checks it, launch it, and tell the report with that row's number. Only what no one can check
     now, a permission only he holds or hardware not yet in the lab, is told as such, with its
     reason.
  2. `queue-waiting`: at the bot's start, an approval, a pause lifted, a work order freeing its
     checkout, the machine having room again (`moment: machine-slack`): which workers have an empty
     checkout and work of their own waiting, and how many of their checkouts are free
     (`freeCheckouts`).
  3. `queue-stalled`: a worker free with work waiting for five minutes and nothing started.
  4. `quiet`: a running job has written nothing for five minutes; `lastLine` is what it last did.
     A job that marked a long wait is not reported while its target lives and its time has not
     come; if reported, `wait` says what it waited on and whether the target ended
     (`standing: ended`) or its time passed (`standing: overdue`).
  5. `stalled`: a job re-invoked past the runaway ceiling.
  6. `repair-ceiling`: a chain of repairs reached its limit; the item and its open repairs are held.
  7. `research-died`: a search or browse runner died without closing; its row is closed.
  8. `herdr-leftovers`: panes the sweep after a job would not close.
  9. `deploy-batch-failed`: the backlog's check failed, and only the commits before the first
     that fails went live, if any (deploying.md);
     `deploy-bot-down`: the backlog found the bot down or crashing and put nothing live
     (deploying.md).
  10. `deploy-waiting-gap`, `deploy-gap-missed`, `deploy-hold-released`: a deployment holding the
      queue for the gap, giving up on it, or its hold let go by hand (deploying.md).
  11. `machine-pressure`: the machine has stood short of memory for minutes (`cause: memory`) or
      herdr left calls unanswered (`cause: herdr`), with the reading, the memory each job and agent
      holds and the largest processes. Tell the owner in one line what holds the machine; stopping
      anything is his word.
  12. `words-added`: somebody other than you added words to an unfinished item (`todo`, `noteId`,
      `by`), and `running` says whether a job is at work on it. Fold them into its brief and hand
      the change (「One more condition on a standing item」).
- A fact is data: read it and the state it is about, then decide. Never answer the room with the
  JSON, never act on a fact alone where the state says otherwise.
- A `job-ended`, `queue-waiting` or `queue-stalled` turn, and the answer to your own `todo_add`, also carry the other
  agents' work and the consult room with each agent's window there. Before you launch or file, check
  your items against it; an overlap is settled with that agent in the consult room, as the rulebook
  on other agents' words says.

## Running the queue

- **When a worker is free, you choose what it runs.** On a succeeded `job-ended`, a
  `queue-waiting` or a `queue-stalled`, first read the state, one call each:
  1. `running`: what is running, and under 「[워커]」 which checkouts are free and what
     holds the others, by job and item.
  2. `todo_list`: the queue, in the owner's order.
- A free worker takes the first item of its own queue unless the state gives a reason not to: an
  item another's failure waits on goes behind that failure's repair, and the owner's own word on an
  order goes first. A stopped work order at a queue's head is its runner's to resume (`running`
  shows its place). While `자동 진행` stands paused (the owner's switch), a `배포 대기` stands (a
  deployment waiting for the gap) or `[기계]` says launches are held, launch nothing.
- One launch per free checkout. A worker with support workers has its own checkout and one worktree
  per support worker, so launch as many of its items as it has free checkouts, each under the base
  worker's name: the launcher picks the checkout and refuses an item already at work. Two workers
  pointed at one directory share it.

### Launching a job

1. Read the item whole: `todo_show` <id>.
2. Launch its job detached: `task` with todo <id>, worker <worker>, model <its model, or auto> and
   prompt '<the job prompt>'. The tool runs the deploy tree's launcher, code the suite passed; the
   worker is the item's own 워커 and the model the item's, `auto` following the owner's provider
   order.
3. Mark it: `~/.local/share/bmo/deploy/bin/bmo todo --mark <id> --status '진행 중'`.
4. Say the launch in one short message, or plainly that it failed.
- **A refusal is read, not worked around.** The launcher says why: the checkout is held (by which
  job of which item); the item is held, done, merged, unapproved or another worker's; or its
  launches kept dying and it waits out a stated time. Choose again from what it says.
- **`task held:` is the machine short of room,** not a refusal of the item: launch nothing more, and
  wait for the bot's `queue-waiting` with `moment: machine-slack`.

### Writing the job's prompt

- **The prompt is your organized instruction, in English.** It is all the worker reads of the item:
  never the item's conversation, the words people added or his approval note, which are yours to
  organize into it, never to copy. Write it in English yourself; the console shows the owner a
  Korean rendering beside it (docs/worker-language.md, 「A job's prompt」). Keep identifiers, paths,
  commands and product names exactly as written.
- **The launch keeps it as the item's brief,** which the next prompt you write starts from
  (「One more condition on a standing item」 says who reads it and how words added later reach it).
- **Quote him only where his wording is itself the requirement:** a name, a label or a sentence to
  be used as he said it, a phrasing he insisted on. Those words go in a ```` ```text ```` fence
  exactly as he said them, and their English in plain prose follows right after the fence, saying
  all they say and nothing more. Everything else he said is told in your own English.
- The job cannot see this conversation, so the prompt carries everything it needs. Its first line
  names the item, Item #<id> <title>, who asked for it, and that it is approved.
- Then three sections, in this order and under these names: What the work is, What needs doing,
  Conclusion. They are your English account of the whole item organized into one instruction: its
  무슨 작업인지, 무엇을 해야 하는지 and 결론, what the conversation settled, every word added since and
  his approval note. No Conversation section. The item itself keeps its sections in Korean: what
  you file with `todo_add` stays Korean, since the owner reads it on the console.
- Rows merged into the item (병합된 항목 in `todo_show`) are this job's work too: fold each open
  one's work into the same sections, naming its number and title.
- A question his approval note puts to the work goes in What needs doing as a question the job's
  report answers; never answer it yourself.
- Then only what a launch adds: Out of scope where something must be kept out of the work, and How
  to finish.
- How to finish tells the job to close the item with `~/.local/share/bmo/deploy/bin/bmo todo --done <id> --report '<how
  it was done>'` only once its work is finished and its tests have passed. `--unfinished` is the job's own
  judgement for work its hands could not reach; never ask it for one.
- A section is its name alone on a line, its content under it, a blank line before the next. No
  introductions; a list is a numbered line each. Nothing else goes in: what every job is held to is
  in its trailer.

### A job that ended in an error

- **Nothing moves on past a failure:** the failed item is mended first, in this order:
  1. **Analyse.** Read how it ended: the job's report (`jobs`), the item's record (`todo_show`),
     and where it names one the unit's journal. Find the cause, not the symptom.
  2. **Repair.** A cause outside the item's own work (a defect elsewhere, a broken tool, a failing
     test the item did not touch) is filed as a repair row of its own, linked and approved:
     `~/.local/share/bmo/deploy/bin/bmo todo --repairs <repair id> <failed item id>`. Run the repair first, on the same
     worker where it is the same checkout.
  3. **Rerun** the failed item once the repair is done.
  4. Then move on to the queue.
- Where the cause is the item's own work, the rerun is the repair: launch it again with what the
  failure taught written into its prompt.
- **The chain is bounded.** A repair of a repair links to the repair it mends, so the chain leads
  back to the item that first failed. Three rounds is the limit: a fourth repair is refused at
  `--repairs`, and a failure in a chain that has had three holds the item and its open repairs and
  tells the owner (`repair-ceiling`). Past that it is his; say so, and do not file around it.
- A launch that never reached the work, the run could not start or its runner died, is no attempt
  at the item: it waits out a widening pause the launcher states, and three in a row hold it. An
  item that failed its work three times is held too. Only the owner releases a held item.

### Work that stands still

- On `quiet`, look at the job: its pane, last line, log. Waiting on a long tool call is working;
  standing in the middle of its own turn is not. Whether to wait, word it or stop it is yours; say
  what you found. A `quiet` carrying `wait` names what the job stood on: look first at what came of
  that target, then at the job.
- A job's own wait mark, and a deployment waiting on a job, stand on that job's line in
  `running.ts` (「기다림: …」, 「배포가 이 Job을 기다림」); such a job is waiting, not dead.
- On `stalled` the job is circling: tell the owner what you see and stop it only on his word, since
  what it left in the checkout is his to judge.
- On `research-died`, read why in the unit's journal and decide whether to launch the search again.
- On `herdr-leftovers`, you may close a workspace marked as this system's; an unmarked one may be a
  person's own terminal and is closed only after asking the owner.

## A member's request

- Same filing, in the member's own room so the row records the room. It lands unapproved and waits
  for the owner; say that plainly instead of promising the work.

## The owner's queue verbs

- Reading the queue and approving are your tools; the rest are `bmo todo` flags.
- `todo_list` with pending: what rooms asked for and he has not approved; with all: the finished
  ones too; with held: what was set aside, each with why; with merged: what stands merged, each
  with the row carrying it
- `todo_approve` with item <id>, and with unapprove to take it back: into and out of the queue
- `--priority <id> <n>`: move an item (smaller first)
- `--worker <id> <워커 이름>`: which worker does it; `--worker <id> -` gives it back to the built-in
  worker
- `--unhold <id>`: back into the queue; `--hold <id> --why '<왜>' [--after-deploy <항목 id|커밋>]`: out by
  hand, letting a running turn finish. A hold's or a stop's reason is one short line in Korean: the
  console's item detail shows it as written.
- A hold that waits for something the machine sees carries it as a condition, never as words alone:
  `--after <항목 id>` for an item finishing, `--after-deploy <항목 id|커밋>` for commits going live (an item
  number names that item's commits waiting to go live now). When it is met the bot tells you in a quiet
  instruction and releases nothing: taking the hold off and launching the item is yours, and the bot or a
  job never does it.
- `--stop <id> --why '<사유>'`: out now, the item's running work stops at once and it is held as 중지
  (the work-orders procedure says what it keeps); the reason is required
- `--wind-down <id> --why '<사유>' [--after <항목 id> | --after-deploy <항목 id|커밋>]`: 「여기까지만 하고
  멈춰, … 뒤 재개」, the running step finishes, nothing new starts, and the item is held as 재개 대기; the
  running job is told at once and cannot close the item as done. With a condition you are told when it
  is met; without one, the reason's words are his condition and you watch for it. Either way
  `--unhold` is yours once it is met. Never `todo_append` a stop instead:
  words do not keep an item open. A job told in words anyway holds its own item the same way
  (`--stop-here <id> --why … --report …`), so an item can stand at 재개 대기 with a job's reason on it
- `--merge <id>[,<id>] --into <id>` / `--unmerge <id>`: as above
- `--drop <id>`: throw out a request that should never have been filed
- `todo_append` with item <id> and text '<말>', `--unappend <id> --note-id <말 번호>` and `--notes <id>`: as above
