# What each designated agent owns

Every designated agent — the chat agent and every companion — keeps its own state: its sessions,
its memories, its long-term memory, the reminders and watches that wake it, its room notes, its
rulebooks, its gate settings, its voice and its usage. This document is the canonical statement of
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
  check that it gets its own session, memories, long-term memory, reminders, watches, notes,
  rulebook, voice setting, usage and gate settings, and reads nothing of the others'.

## What the agent's row carries

| Column | What it says | The chat agent's | A companion's default |
|---|---|---|---|
| `brain_folder` | where its long-term memory lives, relative to the brain root | the brain root itself | `agents/<name>` |
| `voice` | the voice its words are spoken in, or none | the briefing voice it speaks in now | none |
| `window_seconds` | how long a room it answered keeps it listening without a call, or none | the conversation window it has now | none |
| `wake_words` | the words that call it, beside its label | the deployment's trigger keywords | its aliases |
| `session_scope` | one session across its rooms (`agent`) or one in each room (`room`) | `agent` | `room` |
| `turn_shape` | the shape of its turns: the chat agent's window (`window`) or its settings' pieces (`pieces`) | `window` | `pieces` |
| `default_answerer` | whether it answers every room that names nobody | yes | no |
| `prompts` | the folder under `prompts/agents/` a companion's system prompt and persona are read from, one plain name, or none | none: the chat line reads its own documents | none, or the folder its seed names |

- **An agent with no voice makes no voice.** No briefing, voice chip or reading is made for its
  turns, and a voice given to its row later turns all three on with no other change
  (`voice-briefing.md`).
- **The window and the wake words are read by the receiver alone** (`docs/companions.md`, 「Who is
  woken」).

## The stores split by an owner column

| Store | Its owner column | Who reads it |
|---|---|---|
| memories, situation rules and state rows | `agent` | the agent's turns, its matcher, its tools, the console's memories screen |
| reminders | `agent` | the reminder clock, which wakes the owner when one comes due |
| watches | `agent` | the watch runner, which wakes the owner when its check ends |
| room notes | `agent` | the owner's digest run, which files them into the owner's long-term memory |
| fragments (knowledge, room rules, person-book pieces, system documents, lore) | `agent`, with `all_agents` | the reference gates of a turn |
| gate options | `agent` | the gates when they run for that agent's turn |
| usage runs | `agent`, already standing | the usage screens, now grouped by agent |
| room sessions | `agent` | the runner, for an agent of the `room` scope (`docs/companions.md`, 「The session」) |
| instructions | `agent` | the agent's own conversation: each agent's takes only its own rows |

- **Every write names the agent it is for.** A tool or a block writes the agent of the turn it runs
  in; a script reads the agent from the run that calls it, whose environment carries the agent's
  name, or from its `--agent`; a write with no agent is refused rather than filed under a default.
- **A reminder, a watch and a memory event wake their own agent:** their quiet instruction is queued
  for the agent the row is, and only that agent's turns take it, through its own conversation in the
  row's room, the chat agent's and a companion's alike (`memories.md`, `docs/companions.md`). Nothing of
  another agent's ever reaches the chat agent's window.
- **A run names its agent.** Every agent's run carries its name in `BMO_TURN_AGENT`, which the memory,
  note, reminder and watch scripts and the chat tools read; a script also takes `--agent`, and a call
  naming no agent is refused.
- **Every read names the agent too.** A turn reads its own agent's rows; the matcher is offered only
  its own agent's condition rows; a reminder or watch wakes its own agent; a room's notes go to its
  own agent's digest. Every room several agents share hold rows of each, and each
  agent sees only its own.
- **A companion's gate options are made when it stands,** copied from the chat agent's as they are
  then, so the gates run for it from its first turn and are tuned apart afterwards.

## Long-term memory

- **Each agent's long-term memory is a folder of the brain:** its `brain_folder`, holding the same
  three parts the brain has always had — `people/`, `rooms/`, `knowledge/` — with the same shapes
  (`people-book.md`, `knowledge.md`). The chat agent's folder is the brain root, so its
  books, rulebooks and knowledge stay exactly where they are.
- **A fragment row names the agent whose folder its body is in,** and the reconcile reads the agent
  from the path's folder, so a body and its row never disagree about whose it is.
- **The shipped libraries are the chat agent's.** The system documents and the lore describe the
  system the chat agent runs and its persona, so their rows carry the chat agent; another agent
  reads one only where it is marked for every agent.
- **Shared between agents is a mark, not a place.** A fragment's `all_agents` mark says every
  designated agent's gates may offer it; the body stays in its owner's folder. It is off unless
  a recollector or a person sets it, and nothing sets it by default.
