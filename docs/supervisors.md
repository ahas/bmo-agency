# Supervisors

A supervisor is the conversational axis of agency (`docs/agents.md`, 「Two axes」): an agent that owns
rooms and talks with people. The chat agent and every companion are supervisors. This document states
what a supervisor's row holds, how one runner carries every supervisor's turns, and what a supervisor
hands to a role. What a companion added on the console holds besides is `docs/companions.md`; what each
supervisor owns apart from the others is `docs/agent-ownership.md`.

## What a supervisor row holds

- **One row of the `supervisors` table is the supervisor's whole declaration.** Nothing in the code lists
  the supervisors or answers a question about one by its name.
- **Who it is:** its name, the key every reader finds it by; its label; the words that call it in a room;
  its owner, the login account it belongs to, or none for the administrator's; its kind's mark on a
  screen.
- **What it is told:** its own prompt folder, the documents its system prompt is composed from, and its
  brain folder, whose base persona its system prompt ends with (`docs/agent-documents.md`).
- **How it converses:** its session scope, its turn shape and whether it answers the rooms that name nobody
  (「One runner」 below).
- **Where it runs — its provider order:** the providers it runs on, in order, each with the model and the
  effort it runs on there, and the quota share below which a provider is passed over
  (`docs/provider-order.md`). Every supervisor carries its own; no order is shared with another supervisor
  or a role.
- **Its cache warming, switched per provider:** for each provider of its order, whether its resident
  program's prefix is kept warm while it is idle on that provider (`docs/keepalive-schedule.md`). A
  supervisor is the only thing agency warms.
- **Its compaction threshold:** the share of its model's window its session may fill before the session is
  folded or handed over (`docs/sessions.md`, 「Session compaction」).
- **Its upkeep of a turn:** its gate switches, its sampling and its voice, which agency keeps and a turn's
  consumer reads; and the repository its TODO items go to (`docs/repositories.md`).
- **Its draft questions** are the receiver's, kept by the receiver and shown on the supervisor's page in the
  console; agency holds none of them and names the supervisor a session is for, so the receiver reads the
  supervisor's own (「What a session's state names」 below).

## The bundles a supervisor holds

- **Every supervisor's session record holds the chat line's capabilities and the supervisor bundles,** the
  same for every supervisor, read from the roles policy's `supervisor` list: `room.chat`, its own records
  and the TODO; `room.send`, sending into the rooms it answers under its own name; and `web.read`, web
  search and page fetch (`docs/agent-roles.md`, 「The bundles」).
- **The chat line itself holds `gate.translate`, `gate.reference`, `deploy` and the voice namespace,**
  whatever the supervisor, as `docs/launch-lines.md` declares.
- **Every tool acts as the supervisor whose turn calls it:** its memories, notes and reminders are its own,
  a send goes out under its own name into a room it answers and nowhere else, and an item it files goes to
  its own repository. The tool host's grant lock judges an acting call the same way for every supervisor.

## Who a send goes out as

- **A send made in a supervisor's turn is that supervisor's.** The turn's run names it, and every sending
  tool and script reads it for every call.
- **Three things are asked before anything goes,** each refusing with a status code and one English
  sentence that names the supervisor: it answers the room — it is among the room's answerers, or the room
  records none and it is the default answerer; its row holds `room.send`; and the room's road carries the
  verb.
- **It is filed under the supervisor's own number and name** on a road that speaks for several speakers;
  the default answerer is the road's own voice and names no speaker.
