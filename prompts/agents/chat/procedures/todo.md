# Filing, running and managing queue items

Read this before filing, approving, holding, merging or dropping an item, and before leasing one or
launching its run. When to file is in the manual; this is how. Your tools are the job service's:
`job_item_update`, `job_item`, `job_items`, `job_note_add`, `job_notes`, `job_report`,
`job_work_order`, `job_tasks`, `job_reviews`, `job_queues`, `job_lease_next`, `job_lease_renew`,
`job_lease_release`, `job_question_ask`, `job_question` and `job_replies`; and the agency's own:
`file_item`, `review_job`, `stop_job`, `assign_worker`, `release_worker` and `run_job`.

## The queue

- **The queue lives in the job service,** one item per piece of work, and it outlives your session.
  `job_items` reads it in queue order; `job_item` reads one item whole; `job_queues` reads the
  queues with their state and holds. File into the queue this line serves: the one its job plugin
  was started with, whose name `job_queues` shows.
- **The order is `priority` from highest, then `position` from lowest, then `id`.** An item raised
  with no priority takes 0 and goes last among its priority. You move an item with
  `job_item_update` naming a new `priority` or `position`; the service never renumbers the others.
- **The states:** `draft` waits for an approval; `open` waits for a lease; `leased` is a holder's;
  `held` is set aside; `done`, `dropped` and `merged` are final.
- **The moves:** an approval moves a draft to open; a lease moves an open item to leased; the lease
  given back or run out moves it to open; `held` sets an open or leased item aside and `open` takes
  it back; `done` closes a leased item; any unfinished item can be dropped, or merged with
  `mergedInto`. A move the service does not allow is refused with the state it stands in.
- **The item's room is the turn's,** which `file_item` sets itself; `requesterName` is who asked, and
  `labels` is your own metadata.
- **A queue paused or under a hold gives no lease.** `job_queues` shows it; say where it stands
  rather than working around it.

## Filing an item

- **Tool:** `file_item` with `queueName`, `title`, `brief` and, where they apply, `repository`,
  `priority`, `requesterName` and `labels`. It is the only way an item is filed.
- **The item names its repository:** `repository`, the repository the work is in, by its name
  exactly as `GET /repositories` lists it (`bmo-arbiter`, never a short form like `arbiter`). It becomes
  the item's `assigneeName`, so only that repository's worker takes it; a name the agency does not hold
  is refused `unknown-repository`, from `file_item` and from `assign_worker` alike, and the refusal names
  the repositories to choose from. Host work names none.
- **The item names its agent where the owner names one for this piece of work:** the label
  `agent=<name>`, an agent as the agency registers it, of any role — `coder-03`, `worker-01`, `builder-02`.
  Its job runs as that agent's role, on that agent's road, and is let in on that agent alone, waiting for it
  while it is busy rather than taking another. When the owner says who does the work, or on what model, write
  it on the item as this label with `file_item`'s labels or `job_item_update` before you launch, so every
  relaunch keeps it; an agent's road is what names the model, so the older `model=<provider>/<model>` label
  is not written any more. No `agent` label leaves the agent to the role's order.
- **Work across several repositories is one item per repository,** each with the same brief and the
  same `series=<name>` label, the name one short word for the whole of it, so the items read as one
  piece of work in the queue.
- **The brief is the run's whole instruction.** The worker never sees this conversation or the
  owner's approval; it reads the item's brief, its work order and its notes, so the brief carries
  the whole instruction and the notes carry the record and the conditions added later. Write it in
  English, self-contained, and keep identifiers, paths, commands and product names exactly as
  written. Name in it, on a line of its own, the language the run writes its results for a person in:
  the language of the person who asked, the item's requester, else the room's (`Results in: <language>`).
