# Work orders

Read this before reading or changing a work order, or before answering in detail how a co-work runs.
Reading one is your own tool, `work_order`; changing one is the deployed `work-order.ts`.
Changing a work order is owner-only work (ABSOLUTE RULE 2).

## How the bot runs one

- Every turn reads the item's brief, the prompt you launched it with, as the order's 지시: the item's
  conversation, the words people add and his approval note never reach a step except as you
  organized them into the brief.
- The builder is the job worker's own resumed session. It plans first, choosing from the archive's
  common co-worker catalogue only the roles and stages the item needs; a plan with no branch at all
  is valid.
- Whether an item runs as one is weighed on its impact grounds (todo.md, 「Co-work」), and the plan
  weighs them again; what it finds reaches you only where it comes to the other verdict, or where
  you filed none.
- The bot launches each stage's other roles as independent runs: each waits for a place in the
  common co-worker pool, runs once on its role's own model in a copy of its own, and leaves a
  result document.
- A builder task always stands in a stage of its own; the bot splits a stage mixing it with other
  roles.
- The builder reads the joined results once and writes the list of things to build, every
  implementation stage of the plan one task; the bot hands it over one task at a time. An empty
  list goes straight to the final turn.
- A build task checks only what it touched and commits its own step. The whole suite, the push and
  the close happen once, in the final turn, and no step waits for a deployment.
- Work the item committed to, and the follow-up that doing it turns up, is a step of the same
  order: the worker adds it itself, and a final turn that adds one sends the order back to the
  build. Only work separate from the item becomes a row of its own.
- `prompts/agents/worker/coworkers` seeds a new archive and is not the live list; ask `work_order`
  <id> for anything that has to be current.
- Never ask the builder to launch a Claude Workflow itself.

## Reading and drafting

- Read: `work_order` <id>, the order in lines: its state, its stages and every task's state and
  title. `~/.local/share/bmo/deploy/bin/bmo work-order --todo <id> --json` for the whole
  structure, long, with each task's instruction.
- Edit a draft: `~/.local/share/bmo/deploy/bin/bmo work-order --todo <id>` with `--intent '<목적>'` and `--instruction '<지시>'`
- A co-work launch must name its TODO.

## A stopped order

- It keeps its place in its worker's queue, and the bot resumes it at its turn:
  1. after the first counted stop, the failed task again as it was
  2. after the second, on a fresh session with a summary of the old one ahead of it
  3. after the third, on the retry model
  4. at the fourth, the item is held with the reason
- Successful peers and earlier stages are kept every time.
- `~/.local/share/bmo/deploy/bin/bmo work-order --todo <id> --resume` is for the owner asking to resume now; it retries the
  failed task plainly, without the ladder.

## A finished order run again

- `~/.local/share/bmo/deploy/bin/bmo work-order --todo <id> --reopen`, with `--intent`/`--instruction`/`--model` in the
  same call when they change. It keeps every completed task and result document as an earlier
  round the new plan carries on from, and puts the item back in the queue.
- Never write a fresh draft over a `work_orders` row; that throws the earlier results away.

## Changing steps

- States: pending, running, done, failed, skipped. Done means the work was carried out; a step
  decided against, or no longer needed, is skipped with its reason.
- One verb a call, through the bot. When the bot refuses, say its sentence rather than working
  around it.
- Change a step's instruction or model: `--task <step> --instruction '<지시>' [--model <모델>]`
- Skip: `--skip <step> --why '<사유>'`
  - A pending or failed step is skipped as it stands, a running branch stopped and skipped; a done
    step can be corrected to skipped on any order.
  - When the rest of a stage is all done or skipped, the runner moves on.
- Undo a skip: `--restore <step>`
- Run one step again: `--rerun <step>`, with `--instruction`/`--model` when they change.
- Add a step: `--add '<제목>' --instruction '<지시>' [--before|--after <step>]`, for a step the owner asks
  to have done in a running or stopped order; without a place it goes at the end of the build list.
- Reorder: `--move <step> --before <other>` or `--after <other>`, a pending implementation step past
  another pending one, or a whole later research stage past another; never a step into a different
  stage.
- End the whole order: `--close --why '<사유>'`
  - Every step not yet passed is skipped with that reason, and the item is held with it.
  - An order holding its worker's checkout stands stopped while the bot commits its open change and
    lets the checkout go, and is done only then; the other verbs refuse it meanwhile.
  - Closing is final; work the owner wants to take up later is stopped instead (below).
  - Closing or skipping is not a failure; say which it was.
- Rebuild an earlier round a later run wrote over, on a done order:
  `--recover <first job>-<last job>`; that round's steps decided against are then skipped one by one.

## Stopping running work now

- `~/.local/share/bmo/deploy/bin/bmo todo --stop <id> --why '<사유>'`: the one way to stop an item's running work at once,
  a builder turn included; the owner has the same as the 중지 button in the console.
  - Every running step and job of the item stops, finished steps stay, and the item is held as 중지
    with that reason. An order holding its checkout commits its open change and lets the checkout go.
  - It is not a failure and never counted as one; say it was stopped, and why.
  - `--unhold <id>` takes it up again, resuming the order from the interrupted steps.
- A skip or a close never stops a running builder turn; the stop does.
- To have it finish the step it is on and stop there, to be resumed later, wind it down:
  `~/.local/share/bmo/deploy/bin/bmo todo --wind-down <id> --why '<사유>' [--after <항목 id>]`.
- To change what a running turn does without stopping it, put his words on the item and rewrite
  its brief with the change, as todo.md's 「One more condition on a standing item」 says: the change
  alone, organized, in English, reaches the running pane, and every later step reads the new brief.
- Never edit `work_orders.body` directly, and never `systemctl stop` or kill a job's unit.
