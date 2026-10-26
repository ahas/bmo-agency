# A chat session's life

How a chat session is kept and resumed, told of documents that changed under it, paced at a start,
folded when its window fills and replaced when its memory stops being worth its price. Clearing a
session on the owner's ask is `docs/session-resets.md`.

## One session per chat

- One Claude session per chat is kept and resumed, so the chat agent remembers previous exchanges. Every chat
  run is on the model and effort the tier of the `bmo` work key maps to on the provider it runs on; a
  background job is on those of `job-worker`'s tier unless it was launched with `--model`. The tier
  table is where the model is named (`docs/model-tiers.md`), and `bmo-agency agent` shows what each agent
  is configured to run on.

## A restart hands the session over

- **The chat works in a folder whose name a release does not change.** Each program keeps the session
  it stands in by the folder it was stood in, and a program stood again on the same folder takes the
  session up again rather than opening a fresh one beside it:
  - on a program that keeps its session by the folder's path as given — dsh, whose screen records the
    session with the folder in its place — the chat works in the deploy tree's link, whose path is the
    same while its target moves from release to release;
  - on Claude Code, which names a session's records after the folder's real path, every link followed,
    the link would be a new folder with each release, so the chat works in its agent's own home under
    the data root, `companion-homes/<agent>`, a real folder, as a companion does; its prompt, its
    configuration, its settings and its MCP servers are handed by absolute path, so nothing of the
    launch reads the folder it stands in.
- **A deploy moves the link's target, not the chat's folder,** so a program stood after it resumes the
  conversation it held, on Claude Code by `--resume` in the same project folder. The relaunch
  instruction a consumer writes at a deploy is the deployment briefing, not the notice of a closed
  session.
- **Agency is a session relay, and its own restart is no session event.** It opens a model session
  where none stands and relays turns into the one that stands; a turn in flight across agency's restart
  is picked up again, not lost:
  - every turn sent is kept in agency's settings while it goes, under its run (`turn_in_flight:<run>`,
    `docs/store.md`), with what its end needs to reach its caller: the session and the turn's id, the
    agent, the room, the chat turn's run in the ledger and the job;
  - a server that starts reads each turn the process before it left there against the record of the
    agent's pane program, `loop.jsonl` in its place, by the run's name, before it reads anything else that
    process left;
  - a turn whose end the record holds is handed back through the path a live turn's end takes: what of
    its record the run log lacks is filed there with its end, its execution row and the place's run are
    closed, a chat turn's road, spend and ledger run are written, its end is published as `turn.ended`
    under the turn's own id, and a job's turn ends its job with its report as the job's launch would have
    (`docs/api.md`, 「assignments and jobs」);
  - a turn the program is still working on, or still holds in its inbox, is attached to: the server
    holds the agent's program, so no turn of its own goes in meanwhile, waits on the record, and hands
    the end back once it is written; a program that goes away meanwhile ends the turn failed;
  - a turn no record names — one on a program that keeps no such record — is ended failed for its
    caller, and a job's is left to the pass that ends a job left running.
  The progress read of a turn picked up this way answers `unknown-turn`; its caller reads its end on the
  event stream.
- **A program is stood afresh only between turns** (`docs/launch-lines.md`, 「What is fixed」). A
  program whose command changed while one of its turns works — a deployment that moved its line, its
  prompt or its environment — is left standing until the record says that turn ended: the turn that
  found it changed goes into the program as it stands, waiting in its inbox behind the working turn, and
  the next turn's stand replaces it. A program that goes down while a turn runs and is stood again
  resumes the same session; the turn it was on is the caller's to ask again from its own round, which
  holds the item, the message it answers and the report in hand. One session answers a room: the
  folder's record is what tells a program which session that is, so no second one is opened while the
  first stands.
- **A caller that names a folder works in it,** as a conversation opened on one does; only the chat's
  own launches, which name none, stand in the folder above.