- **Three axes, kept apart:**
  - the room scope — every room or one room (`[shared]` or `[chat:<room>]`, and the row's room);
  - the person a person-book piece is about — the row's `owner`;
  - the agent whose memory it is — the row's `agent` and its `all_agents` mark.
- **A turn's candidates are its own and the shared.** Every reference gate, people-book search
  and the situation matcher take as candidates only the rows of the turn's agent and the rows marked
  for every agent, before any narrowing (`reference-selection.md`).

## The recollectors

- **The recollectors are the agents that decide what becomes an agent's long-term memory:** the
  digest, the person-book writer and the session summariser. Their kind is `recollector`, and
  every screen groups them under that name.
- **Each run is for one agent.** It reads the words of rooms that agent answers, writes into that
  agent's folder and rows, and writes from that agent's own first-person view
  (`prompts/rules/recollection.md`). A room several agents answer is read once per agent, and each
  run writes its own.
- **A room is read for an agent where the agent answers it,** as the room answerers record says;
  a room with no answerers is the chat agent's alone. The digest takes each agent's own notes, and
  the person-book writer keeps each agent's own marks of what it has read (`people-book.md`,
  「Each agent's run」).
- **Every recollector is told whose memory it writes.** Its task names the agent by the label its
  row carries, and an editor run stands in that agent's brain folder (`fragments.md`). The
  session summariser's prompt names the agent the handoff note is for, and the transcript it reads
  sets that agent's words under the same label.
- **A companion hands its own session over,** the session itself writing its state on its own
  model in one more turn once its context reaches the handoff mark, and no session summariser runs
  for it
  (`docs/companions.md`, 「Its limit」).

### Which model a recollector runs on

- **Each recollector's own key's road, by default:** the person-book writer on `people-digest`, the
  summariser of the standing sheets on `people-standing`, the digest on `digest`, each changed on
  agent management (`people-book.md`, 「Records」). The chat agent's recollectors always run so.
- **A companion may name its own:** where its settings name a registration as its recollectors'
  model (`docs/companions.md`, 「Its settings」), every recollector run for that companion — a
  batch, a cut, a sort, a reform, a sheet, a digest of its notes — goes out on that registration on
  the registry's provider, with the registration's own answer size, and the key's road is not
  read. An editor run thinks as the companion's own row says its turns think, and as the
  registration does where the row names nothing; a sheet keeps the summariser's own thinking. The run is the same run: its prompt, its tools, its desk and its checks are the
  recollector's, and only the model answering it is the companion's.
  - **A sheet so asked** is the summariser's request sent to that registration with the summariser's
    own wait and answer room (`gates.md`), as a gate on the registry's provider is asked.
  - **No other road stands behind it:** a registration that does not answer fails the run as a chosen
    road that did not answer does, and nothing turns to the key's road or to Claude in its place.
- **Why the companion's:** what a companion remembers is read back into its own turns, so it is
  written by the model that speaks as it, from its view, and no other model's judgement of what is
  fit to say reaches its turns through its memory.
- **Each run sorts what it keeps:** a fact every agent is better for knowing — about the owner or
  another person, about the system, the machines or their running — is written with the
  `all_agents` mark; a memory of the agent's own — what it said, promised, felt, how it plays or
  works — is written as its own.

## The console

- **The agent menu groups by kind.** The chat agent's and the companions' kinds stand in one group,
  labelled as designated agents, each agent under its own label, so another companion stands there
  the moment its row does. The recollectors stand in their own group, labelled as recollectors. The
  console reads the groups from agency's agent list (`docs/api.md`, 「Agents」).
- **The memories screen shows whose a row is** and filters by agent, listing the agents from the
  table.
- **Each agent's own screen shows its usage,** its week's tokens read from its usage rows, a
  companion's from the usage its turns write (`web.md`, 「Agent screen」).

## Shared knowledge proposed for the mark

None of these is marked by this change; each is the chat agent's until the owner marks it.

- **Facts of the system and the machines:** the compute nodes and their GPUs; a registered node
  name read from registration state at runtime; the hardware screens' physical units; the infra
  services deliberately stopped; the Claude accounts and their order; the gate routes; the console's
  audio player.
- **Facts about people and places:** a person's pizza preferences; what the numbers in member
  nicknames mean; a board-game cafe nearby.
- **Kept as the chat agent's own:** its briefing voice, its searcher limits, its way of recording
  memories, its handling of job failures, the cause-guessing contest, a cloud provider's login, the
  death-notice instruction and the words kept apart with it.
