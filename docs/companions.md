# Companions

A companion is a conversational agent of its own, answering in rooms beside the chat agent. Agency
carries a companion's turns, keeps its sessions, stands its program in a pane and shares its engine,
the same way it does for any agent; what a companion is told, whom it answers and what it remembers
are the companion unit's, not agency's. This document covers only what agency holds of a companion.

## Where a companion and its rooms come from

- **A companion is seeded by the deployment or added on the console.** A seeded one stands again at
  every start, so it is never removed. One added on the console answers a room of the console's own
  alone, and the room is the consumer's that holds the console's rooms: its number, its title, its
  answerers and the companion's sender number there. Agency holds the agent's half
  (`docs/api.md`, 「Agents」).
- **Adding it writes the agent's row,** once every field has passed, in the order the dialog shows them:
  - the name follows the companions policy's pattern, and no agent holds it yet;
  - the label is given;
  - the provider carries a companion's answers, and the model has an address where that provider
    answers only a registration;
  - the worker its items go to is a worker.
- **The row it writes:** the kind `chat`, the chat role, the `room` scope, the `pieces` shape, not the
  default answerer, the label, the console's mark from the companions policy, the pinned provider and
  model, the worker, the brain folder `agents/<name>`, no prompt folder and not seeded. Its gate options
  are the gate's. Nothing is restarted: the runner reads the row and its settings for every turn, and
  its place and program are stood by its first turn.
- **Removing it takes everything of its own,** and only a companion the console added:
  - first a reset of both ranges, every room the caller names read as one it shares, so each keeps its
    record (「Starting it afresh」);
  - then in one write its settings, its base prompt choice, its session reset asks, its gate switches,
    its sampling, its reset point and its row;
  - then what stands for it: its program killed, so it writes no record of what the removal took, its
    workspace closed, its place and home folder taken away, its execution rows with their envelopes —
    an open row waited on for thirty seconds first, since its collector writes it as it ends — the
    agent host's view and head records of it, its agent lock, and the brain asked to forget its
    folder's editing lock. What could not go is logged and refuses nothing, since the removal is
    written.
  - Once begun it runs to its end apart from the request, as a reset does.

## The session

- **One session for each companion in each room,** a row of the room sessions table keyed by the
  agent and the room: the harness session its turns there go on in — the id it goes by, the provider
  and model it was opened on and the place it stands on — with its generation and when it opened, the
  last message the agent took, the hash of the system prompt its last turn was sent with, when that
  last changed, and what the runs on the words not yet taken have spent (「A turn that goes nowhere」
  below).
  - It is a table of its own because a companion holds one session in each room it answers, where the
    agents table and the worker sessions keep one session for an agent or a worker.
