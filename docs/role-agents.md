# A role's agents

A role says what a kind of work is: its prompt, its tools, its skills, its line and how its sessions are kept.
An agent is one seat of a role: a row that names its role and the one road its runs go out on. This document
states what an agent holds, how a run comes to an agent, and how the agents are changed. What a role itself
holds is `docs/agent-roles.md`; how a run waits for its turn is `docs/role-slots.md`.

## What an agent is

- **An agent is one seat of one role.** A run of a role is let in on one agent of that role and on no other
  thing: the agent it is let in on is the seat it holds, and an agent carries one run at a time. A role with
  three agents runs three of its runs at once, and a role with none runs nothing.
- **An agent's road is fixed.** It names one provider, one model of it and the effort and thinking its runs
  go out with there. Its runs go out on that road and never on another: an agent whose provider cannot answer
  now, or has no quota, takes no run until it can, and no run of its moves to another provider.
- **An agent that names no road follows the supervisor that asks it.** Its run goes out on the asking
  supervisor's own provider, model and effort as they stand when it is asked; it takes no run that names no
  road and that no supervisor asked. It also takes a run whose ask names a road no agent of its role stands
  on — an item's model, a repository's pin — on that road. A role all of whose agents name no road refuses
  a run that names no road and that no supervisor asks, with why. The doppelganger's agents are seeded so.
- **A role holds no order, no cap and no forced provider.** How many of a role's runs go at once is the
  number of its agents that are switched on, and which road a run takes is its agent's.

## What an agent row holds

- **Kept in agency's store,** the `role_agents` table (`docs/store.md`), one row an agent:
  - **its name,** its key, kebab-case, which a screen and a log show the agent by; an agent added with no
    name is named after its role and a number, `coder-01`;
  - **its role,** one the store holds;
  - **its road:** `provider` and `model`, both or neither, with `effort`, `thinking` (`on`, `off` or the
    provider's own) and `answerTokens`, each where it is set;
  - **whether it is switched on,** `enabled`; an agent switched off takes no run and counts for nothing;
  - **its place among its role's agents,** `position`, lowest first: the order a run looks for a free agent
    in;
  - **whether its kept session is kept warm** between runs, `keepsCache`: the session its last answered run
    kept is pinged before its cache cools, so its next run there resumes it warm
    (`docs/keepalive-schedule.md`, 「An agent's kept session」);
  - **its compaction threshold,** `foldRatio`, the share of its model's window a run's session may fill
    before it is folded; where it sets none, its role's;
  - **what it lays over its role's preset,** each none where the role's own stands: `prompt`, which then
    stands in place of the role's prompt; `bundles`, which then are the bundles its runs are given in place
    of the role's; and `skills`, the names of the skills its runs carry in place of the role's bundle of
    them.
- **A role's preset is the default of every agent of it.** A run reads its agent first and its role for
  whatever the agent leaves unset, so an agent that sets nothing runs exactly as its role says.

## What a run reads off its agent

- **A run finds its agent by the slot it holds:** the seat of the slot request of its role that stands
  active at the folder the run works in — a job's checkout, a delegation's task folder, a place — the newest
  where more than one stands there. A run that names no folder, a translation among them, takes the seat of
  its role's only active request. A run whose seat neither tells stands on its role's preset alone.
- **Its prompt:** the agent's `prompt` stands in the place the role's prompt takes in the run's system
  prompt (`docs/agent-roles.md`, 「A role's prompt」). A system prompt is a word of the run's command, so an
  agent with a prompt of its own keeps a session of its own (`docs/sessions.md`, 「A role's session」).
- **Its bundles:** the agent's `bundles` are the bundles the run's token is issued with and its session's
  record carries, in the place of the role's (`docs/agent-roles.md`, 「Where the role is read」); the line's
  own capabilities stand whatever the agent names.
- **Its skills,** on the worker line: an agent that names skills is handed a plugin of its own, a folder
  under the agents root, `role-agents/<agent>/plugin`, holding the manifest of its role's plugin and the
  named skills alone — each from its role's plugin where that carries it, else from the first other plugin
  of the release that does. The folder is written whole whenever what it should hold differs from what it
  holds — the agent's list changed, or a release moved a skill's text — and stands at the same path run
  after run. A named skill no plugin of the release carries any longer is left out and said in the log. A
  list that names none hands the run a plugin with no skill.
- **Its compaction threshold:** the agent's `foldRatio`, where it sets one, is the share a run's kept
  session is folded at as the run ends, in the place of the role's (`docs/sessions.md`, 「Session
  compaction」).

## Which agent a run takes

- **A run is let in on the first agent of its role, by position, that is switched on, carries no run, and
  whose road can answer now.** An agent carries a run while a slot request let in on it stands active. In a
  role that holds both, an agent on a road of its own and an agent that names none stand in the one order: a
  run a supervisor asks takes whichever comes first, the second kind on that supervisor's road, and a run no
  supervisor asks passes the agents that name no road over.
- **A run that names a road takes an agent of that road alone.** An ask naming a provider and a model, or a
  model alone, is let in only on an agent whose road is that one, and waits for such an agent however long
  it takes; where no agent of the role stands on that road it is let in on an agent that names none, and
  where the role holds no such agent either it is refused with the roads the role's agents hold. A job names the road it found free: it reads its role's roads with the agents that carry no run
  first, so it asks for the first free agent's road.
- **A run waits where no agent of its role is free for it,** in its role's line, in the order asked
  (`docs/role-slots.md`, 「The line」). Where the only agents free are ones whose provider cannot answer, the
  request says why it waits, with the provider's own sentence.
- **The slot request names its agent,** as its `seat`, from the moment it is let in, with the provider, model
  and effort it goes out on, and keeps them to its end.
- **A native co-worker's call takes a seat and no road.** It stands inside its caller's program, which
  already goes out on its own road, so it is let in on the first free agent of its role whatever that
  agent's road and goes out on its caller's.
- **A reservation holds seats of a role by count,** not agents by name: a reserved seat is one fewer agent a
  run may be let in on, and the request that takes the reservation's place is let in on whichever agent is
  then first free.

## Changing the agents

- **`GET /role-agents`** lists every agent, by role and position, each with every field of its row and the
  run it carries now, or none; `role` narrows it to one role's.
- **`POST /role-agents`** adds an agent to a role: `role` required, every other field optional. It refuses a
  role the store does not hold, a name out of the pattern or already taken, a provider agency does not
  carry, a model its provider does not hold, a provider without a model or a model without a provider, a
  bundle the policy does not declare and a skill no plugin of the release carries.
- **`PATCH /role-agents/{agentName}`** changes any field of the row, each judged as the add judges it; a
  field set to null is cleared, which for an override gives the role's own back.
- **`DELETE /role-agents/{agentName}`** removes an agent, and is refused 409 while it carries a run.
- **A change holds from the next admission.** A run already let in finishes on the agent as it stood when
  it was let in; an agent switched off or removed while it carries a run takes no further run.
- **A role's own routes carry no road.** `POST /roles` and `PATCH /roles/{roleName}` take no order, no cap
  and no forced provider, and refuse one named; `GET /roles` answers each role's agents beside it.

## The roles the policy seeds

- **A role the policy seeds is seeded with its agents:** as many as its declaration's count, each on the road
  the declaration names, or on none where it names none. An agent is given once, with its role; a role the
  store already holds is given no agent by the policy again.