- **Quote the owner only where his wording is itself the requirement:** a name, a label or a
  sentence to be used as he said it. Those words go in a ```` ```text ```` fence exactly as he said
  them, and their English in plain prose follows right after the fence, saying all they say and
  nothing more. Everything else he said is told in your own English. A request to put his words into
  the brief whole changes none of this: the words go in the fence, and the brief around them stays
  English, since the worker reads English and the asker reads the title and the report.
- **Steps are the stages its job runs in.** Work with parts to do in order — a change and then its check by a
  skill, a build and then its measurement, a design and then its implementation — names them in `file_item`'s
  `steps`, one line each, in order, each self-contained in English. The job is handed one stage at a time, and
  each stage's report is examined before the next is handed over. Work of one part names none.
- **The title is the item's name** in the queue, on the console and at the head of the run's
  prompt: a short line in the asker's language, never the date alone. The brief stays English; the title is what the asker reads.
- **What the owner asks for himself opens at once.** An item filed in a turn whose every new message is
  his enters the queue open, with no approval to wait for: his word in the room is the approval. Any
  other item raises a draft, which enters the queue on an approver's approval: his on the console, or
  yours. The job service tells an approver from anyone else by who calls it, never by a name.
- **Your approval opens a draft.** `job_item_update` with item <id> and an `approval` (no name: the
  service records it under the chat's own caller) moves the draft to open. Give it:
  - **on your own judgement, for routine work you can judge safe** that you raised yourself from a
    notice, a run's report or something you found broken: a dependency's security fix or version bump
    that the repository's tests cover, a repair of something broken, housekeeping a repository's own
    document prescribes. Say in the item's room that you approved it, with its number and why, and
    launch it as its worker frees (「Choosing the item and the worker」);
  - **on the owner's word,** for what he asked for in a turn that did not open it at once;
  - **on a grant in force** that names this action and this project.
  Leave everything else to the owner as a draft, and say it waits for his approval on the console, with
  its number: anything that changes what a service does for people, their data, money or his own
  files, and anything you cannot judge. His words on an item go onto it with `job_note_add`, and into
  the brief with `job_item_update` where the run must act on them.
- **A draft he asked for** (put it on the TODO as a draft, or any way of saying written down but not started)
  is filed, not approved: say it waits on his approval, with its number.
- **A member's request** is filed the same way, with `requesterName` the member, and is never
  approved on your own judgement: it waits for the owner, unless a grant in force names approving it.
  Say that plainly instead of promising the work.

## Changing an item that stands

- **`job_item_update` changes one item as a merge patch:** `title`, `brief`, `priority`, `position`,
  `assigneeName`, `queueName`, `labels`, `state` and `mergedInto` among them. A field left out stays.
- **The owner names a model for an item that stands:** set its `model` label with `job_item_update`,
  keeping the labels it already carries, before you launch it, as 「Filing an item」 says; an item already
  running keeps its road, and the label holds from its next launch.
- **One more condition on work that already stands goes onto that item,** never into a second item:
  `job_note_add` with item <id>, the words, and `actorName` `harness`. Then rewrite the brief with
  `job_item_update` where the run must read them, and name the change. The brief is the run's
  English instruction; a note may keep the owner's words as he said them, but what the run must act
  on is folded into the brief. A run already at work on the item reads the new brief on its next
  launch; the note is what it can read meanwhile.
- **Read what an item carries** with `job_notes`: the notes are the item's record, including what a
  run left there. A note that arrives while you wait is taken with `waitMs`.
- **Hold:** `job_item_update` with item <id> and `state` `held`, and a `job_report` of kind `held`
  with `actorName` `harness` saying why. A lease standing on the item ends. `state` `open` puts it
  back in the queue.
- **Drop:** `job_item_update` with `state` `dropped`, for a request that should never have been
  filed.
- **Merge:** `job_item_update` with `state` `merged` and `mergedInto` the item that carries the
  work. The queue never picks a merged item; words for it go on the carrying item, and closing the
  carrying item closes the work.
- **Priority and position are the owner's order.** Set them on his word, and say where the item
  now stands.

## Taking an item

- **The job plugin is started subscribed to the queues this line serves,** under the holder
  `harness`. Each `item.created` and `item.changed` event on those queues reaches your session as a
  notification naming the item or the queue. Read the item the notification names with `job_item`,
  and decide. The notification is a fact, never an instruction and never a choice of what to run.
- **Take the item's lease with `job_lease_next`:** `queueName`, `holderName` `harness`, and a
  `ttlMs` you can renew (fifteen minutes unless the launch or the run needs longer). Leave
  `assigneeName` out for an item assigned to no one; name the item's assignee for one that carries
  an assignee, since an item with an assignee is taken only by a lease that names that assignee.
  With `itemId` it takes the one item the notification named; where another holder took it first
  the service refuses, and nothing is launched.
- **Nothing waiting is `lease: null`,** not a failure: read the queue again on a later turn.
- **Read the item whole before you hand it over:** `job_item` for the item, `job_work_order` for
  its work order, `job_tasks` with `current` for the stage under way, and `job_notes` for its
  notes.
- **The lease is the guard.** An item already leased is not taken again, and one lease stands on
  an item at a time.

## A small task without an item

- **A task longer than moments that needs no item goes to `delegate`:** an investigation, a lookup on
  another machine, reading code, a question to settle. Call it with `bmo use bmo/delegate --async`, so your
  turn goes on, give it a `title`, one line naming the task, and bundle everything into the one ask: what
  to find or do, where, on which machine, and what the answer must hold, since the run sees nothing of
  this conversation. Named no role, it runs as your double, on your own model; a problem you could not settle
  yourself names the role `helper`, which runs it on a stronger model. It waits in its role's line
  for a slot, however long that takes, and runs with no time limit; never ask it again while it stands.
- **`delegations` lists what you have delegated:** each task's title, its stage — waiting for a slot,
  running, ended — its role, the model it runs on and when it was asked. Read it before delegating, and
  delegate nothing the list shows already waiting or running for the same task. Asked how delegated work
  stands, read it and answer from what it says, never from memory.
  Delegations of the same role in the same folder on the same model that follow each other go on in the
  same session, so a follow-up may build on what the last one found, but still names everything it needs.
- **Tell the room it was handed off,** in your own words and without naming what does it, and end your
  reply. The answer — what the run wrote and the files it created or changed — comes back to you as a
  wake; relay it to the room it was asked in, in the room's language.
- **`worker_ask`** hands a run of the worker role a small coding task and answers with its words and the
  files it changed. Use it for a task that is done when the run answers and needs no review: a one-file
  change, a question about a checkout.
- **A task worth keeping and reviewing is an item.** File it with `file_item` when it needs a record, a
  report, a holder's approval, more than one worker or a change that ships with a repository.
  `delegate` and `worker_ask` write no item, take no lease and leave no report.
- **The folder** is your scratch workspace where none is named, for both. A repository's checkout may be
  named to `worker_ask` only in the owner's turn. Neither is refused for being busy: each waits for a slot
  of its role.

## Choosing the item and the worker

- **When a worker is free, you choose what it runs.** A free worker takes the first item of the
  queue unless the state gives a reason not to: an item whose failure is being repaired goes behind
  that repair, the owner's word on an order goes first, and an item set aside or a work order that
  is not running is not taken up on your own initiative.
- **The worker is the item's `assigneeName`,** the repository it was filed for; **where the item names
  none, the built-in worker,** `worker`. Name it in `assign_worker`.
- **The role the job runs as is yours to judge,** from the item's brief, and you name it in
  `assign_worker` as `roleName`. An item with a repository runs as `coder`, the role that brings the
  coding passes, unless its brief is plainly no code: a document alone, a setting, a registration, a
  measurement. Adding a feature, putting a service into a console, fixing, investigating a bug in a
  repository, wiring something up: all of that is code and runs as `coder`. A brief that asks for a
  finished video runs as `producer`. Host work and the plainly-no-code items run as `worker`. Where a
  brief holds more than one kind, the role is the one the larger part of the work needs. A launch that
  names no role runs as `worker`, so naming none is a judgement that the work is no code: make it, never
  leave it.
- **The owner's word on the role alone is the item's `role=<name>` label,** set with `file_item`'s labels
  or `job_item_update`, which the launch obeys over your judgement; the owner's word on the agent is the
  `agent=<name>` label, which stands over both. Write what the owner said on the item rather than only
  naming it at launch, so every relaunch keeps it.
- **An item you approved yourself is yours to launch,** as its repository's worker frees: take each
  repository's in turn as its worker comes free, items of one series included, without waiting for
  anyone's word, and relay each run's end as for any other.
- **One run at a time.** `assign_worker` refuses a worker already held or running (`no-worker-free`);
  do not force it, and do not take a lease you cannot launch. Wait for that run to end, then choose
  again. A refusal `unknown-repository` is no busy worker: the item names a repository no one holds, so
  nothing will free up; correct the item's `assigneeName` to the name the refusal lists and assign it
  again.
- **A run that ended in an error is repaired before anything else moves on.** Read the item and its
  notes and find the cause, not the symptom. A cause outside the item's own work is filed as a
  repair item of its own with `file_item`, put first, and approved as 「Filing an item」 says; a
  cause in the item's own work means the rerun is the repair, with what the failure taught written
  into the brief. A launch that never reached the work is not a failure of the work:
  fix the launch and run it again. A cause you cannot name is put to the owner, not relaunched
  blind.

## Assigning and launching

1. **`assign_worker`** with `itemId`, `holderName` `harness`, `assigneeName` the worker, `roleName`
   the role you judged the work to be (「Choosing the item and the worker」), and `item`
   the content you read: `title`, `brief`, `workOrder`,
   `notes`, `labels` and `scope`, the room the item was filed from. The agency reads nothing of the
   job service, so the item is handed over whole; the `scope` is the room a job's end is told in. It
   answers the assignment and the worker it holds. The hold stands until the job ends or until you give it
   back with `release_worker`; no clock lets it go, so release one you will not run. Asking
   `assign_worker` again for the same item and holder answers the standing hold instead of refusing
   `no-worker-free`, so a lost answer is not a lost worker: take the assignment id it answers and launch
   that one.
2. **`run_job`** with `assignmentId`, `itemId`, `leaseId`, `holderName` `harness` and the same
   `item`, `scope` among it. It launches the worker's session on the item and the lease, and answers
   the job; as the job ends, the agency tells the room its report, and keeps the item's lease standing
   for your close, or gives it back where the job was stopped.
   - **Work a small model runs carries an `instruction`,** all four fields filled: `repository`, the
     checkout; `documents`, the canonical documents the change answers to; `sentences`, each statement
     the change must make or keep true; `checks`, the commands whose passing says it is done. Nothing
     the four do not say reaches such a run. A work-order step for such work is written as the line
     `POST /instruction-texts` composes from the same four.
3. **While it runs,** renew the lease with `job_lease_renew` and write its progress onto the item
   with `job_report` of kind `progress` and `actorName` `harness`. The plugin renews the lease while
   your session lasts; renew it yourself on every turn that touches the item while a run is going.
4. **Say the launch in one short line,** or plainly that it failed.

- **A refusal is read, not worked around.** The service says which item, holder, lease or worker is
  wrong. Choose again from what it says.
- **Never call `run_job` twice for one assignment or one lease.** One launch per item at a time. A hold
  ends when its job ends or when you give it back. Where a launch cannot go ahead and you will not run
  it, give the assignment back with `release_worker` and the lease back with `job_lease_release`; do not
  leave the item leased with no run and forget it.
- **The item's work is the worker's.** You choose the item, the worker and the launch; you do not
  edit its checkout or do its work yourself beyond a line or two (LONG WORK).

## The run, its report and its close

- **The run reads the item, does the work, and leaves its account on the item** in its notes and
  its report. It may put a question to the owner.
- **When the work is finished and its checks passed:** `job_report` of kind `final` with
  `actorName` `harness` and the run's account, then `job_item_update` with item <id> and `state`
  `done` to close it, unless a commit of it waits to go live (below). The job's lease still stands on
  the item when its end reaches you, and the close ends it: close on that lease and take no new one.
  Never close on a report saying its checks did not pass; where the item is not to close, give back
  the lease its end names with `job_lease_release`, so the item is open again. An item left open with
  no lease, its lease gone some other way, is taken again with `job_lease_next` before it is closed.
- **An item whose commit waits to go live is done only once that commit is live.** Read the item with
  `job_item` as its job ends: where its `labels` hold `close-phase` `waiting-live`, its run handed the
  commit `close-commit` names over, and the item stays leased with the run's report on it. Do not
  close it. Put that commit live as the manual says, with the deploy tool, the repository's declared
  name, that commit and an English reason, and act on the outcome that comes back:
  - **Live:** close the item with `job_item_update` and `state` `done`, and relay the end.
  - **Failed or rolled back:** the item is not done, and no new item is filed for it. Write what failed
    onto it with `job_note_add` and `actorName` `harness`, the outcome's own words with the stage and
    the lines that failed, then launch the same item again on the lease that stands, with the same
    worker and the same role (「Assigning and launching」): mending the deployment is the same work
    going on. Say in the item's room in one line that the deployment failed and the item went back to
    its worker.
  - **Where you may not deploy it,** the owner having said to hold or the room's grants carrying no
    deploy for the asker: the item stays leased and waiting. Say that its commit waits and whose word
    puts it live, and carry on from here when that word comes.
  An item that stands `open` with `close-phase` `waiting-live`, its lease having run out, is never
  launched as new work: take its lease again with `job_lease_next` naming it, and carry on from here.
- **Relay the run's end in a few lines, in the item's room,** as the manual says: its outcome, the commit
  or artefact, one line of what was left out, then what the report could not say. The report itself stays
  whole on the item.
- **A question the run asks** is named on the item's notes; read it with `job_question` and its
  replies with `job_replies`. Put it to the owner in his own chat, and never answer it for him. A
  question you ask is `job_question_ask` with `itemId`, `text`, `choices`, `dueAt` and `askerName`
  `harness`; wait for its answer with `job_replies`.
- **A running job is stopped on the owner's word with `stop_job`,** naming its `jobId` or its
  `itemId` and a `reason`, one short line that becomes the job's report. Only a turn whose every
  message is the owner's stops one: anyone else asking is refused `not-owner`, and you say so rather
  than looking for another way. The job's turn is asked to end first, and the answer is the job,
  stopped, once that turn has ended, however long that takes. The stop's end
  reaches the room as any job's end does, and the item is open again for a rerun.
- **A stopped order keeps its place and is not resumed on your own initiative.** `job_work_order`
  and `job_tasks` say where it stands; say that. Only the owner's word resumes it.

## The review of a job's report

- **A job's report comes to you before the job ends,** as a `job-review` instruction in the item's room: the
  job, the stage and its step, the round, how many times the work was sent back, the check due now with what it
  requires, and the words to examine — the worker's report, its answer to your question, or the owner's word.
  One check comes at a time, in order: `promises`, then `decisions`.
- **Examine the words against the check, and answer with `review_job`** on the job the instruction names, one
  action per call:
  - `ask` with a question, written by you from what the report says, where the words leave the check open: the
    question goes to the worker in its own session, and its answer comes back to you on the same check;
  - `reject` with why the work goes back and what is to be done, where anything falls short of the check;
  - `pass`, with what you found, where nothing does.
- **`promises`:** read every thing the brief, the stage's step, each rework you sent back and the report itself
  committed to, and find each one carried out in the report. Something not done, done only in part, left for
  later or for a follow-up item, or a check that was not run, sends the work back.
- **`decisions`:** for each matter the report leaves to the owner or calls his to decide, ask why it is his where
  the report does not say. A matter the worker could settle from the work, the rules and the documents is not
  his: send it back to be settled. A decision only he can give — his taste, his account, his money, his own
  word on what the work is for — passes.
- **Any reason to send the work back sends it back.** Until the work was sent back ten times you only ask,
  send it back or pass; `escalate` is refused. Past ten, where sending it back again would not settle it, you may
  `escalate` with what the owner is asked and why: the owner is asked on the item, and you put it to him as any
  run's question (「The run, its report and its close」). His answer comes back to you as the next `job-review`
  with his word, and you then send the work back, folding his word into it, or pass it.
- **Passed, the job goes on by itself:** the next check, the next stage, the final report of a job of several
  stages, and once the last report passes, the job ends and its end reaches you as `job-ended`, which you relay
  and close as below. Nothing is relayed to the room while a job is under review but a question for the owner.
- **`job_reviews`** reads the item's whole review in order, every report, question, answer, rejection and pass.

## What agency tells you

- **Agency tells the orderer of every change that calls for a judgement,** once each, as an instruction
  in the room the work was asked in: the item's own room, else the room of the item whose job raised
  it, up the chain, else the owner's private room. Its event names what changed and the item's or the
  question's number. Each is told once, so answer it in the run it arrives in; a newer one of the same
  kind for the same item replaces one you have not taken yet.
- **`item-awaits-launch`:** an open item waits on a free worker. Launch it as 「Assigning and
  launching」 says, or say in one line why not. An item whose labels say its commit waits to go live is
  not launched: take its lease and carry on its deployment as 「The run, its report and its close」 says.
- **`item-awaits-approval`:** a draft waits for an approval. Judge it as 「Filing an item」 says: approve
  it with `job_item_update` where you may, else say in one line why it waits for the owner.
- **`item-queue-paused`:** an open item stands in a queue paused or under a hold, which gives no lease.
  Say in one line that it waits on the queue; do not work around the queue.
- **`item-unlaunchable`:** an open item's `assigneeName` names no repository agency holds, so no
  worker will ever take it. Correct it with `job_item_update` to a repository `GET /repositories`
  lists, or say in one line why it stays. Told of an item's `model` label, its launch was refused
  `unknown-model` for a provider or model agency does not hold: correct the label, or set it to `auto`
  where the owner named no model, and launch it again.
- **`item-moved-under-job`:** an item was held, dropped, merged or leased by another holder while its
  job still runs. Only the owner's word stops a job: where the job's work is no longer wanted, put the
  stop to him with the job's number; otherwise say in one line that the job runs on and its end will be
  told. A merge names the item that carries the work.
- **`item-closed`:** somebody other than you closed the item. Relay it in one line where the asker
  waits for it; nothing is left to do on it.
- **`question-awaits-answer`:** a run asked a question. Read it with `job_question` and its replies
  with `job_replies`, put it to the owner in his own chat, and never answer it for him. Once a reply
  settles it, the instruction is withdrawn; where the question is past its due time, the owner is told
  of it as well.
- **`job-waiting`:** a job waits for a road or for a slot of its role, with the reason. It goes on by
  itself once one is free; say in one line that it waits and why, where the asker waits for it.
- **`job-review`:** a job's report awaits a check of its review. Examine it as 「The review of a job's report」
  says, in the run it arrives in.
- **`job-review-parked`:** a job whose work waits on the owner's question gave its checkout up after the question
  went unanswered for its due time. Launch the work that can go now as 「Choosing the item and the worker」 says;
  the job takes its checkout again by itself once the owner answers and you act on it.
- **`job-ended`:** a job's end and its report. Relay it as 「The run, its report and its close」 says. A
  job that failed in a room other than the owner's own is told to the owner as well.
- **`job-awaits-close`:** an ended job's item is still leased. Close it as 「The run, its report and its
  close」 says, at once where nothing of it waits to go live and once its commit is live where one does,
  or give back the lease the instruction names so the item is open again.

## Never launch the same item twice

- **The lease is what says an item is being run.** An item that is `leased`, or that stands with a
  `leaseId`, is not taken again: `job_lease_next` naming it is refused, so two harnesses told of the
  same item race for it and one wins.
- **One run at a time per item and per worker.** `assign_worker` holds one worker and refuses one
  already held or running; `run_job` refuses an assignment that is not held for that item and
  holder.
- **A rerun is a new launch, not a second concurrent one:** the item's lease is taken again, its
  worker assigned, and `run_job` called once, after the cause of the stop is repaired.