- **Each turn goes on in that session.** The companion's program stands on the room's session — the
  one it stood on, or the same program started again on this room's in the same pane where it stood on
  another room's — and the turn's prompt is its new words alone: the program keeps the conversation,
  so the turn goes on from the one before and the engine computes only what is new (`docs/agents.md`,
  「The session's scope」).
- **One turn at a time in the companion's program.** Turns of two rooms take the agent's own lock in
  turn, since they go into one program.
- **The system prompt is handed to the harness as a file,** composed by the companion unit for each
  turn; agency hands it over as it stands and reads none of it. It is the same bytes from one turn to
  the next until the companion unit changes it.
- **A changed system prompt keeps the conversation,** as any Claude session goes on under a changed
  system prompt:
  - whether it changed is read off the prompt actually sent: the SHA-256 of the system prompt against
    the hash the session row keeps from its last turn;
  - the turn resumes the same session under the new prompt, so nothing of the conversation is dropped
    or summarised;
  - the engine's saved state of the old prompt is not carried over: a state is found by the tokens at
    its start (`docs/work-nodes.md`), so none saved under the old prompt is loaded for the new one,
    the engine reads the conversation whole once, and the disk budget removes the old state in its
    turn;
  - the session row keeps when it happened, and the turn's log carries a line saying the system prompt
    changed, which the console shows;
  - a session's first turn records the hash it was sent with and is no change.
- **A new generation is a new harness session,** opened by the turn that finds:
  - no session held in the room yet;
  - a reset of the session asked for in the room and not yet carried out (「Clearing it」 below);
  - the held one opened on another provider or model than the pin names now, or on a place its
    registration no longer has;
  - the held one no longer there to resume: its program keeps no record of it, which the turn says in
    the program's own words before anything is stood. That turn is asked again once on a fresh session.
- **What each turn was given and how it answered is kept,** generation by generation, in the room
  session turns table: the user turn exactly as it went out and the answer as the run wrote it. The
  follow pane and the console read it; no run is built from it.
- **A restart loses nothing.** The session row keeps the last message the agent took, so a message
  that woke the agent and was not yet answered when the bot stopped is taken again at the start. What
  the cut run had spent on it is kept with it, so the restart takes the turn up within the same nets
  rather than afresh.

### Its limit

- **The window is its registration's.** The harness is told the registration's context window and
  answer ceiling as the run starts (`docs/own-llm.md`, 「How requests go」).
- **The session hands itself over at the handoff mark,** as every conversational agent's session on
  the registry's provider does, since that provider declares the handoff as its way of handing a full
  session over (`docs/agents.md`, 「What a provider declares for a conversation」). A turn whose context
  — its last request, as its harness reports it — has reached the share of the window the workers'
  sessions are handed over at, so with a margin of it left, is followed by one more turn of the same
  session, after its answer has gone out and before the room's next turn:
  - the turn is the session itself asked for its handoff: its prompt is the companion handoff
    instruction, sent as an ordinary turn on the very run the room's turns go out as, so the engine
    reads only the instruction past what it already holds and the session writes the note with the
    whole conversation in front of it;
  - what the instruction asks for and how the note is bounded are the companion unit's; agency hands
    the instruction over and keeps the note;
  - nothing of it goes into the room and no word of the room is taken by it: it is no round of the
    room's, its usage is the companion's, its log is a run of its own, `handoff:<room>:<time>`, and the
    instruction and the note are kept as the last turn of the generation they close;
  - the note opens the next generation: the room's session opens a new generation with no harness
    session and the note held on it, and the next turn opens a fresh harness session whose first prompt
    stands the note at its head; the note is let go once that turn answers, and the turn is kept with
    it as it went out;
  - an answer that hands nothing over leaves the session as it was, its log says why, and the next turn
    that ends at or past the mark asks again.
- **Why a turn and not the harness's fold:** the harness's own `/compact` folds on a request of its
  own shape, which the engine has no saved state for, so the engine would read the whole conversation
  again — on a long session, many minutes — where an ordinary turn on the same session reads only its
  new words.
- **The harness's own fold stands past the handoff mark, as a guard.** Claude Code folds a
  conversation unasked only near the end of the window it was told, a margin short of it, on its
  built-in instruction, which no hook or setting of agency's can change. On a wide window that margin
  stands past the mark, so between turns the handoff comes first and the harness's fold is reached only
  by one turn that grows past it in its own tool calls. It is left on: switched off, a session that
  once grew to the end is refused as too long on every turn after. A window narrow enough that the
  margin reaches below the mark has the harness fold first and the handoff never come.
- **No session summariser reads it.** The summary the session goes on from is the companion's own,
  written on its own model.
- **The window is the engine's whole context:** when other work takes a card, the engine waits at
  that card's stage rather than running on fewer cards (`docs/work-nodes.md`), so the window does not
  shrink and a session goes on as it was.
- **A turn at the limit fails** with the harness's own line about it (`docs/own-llm.md`, 「A turn at
  the input limit」).

### Clearing it

- **A reset of a room's session is an ask, carried out by the next turn there**
  (`docs/session-resets.md`): the turn opens a new generation before anything else, so a turn running
  when the ask comes ends on the session it began on.
- **The session cleared is kept:** the harness keeps the old session's record under the companion's
  folder, and the room session turns table keeps its turns under their generation. The new generation
  starts from the last message the old one took, so nothing said before it is replayed.
- **Who asks:**
  - the owner, with the reset control of a room session on the companion's settings or on the agent
    settings' session list;
  - a change the companion unit makes that it asks a reset for — a save of the companion's generation
    settings, a change of its base prompt (`docs/base-prompts.md`, 「Saving」) — which asks it of every
    room it holds a session in.
