# What a change does to running work

Every change to a setting or a state agency acts on meets the work already running under it: a job's turn on a
worker agent, a supervisor's turn, a session. Each kind of change takes exactly one of three branches, written
here, and the verb's answer and the notice the change raises say which branch it took.

- **Run to the end.** What runs finishes as it started, and the change holds from the next run. The default.
- **Graceful resume.** Only where the running work cannot go on — the node under it died, its session is
  replaced, agency restarts for a deployment — and then by a checkpoint or a handover the next run takes up.
- **Stop.** Only where somebody said in so many words to stop it, or where going on is unsafe.

A change never ends running work as a side effect: work past a lowered limit waits in the queue rather than
failing on its next request.

## The kinds of change

| Change | Branch | What holds |
|---|---|---|
| An agent's provider, model, effort or advisor changed (`PATCH /role-agents/{name}`) | Run to the end | The run goes on as it was admitted; the change holds from the next admission, and the answer says so with `runningWork` `run-to-end` (`docs/api.md`, 「role-agents」). |
| An agent's fallback roads added, removed or moved (`/role-agents/{name}/fallbacks`) | Run to the end | A run already let in keeps the road it went out on; the list holds from the next run, and the answer says so with `runningWork` `run-to-end` (`docs/role-agents.md`, 「An agent's fallback roads」). |
| An agent's own road stops answering for a clear reason under its job | Graceful resume | The job moves to the agent's next fallback road that answers, on the same seat and checkout, its new session opening on the handoff of the one it left, and stays there to its end (`docs/role-agents.md`, 「An agent's fallback roads」). |
| An agent switched off | Run to the end | Its run goes on; nothing new is admitted on it. |
| An agent removed | Run to the end | The removal is refused `409` while the agent carries a run, saying so. |
| A supervisor's road changed (`PATCH /supervisors/{name}`) | Run to the end, then graceful resume | The turn running finishes on the road it went out on; its end hands the session over to one on the new road (`docs/supervisors.md`, 「Where a supervisor's turn runs」). |
| A supervisor removed (`DELETE /supervisors/{name}`) | Stop | The owner's removal is an order to stop: its program ends with its sessions (`docs/supervisors.md`, 「Adding and removing a supervisor」). |
| A role changed | Run to the end | Its runs go on; the change holds from the next admission, and the answer says so with `runningWork` `run-to-end` where a run of it goes. |
| A role removed | Run to the end | The removal is refused `409` while a run of it goes or waits. |
| A provider switched off (`PATCH /providers/{name}`) | Run to the end | Only the switch is written; a run going out on it goes on, and no new run goes out on it; the answer says so with `runningWork` `run-to-end` where a run goes out on it. |
| A person's node renamed or re-placed | Run to the end | A waiting turn is placed by the record standing now (`docs/person-nodes.md`). |
| A node's link gone silent under a running job | Graceful resume | The job is taken up again in its checkout once its road answers (`docs/worker-slots.md`). |
| An account's window full | Run to the end | The run is never cut; the login takes no new run (`docs/usage-windows.md`). |
| A source refused under a run (`429`, `401`, `402`) | Graceful resume | The same run goes out on the next source in a new session that continues its checkout (`docs/supply.md`). |
| No source left under a run | Graceful resume | The run goes on one reservation more, then its turn is interrupted and the job waits for supply holding no slot, going out again once a source takes it (`docs/supply.md`, 「Running out mid-turn」). |
| Agency restarted for a deployment | Graceful resume | A running job survives and its lease is taken again; a supervisor's turn is kept in flight and re-attached (`docs/sessions.md`). |
| A queue paused or held in the job service | Run to the end | Running items keep their leases; only the next lease is held. |
| An item's frozen labels changed while leased | Run to the end | The job service refuses the change `409` `item-frozen`; a label a lease does not freeze changes and the job goes on as launched. |
| An item moved, dropped or merged under its job | Run to the end | The job goes on; its orderer is told `item-moved-under-job` (「An item's change is told to its orderer」 in `docs/agent-control.md`). |
| A room's supervisors changed while a turn runs | Run to the end | The turn finishes; the list holds from the room's next turn. |
