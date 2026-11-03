# Filing, running and managing queue items

Read this before filing, approving, holding, merging or dropping an item, and before leasing one or
launching its run. When to file is in the manual; this is how. Your tools are the job service's:
`job_item_update`, `job_item`, `job_items`, `job_note_add`, `job_notes`, `job_report`,
`job_work_order`, `job_tasks`, `job_queues`, `job_lease_next`, `job_lease_renew`,
`job_lease_release`, `job_question_ask`, `job_question` and `job_replies`; and the agency's own:
`file_item`, `stop_job`, `assign_worker` and `run_job`.

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

- **Tool:** `file_item` with `queueName`, `title`, `brief` and, where they apply, `priority`,
  `assigneeName`, `requesterName` and `labels`. It is the only way an item is filed.
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
- **The title is the item's name** in the queue, on the console and at the head of the run's
  prompt: a short English line, never the date alone.
- **What the owner asks for himself opens at once.** An item filed in a turn whose every new message is
  his enters the queue open, with no approval to wait for: his word in the room is the approval. Any
  other item raises a draft, which enters the queue only on his own approval on the console; the job
  service tells his approval from anyone else's by who calls it, never by a name.
- **What you record is a recommendation.** `job_item_update` with item <id> and an `approval` (no name:
  the service records it under the chat's own caller) is your recommendation that the owner approve
  it; the draft stays a draft until he does. Record one only on his own word, or on a grant in force
  that names this action and this project, and say the item waits for his approval on the console,
  with its number. His words on it go onto the item with `job_note_add`, and into the brief with
  `job_item_update` where the run must act on them.
- **A draft he asked for** (put it on the TODO as a draft, or any way of saying written down but not started)
  is filed, not approved: say it waits on his approval, with its number.
- **A member's request** is filed the same way, with `requesterName` the member, and is never
  approved by you: it waits for the owner. Say that plainly instead of promising
  the work.

## Changing an item that stands

- **`job_item_update` changes one item as a merge patch:** `title`, `brief`, `priority`, `position`,
  `assigneeName`, `queueName`, `state` and `mergedInto` among them. A field left out stays.
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
  this conversation. Named no role, it runs as your double, on your own model. It waits in its role's line
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
- **The worker is the item's `assigneeName`; where the item names none, the built-in worker,**
  `worker`. Name it in `assign_worker`.
- **One run at a time.** `assign_worker` refuses a worker already held or running (`no-worker-free`);
  do not force it, and do not take a lease you cannot launch. Wait for that run to end, then choose
  again.
- **A run that ended in an error is repaired before anything else moves on.** Read the item and its
  notes and find the cause, not the symptom. A cause outside the item's own work is filed as a
  repair item of its own with `file_item`, put first, and approved only on the owner's word or
  a grant; a cause in the item's own work means the rerun is the repair, with what the failure
  taught written into the brief. A launch that never reached the work is not a failure of the work:
  fix the launch and run it again. A cause you cannot name is put to the owner, not relaunched
  blind.

## Assigning and launching

1. **`assign_worker`** with `itemId`, `holderName` `harness`, `assigneeName` the worker, and `item`
   the content you read: `title`, `brief`, `workOrder`,
   `notes`, `labels` and `scope`, the room the item was filed from. The agency reads nothing of the
   job service, so the item is handed over whole; the `scope` is the room a job's end is told in. It
   answers the assignment and the worker it holds; the hold stands until the job starts and ends, or
   until you release it, and no clock lets it go: release one you will not run.
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
- **Never call `run_job` twice for one assignment or one lease.** One launch per item at a time. Your
  tools hold no release of an assignment: where a launch cannot go ahead, give the lease back
  with `job_lease_release` and let the hold run out; do not leave the item leased with no run and
  forget it.
- **The item's work is the worker's.** You choose the item, the worker and the launch; you do not
  edit its checkout or do its work yourself beyond a line or two (LONG WORK).

## The run, its report and its close

- **The run reads the item, does the work, and leaves its account on the item** in its notes and
  its report. It may put a question to the owner.
- **When the work is finished and its checks passed:** `job_report` of kind `final` with
  `actorName` `harness` and the run's account, then `job_item_update` with item <id> and `state`
  `done` to close it. The job's lease still stands on the item when its end reaches you, and the
  close ends it: close on that lease and take no new one. Never close on a report saying its checks
  did not pass; where the item is not to close, give back the lease its end names with
  `job_lease_release`, so the item is open again. An item left open with no lease, its lease gone
  some other way, is taken again with `job_lease_next` before it is closed.
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

## Never launch the same item twice

- **The lease is what says an item is being run.** An item that is `leased`, or that stands with a
  `leaseId`, is not taken again: `job_lease_next` naming it is refused, so two harnesses told of the
  same item race for it and one wins.
- **One run at a time per item and per worker.** `assign_worker` holds one worker and refuses one
  already held or running; `run_job` refuses an assignment that is not held for that item and
  holder.
- **A rerun is a new launch, not a second concurrent one:** the item's lease is taken again, its
  worker assigned, and `run_job` called once, after the cause of the stop is repaired.