- **Only a supervisor's own round sends.** A send anything else asks — a job, a role's run, a person at a
  shell, a script — goes out nowhere: the receiver hands its words to the room's supervisor as a quiet
  instruction, and that supervisor says them in its own turn or lets them go (the receiver's
  `docs/messages.md`, 「Words handed to the room's supervisor」).
- **Another supervisor's room stays closed:** a supervisor never sends into a room it does not answer.

## One runner

- **One runner carries every supervisor's turns.** A caller (the receiver) hands the words that woke a
  supervisor, and the quiet instructions queued for it, to the runner the same way for every supervisor;
  the runner reads the row for every turn and runs it by what the row says.
- **What the runner does the same for every supervisor:**
  - the room's queue: words gathered for the queue's delay, the window that keeps it listening, and quiet
    instructions taken into the next turn; in a turn of the window shape, a wait the session asks for and
    words said while the turn works put into it where its provider takes them;
  - one turn at a time per supervisor, since its turns go into its one program;
  - the session the turn goes on in, kept by the supervisor's session scope;
  - the turn run in the supervisor's resident program, on the provider its order gives it now;
  - the answer sent into the room under the supervisor's own identity, and handed back to the receiver as
    any word filed there;
  - the session handed over at its compaction threshold in the way its provider declares (「What a provider
    declares for a conversation」 below).
- **What each row says:**

  | Setting | What it says | The chat agent | A companion added on the console |
  |---|---|---|---|
  | `session_scope` | `agent`: one session across every room it answers, each turn carrying its room's block; `room`: one session in each room | `agent` | `room` |
  | `turn_shape` | `window`: the chat documents as its system prompt, the window's round prompt and the re-asks after an answer (`bmo-window.md`); `pieces`: the platform's harness opening, the companion prompt and its persona as its system prompt, its new words as its turn and no re-ask (`docs/companions.md`, 「What a turn is given」) | `window` | `pieces` |
  | `default_answerer` | whether it answers every room that names no answerer | yes | no |

- **The scope and the shape go together:** a turn of the window shape goes on in the one session of the
  `agent` scope, and one of the pieces shape in the session of its room.
- **The chat agent** is the supervisor whose row says it is the default answerer. A **companion** is any
  other supervisor.

### The session's scope

- **A supervisor of the `agent` scope keeps one session across every room,** each turn carrying its room's
  block.
- **A supervisor of the `room` scope keeps one session in each room it answers** (`docs/companions.md`,
  「The session」). Its one program goes on in the session of the room the turn is for: a turn for another
  room changes the command's session, and the program is started again on it in the same pane.
- **One turn at a time per supervisor,** whatever its scope.

### What a provider declares for a conversation

- **How a full session is handed over:** `fold`, the program's own fold put into it as a turn (Claude
  Code's `/compact`); or `handoff`, the session asked for a note in one more turn and a new generation
  opening with it (`docs/companions.md`, 「Its limit」). The registry's provider hands over; Claude Code's
  folds.
- **Its default compaction threshold:** the share a supervisor's row is seeded with where it is added on
  that provider and names none. DeepSeek and OpenRouter declare 0.5; a provider declaring none gives 0.85.
- **The limits that cut a turn going nowhere,** where the provider declares them (`docs/companions.md`, 「A
  turn that goes nowhere」). The registry's provider declares them.
- **The wait for an engine a job holds:** a turn on a registration an open job runs on waits until the job
  ends (`docs/companions.md`, 「The engine it shares」). The registry's provider declares it.
- **Nothing else in a conversation turns on the provider or the model.**

## Where a supervisor's turn runs

- **On the first provider of its order that has quota above its share and can answer now,** on the model
  and effort the order names there (`docs/provider-order.md`, 「Choosing the provider」). A turn whose
  provider fails for a limit or its availability goes on to the next provider of the order; the next turn
  begins at the top again.
- **Where no provider of its order has quota,** the turn waits, never refused, and goes out once a provider
  of the order has quota again (`docs/provider-order.md`, 「When every provider is out」).
- **A provider move changes the program's command,** so the next turn stands the program again on the new
  provider's session for its scope; a fresh session of a supervisor is never opened without the handoff
  (`docs/sessions.md`, 「A restart hands the session over」).
- **No supervisor holds a slot.** A supervisor is not a role and has no cap: its turns are bounded by its
  one program.

## What a supervisor hands to a role

- **A delegation:** the supervisor hands a task off through `bmo/delegate`, naming any role, the helper
  role where it names none (`docs/tools.md`, 「The delegate」). By default the run goes out on the
  supervisor's own provider and model as they stand at that moment — the supervisor's double — and holds a
  slot of the role it names (`docs/role-slots.md`, 「A delegated task」).
- **A worker ask:** a small coding task as a run of the worker role in a repository's checkout or the
  supervisor's scratch folder (`docs/tools.md`, 「The worker ask」).
- **A TODO item:** filed to the supervisor's repository, naming the role its job runs as, `worker` where it
  names none (`docs/repositories.md`, 「A job」).
- **An unknown role is an error:** an ask naming a role the store does not hold is refused with the name
  and starts nothing.

## What a session's state names

- **The supervisor it is for,** so a consumer whose own settings are kept per supervisor — the receiver's
  draft questions among them — reads the supervisor the session runs for; and its compaction threshold as
  `foldRatio`, read afresh from the row as the state is read (`docs/sessions.md`, 「Session compaction」).

## Adding and removing a supervisor

- **`POST /supervisors`** registers one (`docs/api.md`, 「supervisors」): its name, label, owner, provider
  order, scope, shape and repository, judged before anything is written. A supervisor added is given its
  own brain as it is registered; one the brain cannot make stands all the same, the failure logged.
- **The console's companion dialog** adds one with a room of the console's own that it answers alone
  (`docs/companions.md`, 「Where a companion and its rooms come from」).
- **`DELETE /supervisors/{name}`** ends one, with everything it kept of its own; its brain is data and
  stays unless the caller asks it let go. A seeded supervisor and the default answerer are not removed,
  since the deployment stands them again.