- **The program on the session is taken down as the ask is written** (「The program on an ended
  session」 below), so nothing shows or keeps the cleared session while the ask waits; the ask itself,
  the new generation, is still carried out by the next turn.
- **The turn that carries it out** settles the ask and says so in its log.

### The program on an ended session

- **The resident program stands only on a session one of the companion's rooms goes on in.** A turn
  stands it on its room's session, and between turns it stays standing there, its pane showing that
  conversation.
- **Ending a session takes its program down at once,** rather than leaving it to the next turn:
  - a turn its guard cut short, for the session the cut ended (「A turn that goes nowhere」 below);
  - a reset of a room's session asked, for the session that room holds (「Clearing it」 above);
  - a reset of the companion's conversation, for whatever session it stands on, every one of them the
    conversation's (「Starting it afresh」 below), or, where an overtaken turn holds it, by that turn as
    it ends (「What the reset cuts off」 below).
- **Why at once:** the program holds the ended conversation for as long as it stands. Its pane shows
  it, a person typing there goes on in it, and its loop starts it again on that session if it ends; a
  restart of the service leaves it standing as well. Taken down, the place holds no program, its pane
  serves the host, and the next turn in any room stands the program on the session that turn goes on
  in.
- **It never cuts a turn.** It is done only under the companion's own lock, taken without waiting;
  where a turn holds the lock, that turn has stood the program on its own session, and nothing is taken
  down.
- **Only a program on an ended session goes:** one standing on another room's session is left as it
  is. The session token the program carried is taken back from the tool host with it.

## Starting it afresh

- **A reset of the companion takes what it has said, what it remembers, or both, out of the system,**
  so it answers again as on its first day in that respect, under the settings it holds now. It is the
  owner's alone: the room screen and `bmo-agency companion reset <agent>` ask it, and nothing else
  does.
- **It is a route of agency's server,** a dry-run read of what it would take and a write that takes
  it, so it runs on the store wherever it stands and the verb and the console only ask it
  (`docs/api.md`).
- **The caller names the rooms the companion answers,** those it answers alone and those it shares
  with other agents, since which agents answer a room is the room's owner's; agency reads no room's
  answerers, and judges only its own rows by the rooms it is handed.
- **What each range takes of agency's is agency's; the rest is its owners',** told by the reset's
  event as it ends (`docs/api.md`, 「Events」, `companion-reset`), so each unit takes its own rows:
  - the conversation range takes the companion's room sessions and their kept turns, its run log lines
    and their run records, the run log lines of the rooms it answers alone, its runs in the run ledger
    and those of the rooms it answers alone, the records its harnesses keep of the sessions run in its
    home and the prompts their histories kept of them; the rooms' own records — their messages,
    judgements, usage and the windows they hold open for it — are their owners';
  - the person range is the brain's: agency asks the brain to let go of everything it holds under the
    companion's scope — its memories, notes, watches, reminders and fragments — so nothing it
    remembered comes back (`docs/api.md`, 「Calls agency makes」, the brain); agency takes the records
    its harnesses keep of the recollectors' sessions run in its folder;
  - what it keeps, whichever range — its agent row and pin, its settings, its base prompt choice, its
    gate switches, the rooms themselves, the registry and the engine's placement, and whatever range
    was not named.
- **A dry run reads the same,** agency's rows counted and the brain asked for a preview, nothing
  written.
- **Once begun it runs to its end,** apart from the request that asked it, so a caller that stops
  waiting — the console's request running out, a verb killed — loses only the answer and never leaves
  it cut between the steps.

### What the reset cuts off

- **Nothing of what the reset took is written again after it,** by a turn that was running or by words
  that were waiting, whichever range it named.
- **The reset point** is the moment the reset cut the companion off, kept in the store's settings
  under the companion's name. The reset moves it to now twice: as it begins, before it takes anything,
  and again once the store holds nothing of what it took, so a turn begun while it ran, which may have
  read what it took, is cut off too. Each move is published with the point, `companion-cut-off`
  (`docs/api.md`, 「Events」).
