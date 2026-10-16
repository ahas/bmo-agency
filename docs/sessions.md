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
- The session stays as it is: no restart, no `--resume` into a fresh session, no break in the
  conversation.
- **Common system-prompt documents are reloaded without waking chats.** They mark the shared chat
  prefix as changed. Room-specific instructions, tones and traits are carried in a turn's leading
  context block instead; changes are picked up on the next turn without invalidating the common
  prefix or waking the session. Agency remembers the last successfully applied room context per
  provider session in its store (`room_sessions`). See `unified-bmo.md`.
- Which chats a file concerns follows from its place (agency's reload verb): a knowledge chunk the
  chat its index line is scoped to, or every chat when it is `[shared]`; a person document every
  chat that person has spoken in; the base persona, a tone layer and the lore every chat.
- A chat is told at most once an hour: the first change goes out at once, and what changes during
  the hour after it gathers in the archive (`reload_pending`) and goes out as one instruction naming
  every file, when the hour is up, on the instruction scheduler's next tick.
- Only chats that already hold a session are told — a chat without one has no memory to go stale.
- Who queues them follows from where the document lives. A brain document is runtime data: the
  digest queues its own, from a before-and-after snapshot of the document directories, whenever a
  run succeeds, and work that edits one by hand runs `bmo-agency reload --path <file> [--path <file>...]`
  afterwards, which does the same for the files it names; with `--now` it skips the
  hour, so the notice rides the very next round of every chat concerned, carrying along whatever was
  waiting for that chat.
- A document that ships with the code — the lore, a persona layer, a rulebook, an agent document —
  is read out of the deploy tree, so it is told by the deployment that puts it live, through the
  deploy tree's own reload; the same command run from a checkout refuses such a document
  (`deploy.md`, 「What goes live with the documents」).

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
- **A job worker's session is held to its own model's window,** folded at the policy's
  `fold_ratio` of it like every other session:
  - inside a turn, the worker's Claude Code is launched with `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` at
    `fold_ratio` as a percentage, so the program compacts at that share of its model's window while
    the job goes on, rather than near the whole of it;
  - between turns, the runner folds a worker session past `fold_ratio` of its model's window.
  - A worker running on a registered model's road is launched with the share its registration
    leaves for the answer instead (`docs/own-llm.md`, 「A turn at the input limit」).
  - Whatever a consumer sent the session to hold — a co-work result among it — is folded away with
    the detail, so `session-folded` is published once the fold has reached the session, or may have
    (`docs/api.md`, 「Events」); a fold that never reached it, held off by a busy pane or a session too
    short to fold, publishes nothing.

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

