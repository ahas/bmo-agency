---
name: bmo-agency
description: How the bmo-agency MCP tools file an item, run an item on a worker, review a job's report, stop a job and change an agent's fallback roads, with the arguments each takes and what each refusal means.
---

# bmo-agency

The agency supplies, delivers and controls agents. Its MCP server lists nine tools, each one call of the
agency's API. Call each through the tool, never through the agency's routes by hand.

## Starting the server

Register it with the harness as a stdio MCP server:

```
bmo-agency mcp --url <the agency's base address>
```

The address is the one the deployment names; never guess one. `AGENCY_URL` may give it instead; with neither,
the server reads `AGENCY_CONTROL_PORT` and calls the loopback on that port. The token goes in
`AGENCY_CONTROL_TOKEN`, in the file `AGENCY_CONTROL_TOKEN_FILE` names, or in `--token`; never print it.

## The tools

| Tool | What it does | Arguments (required in bold) |
|---|---|---|
| `file_item` | Files an item in the job service. An item the owner asked for himself opens at once; any other waits as a draft for his approval. | **`title`** (a short line in the asker's language), **`brief`** (the whole instruction, in English), **`queueName`**, `repository`, `priority`, `assigneeName`, `requesterName`, `labels`, `steps`, `stepsCheck` |
| `assign_worker` | Picks a worker for an item you hold and keeps it for you. | **`itemId`**, **`holderName`**, **`item`** (`title`, `brief`, `workOrder`, `notes`, `labels` as you read them), `assigneeName` |
| `release_worker` | Frees a worker you will not run. | **`assignmentId`** |
| `run_job` | Runs the item on its assigned worker with the lease you hold. | **`assignmentId`**, **`itemId`**, **`leaseId`**, **`holderName`**, **`item`** |
| `review_job` | Answers the check due on a job's report under review. | **`jobId`**, **`action`** (`ask`, `reject`, `pass` or `escalate`), `text`, `stage` (for `reject`) |
| `stop_job` | Stops a starting or running job on the owner's word. | **`reason`** (a short line, which becomes the job's report), and one of `jobId` or `itemId` |

`bmo use agency/<tool> --describe` prints any of them in full where the tool is reached through the tool host.

## Filing an item

1. Call `file_item` with the title, the brief and the queue. Name `repository` for work in a repository, so only
   its worker takes it; leave it out for host work. One change across two or more repositories is one item,
   its label `repositories` naming every one, comma-separated.
2. Where the item runs in stages, give `steps`, one question for each stage. The first filing with steps is
   refused `steps-unchecked`, naming the skills and tools of the agent the item runs on and a mark. Check each
   step against them, then file again with the steps as they now stand and `stepsCheck` set to that mark.

## Running an item you hold

1. Take the item's lease in the job service first.
2. `assign_worker`. The answer's `workerName` is the worker held for you, with its `assignmentId`. Nothing lets
   it go by itself: release it with `release_worker` if you will not run it. A refusal `no-worker-free` means
   none is free: ask again later or name another `assigneeName`.
3. `run_job`. The answer is a job in state `starting`, and the worker's session is launched on it.
4. The job's `state` moves `starting` → `running` → `reviewing` (where its stages are examined) → `ended`, or
   `stopped`. `outcome` and `report` say what the run came to.
5. A job that ended keeps your lease standing, the agency renewing it, so the item stays leased: report and
   close the item in the job service yourself from `outcome` and `report`, or give the lease back where the item
   is not to close. A stopped job gives its lease back, and the item is open again.
6. A note added to the item while the job runs reaches the job's turn by the agency itself; pass nothing on.

## Reviewing a job's report

A job-review instruction names the job and the check due. Read the report against the stage's question, then
call `review_job` once:

- `ask` with your question as `text`: it goes to the worker in the job's own session, and the answer comes back
  to you.
- `reject` with why and what is to be done as `text`; `stage` names the stage it goes back to, counted from one,
  or leave it out for the stage under way.
- `pass` with what the check found as `text`.
- `escalate` with what the owner is asked and why: refused until the work was sent back as often as the review
  allows.

## Stopping a job

`stop_job` only on the owner's word: only a turn whose every message is the owner's stops one. Name the job by
`jobId`, or by `itemId` for the item's starting or running job, with the reason as one short line.

## An agent's fallback roads

An agent's own road is its primary. Its fallback roads, in order, are asked only where the road before them
cannot answer for a clear reason read before anything goes out: the provider switched off, no credential for it,
its balance spent, no login open, or a block for a refused access or a limit. A turn that ended on an error, a
refused request or an outage block never moves a run onto a fallback. A job that moves onto one goes on there to
its end, its new session opening on the handoff of the one it left.

| Tool | Arguments | Does |
|---|---|---|
| `agent_fallback_add` | `agent`, `provider`, `model`; `effort`, `thinking`, `answerTokens`, `advisor`, `position` (from 0; last where left out) | adds one fallback road |
| `agent_fallback_remove` | `agent`, `position` | removes the fallback at that position |
| `agent_fallback_move` | `agent`, `position`, `to` | moves the fallback to `to` |

Each answers the agent with its `fallbacks` in order, and `runningWork` `run-to-end` where it carries a run: that run
keeps the road it went out on. A road the agent already holds, its own road again, and an agent that names no road of
its own are refused 422; a position it holds no fallback at is 404. Never write `fallbacks` with a change of the agent:
that is refused, since the list changes one road at a time.

## When a tool refuses

A refusal is a tool error whose text is the agency's `status`, `code` and `error` sentence; an answer marked
`isError` is never a success. Read the sentence: it says what to do next. The usual ones:

- `no-worker-free`: no worker is free now; ask again later or name another assignee.
- `steps-unchecked`: check the steps against the skills and tools it names, then file again with `stepsCheck`.
- `not-found`: nothing stands under that id; read the item or the job again.
- `conflict`: the job's state does not allow the move; read the job and act on the state it is in.
