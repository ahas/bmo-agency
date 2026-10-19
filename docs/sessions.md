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
- Agency holds no persona and no lore. The base persona a chat session runs on is handed in with each
  turn as part of its context (`docs/api.md`, 「Sessions」), so a change to it is carried by the next
  turn that hands it in and is no document of agency's to reload.

## Launch pacing

- After a start, the first run of each chat session is spaced from the first run of any other chat
  by at least ten seconds, in the order they came (agency's chat line); a chat's later runs
  go straight through.
- Several sessions starting in the same instant is the shape of abuse and can cost the sessions
  themselves, and a restart is exactly when that would happen: the backlog of every chat, the TODO
  instruction and reload notices all fall due together.
- Instructions that wake rooms are held to the same gap, one cold wake at a time, by the instruction
  pass (`bmo-window.md`, 「Quiet instruction」). Everything else the bot starts — gates, runners,
  digests, compactions — comes one at a time by construction and is not paced.
- The upkeep still waits behind the real work: the digest's first launch comes three minutes after
  start, the people digest's five, the keepalive check's ten. The going-down line is a fixed message
  and starts no session.

## Session compaction

- After every chat turn the program's status line reports how many tokens the session's context
  holds and how large the model's context window is. When a reply leaves the session past the
  threshold (85% of the window, the policy's `fold_ratio`), the chat agent announces in that
  chat (and only there, when that chat's rest notices are on) that it is going to sleep for a
  moment, types `/compact` into its resident program, and announces that it woke up.
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
- Compaction is also when the chat's person book is brought up to date: that chat's people digest
  worker is launched as its own unit just before the compaction starts, and nothing waits for it,
  since it reads the archive rather than the session.
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
  - between turns, the runner folds a worker session past `fold_ratio` of its model's window, the
    same threshold a chat's session is folded at.
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
  unless a warm session is set aside on that model. The new session is seeded with the note of the
  session it leaves, asked of the summariser as on a replacement (「Session replacement」 below) for a
  session of the `switch` kind, set aside rather than closed.
- **Coming back to a model resumes the session set aside on it** only where all of these hold:
  - its cache is warm (the criterion below);
  - it is still the session the worker's record on that model and login names: no reset, lost
    session or job end has taken the record away;
  - it stands on the road the turn goes out on — the same provider, model, effort and login — and
    the prefix it was opened on has not moved.
- **A resumed session is handed no note.** What the turns on the other model produced reaches it as
  the new turn's own words: in a co-work, the stage results its prompt opens, none of which that
  session has read.
- **A session set aside that fails any of these is dropped** with its record, and the turn opens a
  new session seeded with the note of the session it leaves, as on any replacement.
- **The warm/cold criterion:**
  - warm while the time since the session's last answer is under the cache lifetime, cold from the
    lifetime on;
  - the lifetime is asked of the cache siren for the provider and the model (`docs/api.md`, 「The cache
    siren」), which judges every session's requests warm or cold; agency keeps no cache measurement of
    its own;
  - where the siren answers none — none deployed, or none it judges by — the lifetime is the one the
    provider declares for the model, the table the siren is given: Claude Code's is 60 minutes for
    every model, the subscription's one-hour lifetime;
  - a provider that declares none reads one hour for a session set aside; a held session on such a
    provider has no lifetime to outlive.
- **A kept session's record goes cold with its cache.** A record whose last answer is past the
  lifetime is retired as it is read, so neither the standing program nor a turn resumes a session whose
  cache has gone; the held session it stood for is replaced with its note. `BMO_WORKER_SESSION_TTL_MINUTES`,
  where the deployment sets it, retires a record sooner, never later.
- **Only the worker's own Claude Code sessions are set aside,** since they are the ones recorded per
  model and login. A turn on a stand-in road — Codex, OpenRouter, a registered model — replaces the
  held session (「Session replacement」 below). A turn back from one resumes a warm session still set
  aside on its model, as any return does, and otherwise opens a new session; the stand-in's session
  is never set aside.
- **Nothing keeps a session set aside warm.** The keepalive warms the held session alone, and while a
  turn on another model runs, the worker's program is busy with it. A return after the lifetime opens
  a new session.
- **A job's end ends them all:** the held session is handed over with its note for the next job, and
  every session set aside is dropped with its record. A stopped stage retried on a fresh session does
  the same, its kept session's record dropped with it. The job's runner asks for each handover across
  the boundary, naming the session where it has one (`docs/api.md`, 「Workers」).
- **What a job's change moved of the prefix is read by the files a session's prefix is built from:** the
  instructions and the tool declaration at a checkout's head, the worker's settings, standing rules,
  plugin and co-worker definitions, the self-preservation rulebook, and a worker's own rules. A runner
  asks which of its own commits' files those are, to say why it hands its session over.
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
  the two prices are compared (`ts/agent/freshstart.ts`): resuming costs what the session's last run
  reported it was carrying, which the archive keeps on the chat's `agents` row and which every round
  that got an answer writes, an unfinished `<<WAIT>>` round included; starting fresh costs the
  system prompt plus the carried summary, since the round's own messages are sent either way. A size
  nothing has recorded, and a size a fold has just made meaningless, both read as unknown, and an
  unknown session is never cut.
- A fresh start is taken only when it saves at least a hundred thousand tokens **and** resuming
  would write at least four times what it writes — a room a little over the line keeps its memory,
  one carrying several times its own weight does not.
- When the swap is taken, the old session's record is read off the file the CLI wrote under the
  configuration directory of the login that session was opened on — `projects` under that account's
  own directory, and under `~/.claude` for an account with none of its own — never by resuming it:
  resuming is the whole cost being avoided.
- What is taken from that record is the conversation and only the conversation: the chat agent's own answers,
  and out of each round's prompt the messages the room sent and the quiet instructions it was given.
  The rest of a prompt is scaffolding the turn's caller wrote — the chat header, the trailer, and the
  reference block, which carries the room's person book — and none of it reaches the summariser.
- The note is the session summariser's, a hook the caller registers (`docs/api.md`, 「Hooks」): agency
  hands it what closed — the kind of session, the provider, the session id, the folder, the login, what
  was said on a road that keeps no session, the worker's own note where it wrote one, and the room — and
  carries the note it answers. How much of the tail is read, how the conversation is told from the
  scaffolding, the cap, the notes' wording and the context sanitiser the note passes before it seeds the
  new session (`docs/cache-fallback.md`, 「The sanitiser gate」) are the summariser's.
- With no summariser registered, nothing is carried over: the new session opens on its turn's words
  alone.
- The session id is cleared, a new session opens on the next run, and the note rides in front of
  that round as a quiet instruction: the room hears nothing and loses detail rather than the thread.
- Who the people are is not in the note — that reaches the session by the reference gates and the
  person books, as on every turn.
- A summary that fails, times out or finds nothing worth carrying does not cancel the swap, because
  what decided it was the cost of the old context and a failed summary has not made that context
  cheaper; the summariser's note then tells the new session plainly that it remembers nothing rather
  than leaving it to invent.
- A session is never replaced inside a turn: the check runs between rounds, with nothing in flight.
- **The note is written for the line it opens on.** A replacement reaches every line that keeps a
  session, not the rooms alone, and what the new session is handed is read by it as an instruction —
  so a room's note, which ends by telling the session that the swap is nothing the room needs to
  hear, is a room's and goes nowhere else.
- A job worker has no room: what stands in front of it is the job its prompt goes on to state, and
  its note says the same fact in those terms — the session is new, this is what the jobs before it
  were in the middle of or nothing of it could be recovered, and the job written out below is the
  one to carry on with and to answer in full. A worker handed a room's wording holds an instruction
  to stay silent as the first thing its prompt says, and an instruction at the head of a prompt is
  what a run does.

