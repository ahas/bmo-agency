# What each designated agent owns

Every designated agent — the chat agent and every companion — keeps its own state: its sessions,
its long-term memory, the reminders and watches that wake it, its gate settings, its voice and its
usage. This document is the canonical statement of
how that state is split. Agency holds the agent's row in the `agents` table and its room sessions;
the other stores named below belong to the units that keep them, and are listed so every consumer
reads the same owner column. `docs/companions.md` is what a companion's session does with it.

## The principle

- **One owner column, never one table per agent.** A store that holds several agents' rows keeps
  them in the table it already has, with a column naming the agent each row belongs to: the
  agent's name, as its row in the `agents` table carries it.
- **A value that differs per agent is the agent's own,** a column of its row in the `agents` table,
  never a table of agents kept elsewhere and never a branch on an agent's name or kind.
- **Nothing counts the agents.** No code, schema, screen or test assumes a number of designated
  agents or names one of them: every reader takes the agent a turn is for and reads that agent's
  rows, so a companion added as a row has every part below on its own from its first turn.
- **The rows that stood before the split are the chat agent's.** Each owner column is added with a
  backfill naming the chat agent's row — the row of the deployment's agent key, read at the
  settling, never a literal name — so nothing the chat agent held moves or changes.
- **The proof is a third agent.** The tests stand a fake companion beside the two real agents and
  check that it gets its own session, reminders, watches, voice setting, usage and gate settings, and
  reads nothing of the others'; its brain is its own by its keys.

## What the agent's row carries

| Column | What it says | The chat agent's | A companion's default |
|---|---|---|---|
| `voice` | the voice its words are spoken in, or none | the briefing voice it speaks in now | none |
| `window_seconds` | how long a room it answered keeps it listening without a call, or none | the conversation window it has now | none |
| `wake_words` | the words that call it, beside its label | the deployment's trigger keywords | its aliases |
| `session_scope` | one session across its rooms (`agent`) or one in each room (`room`) | `agent` | `room` |
| `turn_shape` | the shape of its turns: the chat agent's window (`window`) or its settings' pieces (`pieces`) | `window` | `pieces` |
| `default_answerer` | whether it answers every room that names nobody | yes | no |
| `prompts` | the folder under `prompts/agents/` a companion's system prompt and persona are read from, one plain name, or none | none: the chat line reads its own documents | none, or the folder its seed names |
| `owner_account` | the id of the login account that owns it, or none | none | the one its registration names, or none |

- **An agent with no voice makes no voice.** No briefing, voice chip or reading is made for its
  turns, and a voice given to its row later turns all three on with no other change
  (`voice-briefing.md`).
- **The window and the wake words are read by the receiver alone** (`docs/companions.md`, 「Who is
  woken」).
- **No column locates its memory.** Its long-term memory is its brain, found by its account and its
  key (「Long-term memory」 below). The row's `brain_folder` locates nothing; a companion's reset reads it
  only to refuse the agent whose value is empty, the chat agent.

## The stores split by an owner column

| Store | Its owner column | Who reads it |
|---|---|---|
| reminders | `agent` | the reminder clock, which wakes the owner when one comes due |
| watches | `agent` | the watch runner, which wakes the owner when its check ends |
| gate options | `agent` | the gates when they run for that agent's turn |
| usage runs | `agent`, already standing | the usage screens, now grouped by agent |
| room sessions | `agent` | the runner, for an agent of the `room` scope (`docs/companions.md`, 「The session」) |
| instructions | `agent` | the agent's own conversation: each agent's takes only its own rows |

- **Every write names the agent it is for.** A tool or a block writes the agent of the turn it runs
  in; a script reads the agent from the run that calls it, whose environment carries the agent's
  name, or from its `--agent`; a write with no agent is refused rather than filed under a default.
- **A reminder, a watch and a memory event wake their own agent:** their quiet instruction is queued
  for the agent the row is, and only that agent's turns take it, through its own conversation in the
  row's room, the chat agent's and a companion's alike (`docs/companions.md`). Nothing of another
  agent's ever reaches the chat agent's window.
- **A run names its agent.** Every agent's run carries its name in `BMO_TURN_AGENT`, which the
  reminder and watch scripts and the chat tools read; a script also takes `--agent`, and a call naming
  no agent is refused.
- **Every read names the agent too.** A turn reads its own agent's rows, and a reminder or watch wakes
  its own agent. Every room several agents share holds rows of each, and each agent sees only its own.
- **A companion's gate options are made when it stands,** copied from the chat agent's as they are
  then, so the gates run for it from its first turn and are tuned apart afterwards.

## Long-term memory

- **Each agent's long-term memory is a brain of its own,** kept by the brain service and keyed by an
  account and an agent (the brain's design document, 「Whose a brain is」). Agency makes it as the agent
  is registered and files it under the id of the login account that owns the agent, else the key
  `AGENCY_BRAIN_ACCOUNT` names, and under the agent's registered name (`docs/api.md`, 「Calls agency
  makes」). The brain tool and the gates find it by the same two keys.
- **Everything an agent reads beside the conversation in front of it is a memory in its brain:**
  episodes, facts, situation rules, a room's rules, its knowledge, its persona and its lore, each a row
  with a type, and the people and rooms they are about as the brain's subjects. A person's sheet is
  rendered from the facts about them and is never a row of its own.
- **Two agents' brains share nothing,** and two accounts never see each other's. What several agents
  need to know is written into each one's brain by the agent that learnt it; no mark makes one agent's
  memory another's.
- **The persona comes from the brain.** At a chat launch the agent's brain renders the layer named
  `persona`, and its placeholders are filled from the deployment's values (`docs/api.md`, 「Calls agency
  makes」); the lore and knowledge reach a turn only as the gates pick them.
- **A brain that cannot be reached leaves the agent registered** and the failure logged; with no brain
  address configured, no agent has a brain.

## Who writes long-term memory

- **Each agent of the chat kind writes its own,** the chat agent and every companion alike, in the
  turn itself and on its own model: as it reads each message, the live one that woke it and every
  archived message a turn hands it — what arrived while it was away, after a handover or on its
  return to a room — it writes what matters of them into its brain. The discipline its system prompt
  carries from the brain says so (`docs/api.md`, 「Calls agency makes」), and no other agent writes an
  agent's memory behind it.
- **It writes from its own first-person view,** into its own brain, so what it remembers is read back
  into its own turns as it wrote it, and no other model's judgement of what is fit to say reaches its
  turns through its memory.
- **A room several agents answer is read by each,** and each writes its own.
- **A companion hands its own session over,** the session itself writing its state on its own
  model in one more turn once its context reaches the handoff mark (`docs/companions.md`, 「Its
  limit」).

## The console

- **The agent menu groups by kind.** The chat agent's and the companions' kinds stand in one group,
  labelled as designated agents, each agent under its own label, so another companion stands there
  the moment its row does. The console reads the groups from agency's agent list (`docs/api.md`,
  「Agents」).
- **An agent's memories are read on the brain's own screen,** by the brain the agent's two keys name.
- **Each agent's own screen shows its usage,** its week's tokens read from its usage rows, a
  companion's from the usage its turns write (`web.md`, 「Agent screen」).