- **A turn begun at or before the point is overtaken.** A turn begins as its round takes the words it
  answers; the handoff after a turn is a turn of its own here, begun as it starts.
  - The reset stands a cancellation over every run of the companion going as it cuts it off — the runs
    `companion-run-going` named going and not yet ended — so a run under way stops where it stands, as a
    run its guard cuts does.
  - Every call of its tools and every verb its shell runs is refused whole from then on, and the
    refusal tells it to end the turn.
  - It writes nothing of its own: no answer goes into the room, and no kept turn, usage, road or
    handoff is written. The run log it wrote under its run is taken back, so the turn leaves no
    record of itself, its first line included.
  - A turn holds the point off whenever it writes, reading the point again under the hold, so a turn
    writing as the reset begins writes whole before the point moves and the reset takes it after, and
    one reading the point after it moved finds itself overtaken and writes nothing.
  - The program it stood goes as it ends, where no room goes on in that session any more: killed where
    it still stands there, and the records its harness keeps of the session, the prompts its history
    kept of it and every line its place's records hold of the session removed after it, since the reset
    left them to the turn holding them.
- **What waited for its turns is let go unread,** in every room it answers: the words its rooms'
  queues were gathering, those its conversations held or waited on, and its quiet instructions, whose
  rows are settled as delivered so no turn takes them again. Those are the rooms' owner's, which lets
  them go on `companion-cut-off`. A run an overtaken turn launched answers
  nothing either: its result is dropped where the run began at or before the point or the reset took
  its row. A round left with nothing to answer runs no turn.
- **The program standing in its place is taken down where the conversation range is named,** once the
  rows are gone, whatever session it stands on, since every one is the conversation's (「The program on
  an ended session」 above), so the pane no longer holds a conversation whose record was removed. It is
  killed rather than asked to end, and waited on until it is gone, so a program asked to end does not
  write its session's record back after the reset removed it. Where a turn holds it, the turn is one
  the reset overtook, and takes it down itself as it ends.

## The engine it shares

- **One engine, one slot, many sessions.** A companion's registration may be served by the same
  engine a worker's jobs run on and the gates ask. The engine itself keeps each session's state apart,
  saving the slot and loading the matching saved state whenever a request does not go on from what the
  slot holds (`docs/work-nodes.md`), so no request reads another's from the start.
- **While a job holds the engine, the companion's turns wait,** as every conversational agent's turns
  on a registration do, since the registry's provider declares the wait. The engine is held when an
  open job runs on the companion's registration. Both are read from rows: the job's from the run it
  noted, or before it noted one from its worker's pin, and the companion's from its own pin; a run or a
  pin resolves to the registration its model names, where the registry holds one.
  - Every room's turn of that companion waits, looking again every few seconds, until no such job is
    open. Words that come in meanwhile, and quiet instructions of its own, join the waiting turn and
    are answered with it.
- **The room shows the wait.** The turn's running line in the room says the agent is waiting for its
  engine and which item's job holds it, from the moment it starts waiting until the turn begins. The
  turn sends a progress of its own in a waiting phase naming the job and its item, and stands in the
  bot's picture with the job it waits on, so a screen opened mid-wait reads the same line.

## What a turn shows

- **The same turn events a turn of the chat agent's flows,** for the room: the turn started, waiting
  for the engine, the answer being written, sent, or failed with its reason. The start and the ending
  name the companion as the turn's speaker, by its number in the room and its name, so no screen knows
  a companion apart.
- **The turn stands in the bot's live picture** for as long as it runs or waits, under the same
  speaker, so a screen that opens or reloads in the middle of it still shows it, and a stop of the bot
  waits on a running one. Its run is an execution of the companion's own host, which a stop reaches as
  it reaches any agent's.
- **Its own pane shows each turn as it goes:** the head pane of the agent's workspace is its program's
  own screen, where a person reads its tool calls and its thinking as the program draws them and can
  type into it (`docs/agent-control.md`, 「One standing pane per agent」).