- **A fresh session of a chat agent is never opened without the handoff.** Where a chat-line turn — the
  chat agent's across rooms, or a companion's in a room — is about to open a fresh session for an agent
  that had one, agency first asks the receiver for the agent's handoff (`POST /handoffs/{agentName}` of
  the receiver, with the reason and the room where the turn names one) and waits for its answer, so the
  session going away writes what the next one needs before anything replaces it:
  - **what counts as replacing a session:** the kept session gone — the program no longer holds it — and
    a session the agent had that the turn does not resume — a reset, a provider or a prefix change, a
    login the session cannot follow — while the agent's program still stands in its pane;
  - **what does not:** a turn that resumes the session it had, and a program stood again on the same
    folder after agency's own restart or a deployment, which takes the same session up again, so
    neither asks anything;
  - **the order:** the ask goes out while the old program still stands in its pane under herdr, before
    the turn's stand replaces it; the old program usually outlives agency's own restart, so a turn after
    a restart that opens a fresh session asks as any other does;
  - **the answer:** 200 with the handoff file's path and size where the old session wrote one, 204 where
    there was nothing to ask or it wrote nothing; a 409, a handoff already under way, is waited on and
    asked again; the wait is bounded by the receiver's own handoff bound and a margin
    (`policy/handoff.toml`), and an error, a refusal or the bound passing lets the turn go on — the
    fresh session is opened all the same, and the run log says the handoff was not had and why. A
    deployment that names no receiver asks nothing.

## Document reload

- A session is resumed, so what it remembers of a document is the version it read then; sessions
  drift apart when the documents move on without them. A document a session reads with a tool — a
  knowledge chunk, a person document, the lore — is one it has to be told about: every session
  concerned is told quietly to re-read it, as one queued quiet instruction per session — through the
  first of its chats the file concerns — which wakes nobody: it rides the next round the session
  opens, in any of its chats, for something else, and a reload still waiting there is folded into
  the next one for that chat, so a quiet chat holds one notice naming every file changed since the
  last turn (`bmo-window.md`, 「Rows that ride」).
- The notice names the changed files and carries their current text (a very long file is named and
  left for the session to open), saying that the memory it holds of them is stale, that the text
  given is current and outranks that memory, and that nothing needs saying in the room.
- A person-book document is the exception: it is named without its text, and the notice says its
  current version will be attached the next time a turn needs it — the version has changed, so the
  reference attaches it in full exactly once.
- Where several chats share one session, a changed file is announced to that session once rather
  than once per chat; a room's own document is announced only to that room.
- A reload that names the agent whose documents moved and the rooms they concern is told only to that
  agent's sessions in those rooms; the brain names both for its own documents, having worked out which
  rooms each changed document concerns (`docs/api.md`, 「Sessions」).
- The session stays as it is: no restart, no `--resume` into a fresh session, no break in the
  conversation.
- **Common system-prompt documents are reloaded without waking chats.** They mark the shared chat
  prefix as changed. Room-specific instructions, tones and traits are carried in a turn's leading
  context block instead; changes are picked up on the next turn without invalidating the common
  prefix or waking the session. Agency remembers the last successfully applied room context per
  provider session in its store (`room_sessions`). See `unified-bmo.md`.
- **Agency names what moved; the consumer holding the rooms tells them.** Agency's reload verb sorts
  the files it is given by their place:
  - a document that is part of a chat session's system prompt marks every awake conversational
    agent's prompt as moved (`prompt_changed_at`, one `prefix_moves` row per file), and is not
    announced;
  - a document a chat turn carries is neither marked nor announced: the next turn carries its text;
  - every other document is handed to the running server (`POST /documents/reload`), which publishes
    `documents-moved` with the paths and whether the notice is due at once (`docs/api.md`);
  - a file under `prompts/rules/` or `prompts/agents/` first has the agents' documents written again
    by the prompt-composing entry (`docs/agent-documents.md`);
  - a file a worker's prefix is built from is answered with the advice to fold the worker sessions,
    since a worker reads its prefix only when a session opens.
- Which chats a file concerns, the hour a chat is told at most once in, and the gathering of what
  changes within that hour are the consumer's: it holds the rooms, their people and the queued
  instructions. It hears `documents-moved` and tells only chats that already hold a session — a chat
  without one has no memory to go stale.
- Who reloads follows from where the document lives. A brain document is runtime data: the brain
  calls `POST /documents/reload` for its own changes whenever a run succeeds, and work that edits one
  by hand runs `bmo-agency reload --path <file> [--path <file>...]` afterwards; with `--now` the
  notice is due at once rather than after the hour.
