---
name: bmo-agency
description: Run an item on one of the agency's workers with the bmo-agency MCP server. Use when a harness holds an item and should have a worker run it: ask for a worker with assign_worker, hand the item and the lease over with run_job, and read the job's progress and outcome from its fields and events.
---

# bmo-agency

The agency supplies, delivers and controls agents. For a harness holding an item, it assigns a worker to
that item and runs it there. The harness keeps the item: it takes the lease, renews it while the run lasts,
reports and closes the item in the job service, and passes words appended to the item into the run.

## Starting the server

Register it with the harness as a stdio MCP server:

```
bmo-agency mcp --url http://127.0.0.1:3220
```

The address is the agency's base address. `AGENCY_URL` may give it instead; with neither, the server reads
`AGENCY_CONTROL_PORT` and calls the loopback on that port. The token goes in `AGENCY_CONTROL_TOKEN`, in the
file `AGENCY_CONTROL_TOKEN_FILE` names, or in `--token`.

## Running an item

1. `assign_worker` with `itemId`, `holderName` and `item` (the item as you read it: `title`, `brief`,
   `workOrder`, `notes`, `labels`). `assigneeName` asks for one worker; `holdMs` says how long the worker is
   held. The answer is an `Assignment` with its `workerName` and `expiresAt`. When no worker is free the call
   fails with a 422 `no-worker-free`; ask again later or name another assignee.
2. `run_job` with the assignment's `assignmentId`, the item's `itemId`, the `leaseId` you hold, your
   `holderName` and the same `item`. The answer is a `Job` in state `starting`; the worker's session is
   launched on it.
3. Read the job with `GET /jobs/{jobId}` or from the agency's events (`job.created`, `job.changed`,
   `job.ended`). Its `state` moves `starting` to `running` to `ended` or `stopped`; `outcome` and `report`
   carry what the run came to. `tokenCount` is what the model spent.
4. End or stop the job with `PATCH /jobs/{jobId}`; its assignment is released. Report and close the item in
   the job service yourself from `outcome` and `report`.
5. Words appended to the item reach the run as a turn into the job's `sessionId`, at
   `POST /sessions/{sessionId}/turns`. The worker's question is put at `GET /sessions/{sessionId}` under
   `question`, and answered with `POST /sessions/{sessionId}/answers`.

Every failure is the common error shape: a status and a body of `code` and `error`.