- **A standing pane shows each session.** In the herdr workspace of the agent, named by its row's
  herdr label, each room session has a tab of its own named by the room's title, running
  `bmo-agency companion follow <room> --agent <agent>`, the room named by its number, out of the deploy
  tree's binary: every turn as it was given and every answer as it was written, as they are appended. A
  pane started from an older binary is replaced. The pane stands apart from the panes of its worker's jobs, and the console's
  agent screen lists the agent's room sessions and shows each pane.
  - **The verb** prints every generation the session has had, each under a line naming its number — the
    newest with when it opened, which is all the session row keeps — then waits on the newest and
    prints each turn as it is appended, going on into the next generation when a new harness session
    opens one. A turn is headed by who it is — the room for what the agent was given, the agent's label
    for its answer — and the time it was kept.
  - **The tabs are stood** for every agent holding room sessions as the chat receiver attaches to
    herdr, on the same clock as the heads, and for one agent as a session of its opens. Every pass is
    the same: a tab whose command line is the one wanted stays, one running an older line — the verb's
    script moved, or the room gone from the agent's sessions — is closed, and a missing one is opened,
    nothing focused.
  - **A follow pane is a standing pane,** told apart by the standing mark its command line carries, so
    no sweep of leftovers closes it, as none closes a head or a dev server.
- **A failure says nothing in the companion's name,** but for one its engine ended. The turn ends
  failed with its reason, which the console shows on the turn's line. A turn cut short is told to the
  room under the chat agent's number (「A turn that goes nowhere」 below).
- **A turn its engine ended with an error carries that error's kind on its ending,** `engineError` on
  `turn-ended` (`docs/api.md`, 「Events」), so the consumer holding the room leaves one line there under
  the companion's own number, in plain words chosen by the kind. The kind is the one the program's
  record and its `StopFailure` hook carry (`docs/agent-control.md`, 「What a resident turn reads」), as
  the program names it, or none where it names none.
  - The words the turn took stay untaken, as after any failed run (「A turn that goes nowhere」 below),
    and words that came in while it ran follow as the next turn.

## A turn that goes nowhere

- **A turn is watched as its run streams** where its provider declares these limits, as the registry's
  provider does for every conversational agent on it (`docs/agents.md`, 「What a provider declares for a
  conversation」), through the tool calls and their results the turn's log takes, and cut short at the
  first of these:
  - **the same tool failing six times in a row;**
  - **eight calls in a row that repeat themselves and learn nothing:** each the same tool as one of the
    eight calls before it, with an input at least 90% alike by its pairs of characters, and each
    failing or answering what that alike call answered;
  - **the last nets:** 80 tool calls in the turn, or 30 minutes of its run, the wait for its engine not
    counted.
- **The nets bound the words, not one run of them.** Words a run did not answer stay untaken, and the
  next turn in the room asks them again: after a restart cut the run in flight, or after a run that
  failed. Each of those runs is the same turn spending again, so what they spend adds up:
  - the room's session row keeps the tally of the words not yet taken: the tool calls made and the
    minutes run, the wait for the engine not counted;
  - a run writes its tally as it goes, at each tool call and each result and once a minute, so a
    restart loses at most a minute of it;
  - a run starts with the tally the runs before it left: its calls count on from there, and its clock
    has only what is left of the 30 minutes;
  - a turn whose words already stand at a net is cut before its run starts, as any cut is;
  - a turn that takes its words, answered, skipped or cut, clears the tally;
  - repetition is read within one run alone, since a fresh run's calls are not the ones before it.
- **Repetition is read first and the nets last,** since a turn going nowhere repeats itself long
  before it reaches either.
- **What a cut does:**
  - the run is stopped, and the turn ends failed with the reason, as any failure does;
  - the room is told in one short line under the chat agent's number, naming the companion and the
    reason;
  - the words it took stay taken, that line with them: the turn is kept on its generation with the
    reason in place of an answer, so neither the room's next turn nor a restart asks them again;
  - the room's session opens a new generation, so the next turn opens a fresh harness session rather
    than going on in the one that looped, which loops again when resumed;
  - the resident program still standing on the session the cut ended — the one the run went on in, or
    the one the room held where the cut came before any run — is taken down at once (「The program on an
    ended session」 above);
  - the chat agent is told, by a quiet instruction naming the companion, the room, the reason and the
    turn's log, and is asked only to file the malfunction where it keeps coming back.