- A document that ships with the code — a rulebook under `prompts/rules/` or an agent document under
  `prompts/agents/` — is read out of the deploy tree, so it is told by the deployment that puts it
  live, through the deploy tree's own reload; the same command run from a checkout refuses such a
  document (`deploy.md`, 「What goes live with the documents」).
- Agency holds no lore and keeps no persona of its own. The base persona a chat session's system prompt
  ends with is the base layer of the agent's own brain, the layer named `persona`, rendered by the brain
  and read through its client once at each launch, with its placeholders filled from the deployment's
  values file (`docs/api.md`, 「Calls agency makes」). The brain renders the same bytes until a piece
  changes, so the prompt's prefix holds across launches, and a changed piece is carried by the next
  launch and is no document of agency's to reload.
- Where the brain gives no base persona — the agent has no brain, no brain is named, the brain does not
  answer, its rendering is empty, or a placeholder stays unfilled — the base persona the turn hands in
  as part of its context (`docs/api.md`, 「Sessions」) stands in its place, and the launch's run log says
  in one sentence why the brain's was not used. Neither case fails the launch.

## Launch pacing

- After a start, the first run of each chat session is spaced from the first run of any other chat
  by at least ten seconds, in the order they came (agency's chat line); a chat's later runs
  go straight through.
- Several sessions starting in the same instant is the shape of abuse and can cost the sessions
  themselves, and a restart is exactly when that would happen: the backlog of every chat, the TODO
  instruction and reload notices all fall due together.
- Instructions that wake rooms are held to the same gap, one cold wake at a time, by the instruction
  pass (`bmo-window.md`, 「Quiet instruction」). Everything else the bot starts — gates, runners,
  compactions — comes one at a time by construction and is not paced.
- The upkeep still waits behind the real work: the keepalive check's first launch comes ten minutes
  after start. The going-down line is a fixed message and starts no session.

## Session compaction

- After every chat turn the program's status line reports how many tokens the session's context
  holds and how large the model's context window is. When a reply leaves the session past its fold
  ratio, the chat agent announces in that
  chat (and only there, when that chat's rest notices are on) that it is going to sleep for a
  moment, types `/compact` into its resident program, and announces that it woke up.
- **The fold ratio is a value per provider and abstract tier,** kept in agency's fold-ratio table and
  changed on the console (`docs/api.md`, 「fold-ratios」), never a constant of a policy file:
  - a session's is the table's value for the provider it stands on and the tier its agent's work key
    is assigned, read afresh as the session's state is read (`foldRatio`), so a change reaches the
    next turn of every session;
  - a provider and tier the table holds no row for read the provider's own declared default — 0.5 for
    DeepSeek and OpenRouter — and a provider declaring none reads 0.85; a session whose agent's work
    key is assigned no tier reads its provider's default the same way;
  - **a session's state carries the context its last turn left,** `context` with `carried` and `window`,
    read off that turn's answer and held while agency's process stands, and let go as the session folds
    or ends; a session no turn has ended on since then carries none, and the next turn's end fills it;
  - **a session whose agent's resident program carries its own turns** — dsh's, one session across the
    agent's rooms, read alike by the agent's own session and each of its rooms' — carries instead what the program's record says of its last answer: `carried` the
    `used` that answer's context names, or where it names none, as dsh writes it only on a turn of
    several requests, the input, cache-read and output tokens of the answer's one request; `window` the
    `size` the record's latest context names. The held context stands only where the record holds no
    answer or names no size;
  - the receiver compares the context ratio a turn ends on — the state's `context`, carried against
    window — with the session's `foldRatio`, and the harness compacts after the turn that crosses it; a
    provider that hands its session over rather than folding it is judged by agency itself at the turn's
    end against the same share;
  - **a chat session on a resident program that carries its own turns folds itself.** dsh takes no
    command through the inbox its turns arrive by, every line put there reaching the model, so the fold is
    its own compaction: the overlay its program stands on names the session's fold ratio as dsh's
    compaction threshold (its `compaction-basic` row's `thresholdRatio`), and dsh compacts inside the turn
    that would carry its context past that share. The session's state says so, `foldsItself` true, and a
    consumer makes no fold call for it; a change of the fold ratio changes the program's command, which
    stands the program again on the same session. Every other session's `foldsItself` is false;
  - the session's state names that tier as `tier`, none where the key is assigned none, so a consumer
    whose own table is keyed by tier, the receiver's draft process table among them, reads the same tier
    the fold ratio was read at.
- Both lines are the session's own words: the full session is asked for its going-to-nap line and
  the compacted one for its waking line, each by a quiet instruction, and the fixed lines, which agency
  names by status code and the caller's language pack words, are only the fallback when the session cannot be asked. Each line is a full
  run of that session, so a chat that would not be shown it — its rest notices off — is not asked
  for it either. A `/compact` the CLI declines to run is a failed compaction, not a silent success.
- **A fold that fails is said out loud.** The room that was just told the chat agent was going for a nap is
  told the nap did not happen, in the fixed line named by its status code, and the owner is told
  which room and what stopped it — a fold nothing can complete leaves the context exactly as full as
  it was, so the next reply asks for another one, and a console log is not where anyone would find
  out. Both are said once per cause: while the same failure stands, the attempts go on in silence
  and the going-to-nap line is not said again, and a different failure says its own piece. A fold
  that works again in that silence comes back in it, since a room told nothing about a nap is not
  told it woke from one.
- Nothing is launched beside a compaction: what the session has read and not yet kept it writes into
  its brain itself, as the brain's discipline asks (`docs/agent-ownership.md`, 「Who writes long-term
  memory」).
- The session id is unchanged and the compacted session keeps its memory of the chat. A fold is given
  the time the policy names (`policy/chat-health.toml`), and what the session is asked and told
  around it stands there too, read by agency's server.
- Compaction runs detached after the reply has been sent, never in the path a chat is waiting on; a
  message that arrives mid-compaction is answered as soon as the compaction finishes. A compaction
  failure leaves the session running as it was.
- **A job worker's session compacts against its model's own window,** as every other session does:
  - inside a turn, its Claude Code is launched with no compaction setting of the deployment's, so the
    program compacts at its own default share of the model's window while the job goes on;
    compacting a large session early costs more than reading its cached context again;
  - between turns, the runner folds a worker session past its fold ratio — the table's value for the
    provider it stands on and the tier its run went out on — by the same rule a chat's session is
    folded by.
  - A worker running on a registered model's road is the one exception: it is launched with the share
    its registration leaves for the answer (`docs/own-llm.md`, 「A turn at the input limit」).
  - Whatever a consumer sent the session to hold — a co-work result among it — is folded away with
    the detail, so `session-folded` is published once the fold has reached the session, or may have
    (`docs/api.md`, 「Events」); a fold that never reached it, held off by a busy pane or a session too
    short to fold, publishes nothing.

## A worker's session per model

- **A worker keeps its last Claude Code session on each model.** The session its last turn answered
  on is the one it holds; a session it left for a turn on another model is set aside under its own
  model, not closed.
- **Turning to another model** sets the held session aside and opens a new one on the new model,
  unless a warm session is set aside on that model. The new session opens on its turn's words alone,
  and the session it leaves is set aside rather than closed.
- **Coming back to a model resumes the session set aside on it** only where all of these hold:
  - its cache is warm (the criterion below);
  - it is still the session the worker's record on that model and login names: no reset, lost
    session or job end has taken the record away;
  - it stands on the road the turn goes out on — the same provider, model, effort and login — and
    the prefix it was opened on has not moved.
- **A resumed session is handed nothing beyond its turn.** What the turns on the other model produced
  reaches it as the new turn's own words: in a co-work, the stage results its prompt opens, none of which that
  session has read.
- **A session set aside that fails any of these is dropped** with its record, and the turn opens a
  new session, as on any replacement.
- **The warm/cold criterion:**
  - warm while the time since the session's last answer is under the cache lifetime, cold from the
    lifetime on;
  - the lifetime is the one the provider declares for the model, each provider answering for its own
    models out of its policy file: Claude Code's is 60 minutes for every model, the subscription's
    one-hour lifetime, and Codex and OpenRouter name theirs per model and per answering upstream;
    agency keeps no cache measurement of its own;
  - the judgement itself, the lifetime in milliseconds, the warm/cold line and a record's own life, is
    the `cache-siren` crate's, which is handed the declared lifetime and knows no provider;
  - a provider that declares none reads one hour for a session set aside; a held session on such a
    provider has no lifetime to outlive.
- **The pool asks this rule before a project's own turn launches** (`docs/coworker-pool.md`, 「A
  project's own session」): the session the worker holds on the turn's model and login is carried on the
  request while it is warm and its last answered turn stayed under the model's context limit; one past
  either is dropped with its record, and the turn opens a new one.
- **A kept session's record goes cold with its cache.** A record whose last answer is past the
  lifetime is retired as it is read, so neither the standing program nor a turn resumes a session whose
  cache has gone; the held session it stood for is replaced. `BMO_WORKER_SESSION_TTL_MINUTES`,
  where the deployment sets it, retires a record sooner, never later.
- **Only the worker's own Claude Code sessions are set aside,** since they are the ones recorded per
  model and login. A turn on a stand-in road — Codex, OpenRouter, a registered model — replaces the
  held session (「Session replacement」 below). A turn back from one resumes a warm session still set
  aside on its model, as any return does, and otherwise opens a new session; the stand-in's session
  is never set aside.
- **Nothing keeps a session set aside warm.** The keepalive warms the held session alone, and while a
  turn on another model runs, the worker's program is busy with it. A return after the lifetime opens
  a new session.
- **A job's end ends them all:** the held session is handed over — closed, with nothing carried to
  the next job — and every session set aside is dropped with its record. A stopped stage retried on a fresh session does
  the same, its kept session's record dropped with it. The job's runner asks for each handover across
  the boundary, naming the session where it has one (`docs/api.md`, 「Workers」).
- **What a job's change moved of the prefix is read by the files a session's prefix is built from:** the
  instructions and the tool declaration at a checkout's head, the worker's settings, standing rules,
  report trailer and the graph text it carries, plugin and co-worker definitions, the self-preservation
  rulebook, and a worker's own rules. A runner asks which of its own commits' files those are, to say why
  it hands its session over.
- **Which model and which session a job's turn goes on are its runner's.** The turn names the model its
  record is kept under, the mapping it goes out on and whether it resumes the kept session, opens a
  fresh one or continues one it names; agency carries that choice through the rules above and judges
  none of it (`docs/api.md`, 「Sessions」, a work turn's parts). A turn whose runner named a session
  that the walk, a reset or the judging of the held session has moved off is put to the runner again
  before anything goes in, since its prompt was composed for what that session has read.
- **A session's generation and its context are kept for its state to read:** how many times the
  worker's session on that model was opened fresh rather than resumed, counted beside the records so a
  record forgotten takes no count with it; and on the record, the tokens its last answered turn carried
  and the share of the window they filled.

## Session replacement

- Folding a session is what happens when its window fills. Replacing one is what happens when its
  memory has stopped being worth its price. A write to a document in the system prompt costs every
  session holding it its cached prefix, so the next turn in each of those rooms writes that room's
  whole context again — and a room that has been talking for a day carries hundreds of thousands of
  tokens.
- At the head of a turn, and only there, agency checks whether a prompt document was written after this
  session last answered. If one was, this turn is paying for the whole context whatever happens, so
  the two prices are compared (`packages/agent/freshstart.ts`): resuming costs what the session's last run
  reported it was carrying, which the archive keeps on the chat's `agents` row and which every round
  that got an answer writes, an unfinished `<<WAIT>>` round included; starting fresh costs the
  system prompt alone, since the round's own messages are sent either way. A size
  nothing has recorded, and a size a fold has just made meaningless, both read as unknown, and an
  unknown session is never cut.
- A fresh start is taken only when it saves at least a hundred thousand tokens **and** resuming
  would write at least four times what it writes — a room a little over the line keeps its memory,
  one carrying several times its own weight does not.
- **A closed session hands nothing over.** No summary of a session is made or kept anywhere: the old
  session's record is not read, and the new session opens on its turn's words alone. A chat agent's
  session carries only the note the receiver hands in with its own turn, which is the receiver's.
- The session id is cleared and a new session opens on the next run; the room hears nothing.
- Who the people are reaches the new session by the brain's index, whose people group its first turn
  walks, and the person books, as on every turn.
- A session is never replaced inside a turn: the check runs between rounds, with nothing in flight.

