# Model fallback, context hygiene and the sanitiser gate

Agency launches every model run and reads back which model, provider and account it actually answered
on. Two things can make a run come back on something other than what it was launched on, and agency
carries both.

- The account safeguard **demotes** a run to a lower same-family sibling when it reads the session's
  accumulated context as material it must refuse. The demotion happens at the provider's side, at run
  time, with no advance signal; agency catches it after the fact by comparing the model a run reported
  against the model it launched.
- When no account and no provider answers at all, agency walks a **fallback order** of accounts and
  then providers to find one that will.

Agency also runs the **sanitiser gate** on the words that cross a session boundary it owns, so the
context the safeguard rereads does not keep catching on a passage that was said once and then stayed
in the record.

None of this is a turn-content decision. Which words a turn carries, which room or rule governs it,
which references it loads, and whether an answer is acceptable are the caller's (`docs/api.md`).
Agency's part is the boundary those words cross on the way into and out of the session it owns, the
model it launched, the account the run went out on, and the walk it runs when a login will not answer.

## The sanitiser gate

- **What it is:** one gate run on a small model (Haiku by default), standing where words a caller hands
  agency enter the session context agency carries. It does not rewrite freely: only where there is
  security-penetration material a model's safeguard would read as inciting a real attack does it mark
  that passage out, and it passes the rest through unchanged.
- **Why it exists:** the safeguard rereads a session's whole accumulated context on every run. A
  passage that caught once stays in the record and catches again every turn, demoting an otherwise
  ordinary run. Sanitising that passage before it reaches the record stops the misfire.
- **What it covers, in the abstract:** an instruction to attack a real target, a report that copies
  the results of such an attack, and a workaround written to slip either past a safeguard. The
  safeguard reads all three alike and all three pile up in the record, so the gate defuses all three.
- **What it leaves alone:** a room's rough tone, work a speaker does on their own system or their own
  model, and administration of a machine the speaker owns and consents to. What divides those from an
  attack is whose machine it is and whether its owner consents, not the words used, so the gate does
  not touch them.
- **The point:** not to make a model carry out anything, but to make the context the safeguard reads
  harmless. A refused turn helps no one — the message it carried was ordinary and nothing could have
  been carried out in any case.

### The incoming side

- **What passes through it:** a round's new messages, the past-record block a first turn reloads, the
  summary a session replacement hands over as its seed, reference pieces loaded into a round, the
  quiet instructions a search or browse run passes with someone else's page, and the prompt handed to
  a job worker.
- **The caller that carries the words asks the gate,** through one memo of verdicts, so the same words
  are not judged twice. Words that enter through these roads are written into the session transcript
  already sanitised, so later contamination from that record is blocked.
- **A document is cleaned where it is written,** by whatever writes it, not on every read — what was
  written once is loaded again and again.

### The outgoing side

- **What passes through it:** every place an answer agency delivered goes back to a room — the
  session's answer, alerts, captions, and what a voice message speaks aloud.
- **Why clean outgoing words:** an outgoing word is soon an incoming word. A sent message stays in the
  record as it was and is reread into the session context, into the digest that writes documents, and
  into later searches, so an attack instruction or a copied report mixed into it returns on its own
  feet days later and touches the safeguard.
- **One clean before the message exists,** so the words a room reads, the words a later gate judges and
  the words kept in the record are one and the same text. Spoken words are cleaned before the sound is
  made, so what a room heard never disagrees with its record.

### The verdict travels with the words

- **One judgement serves every stage after it.** The same text crosses many boundaries — a message
  into a session, the session's answer out again and into the store, a job's text into a worker, a
  candidate into a gate. Text is handed on as a payload that records whether the sanitiser has read it,
  what it made of it and where; a payload already sanitised passes straight through, without the
  pre-screen and without the model.
- **Where a boundary can carry nothing but a string** — a prompt written to a file for another
  process, a message read back out of the store on a later turn, a document on disk — the flag cannot
  ride along, so the verdict is written under a hash of the words themselves and whoever next holds
  those words finds it again.
- **That memo also keeps the answer the same for the same words,** which a model asked twice does not
  promise on its own. It is swept of anything older than ninety days at each start.
- **A job worker's prompt passes the same sanitiser before the run,** since a worker runs on the same
  safeguard and falls the same way: the item's instruction, the owner's note and the account of the
  runs before it are one text, and an order to attack quoted anywhere in it is defused there, with the
  fixed report trailer appended afterwards so the shape a job is held to is never touched.
- **A quoted order counts as much as a fresh one,** in a room or in a job, because the safeguard reads
  the words and not who is said to have said them.
- **Markup handed in with words the sanitiser changed is dropped** rather than sent on.
- **The gate is `ts/agent/sanitizer.ts`,** recorded and costed like the other gates; it is not kept
  warm, since it runs only when a word makes it worth asking.
- **What it cannot reach is a model's own record of what it wrote:** the answer in that transcript is
  the model's own words, so a session can still put an order back into its own context. That is what
  the downgrade heal is for (「Demotion detection and self-healing」).

### What is already stuck

- **The gate does not edit a session's on-disk transcript,** so a session already carrying an
  instruction still reloads it under an ordinary resume.
- **When it clears on its own:** when the session is replaced, the replacement's summary now passes
  sanitising, so the new session opens on a clean seed and the gate keeps it clean thereafter.
- **To clear it at once:** open a new session for that room, which now takes a sanitised seed.

### When it runs, and the cost

- **A cheap local keyword pre-screen stands in front of the gate:** if no attack-meaning word is in the
  context, the gate is not called at all, so an ordinary turn pays nothing.
- **Every incoming road gets the pre-screen,** not one particular room, since a new instruction can
  enter any of them.
- **The cost:** on the rare turn a word catches, one small-model run runs, recorded in usage like any
  other gate. The pre-screen presses that cost near zero, which is what lets the gate stand on every
  road contamination can enter rather than only in an already-contaminated one.

### The answer's shape

- **The gate does not rewrite text.** With no material it answers the single word `CLEAN`; with
  material present it answers the passages to mark out, one per line after `DEFUSE: `, copied verbatim.
- **A passage is kept within one line of the text,** and the target's own name is left outside it so it
  stays in the text.
- **Agency marks out every place that passage appears** and touches nothing else. Because the answer's
  length is unrelated to the text's, even a long text gets a short, fast answer.
- **When a passage cannot be found verbatim,** it is searched for again allowing only spacing to
  differ; if it is still not found, a local mark-out is applied once over the whole text, since the
  gate saw material but could not point at the place.
- **A whole-text rewrite instead of passages** is used as is only when it is longer than half the
  original, and otherwise replaced with a local mark-out. An unintelligible answer is replaced the same
  way.

### Long text

- **When the gate's road limits request size** — an own LLM service does (`docs/own-llm.md`, 「A turn at
  the input limit」) — text that exceeds the limit is not handed whole to another road but split and put
  back on the same one.
- **Splitting** cuts in two around the middle — at a paragraph boundary, else a line boundary, else by
  character count — and each half is put in again; a half still too large splits again. Joining the
  pieces back differs from the original by not one character, and each piece is told it is one piece of
  a longer text.
- **Each piece gets the pre-screen again,** so a piece with no attack word is left as is. One piece is
  one gate run with no fallback, so a piece the road could not answer for is given a local mark-out for
  that piece alone while the other pieces' verdicts stand, and the result is joined back in order.
- **On a road with no size limit it does not split.**

### The boundary

- **This is a context-hygiene device, not a way to bypass a safeguard.** It stops the safeguard's
  misfire — refusing an ordinary new conversation every turn because an old passage stayed in the
  record — and it does not make any model carry anything out.

## Demotion detection and self-healing

### The mechanism

- **A session is always launched on the model its work key's tier names.** A sibling is never chosen at
  launch; when the safeguard reads a session's context and refuses, the provider answers at run time
  with a lower same-family sibling instead. There is no flag that turns this off, and the run stream
  carries no advance signal.
- **So a demotion is caught after the fact:** if a run reports having spent tokens on a same-family
  sibling of the model it was launched on, that turn was demoted. The comparison target is the launched
  model itself, read from the same tier the line was launched on, so a model the owner changes on the
  console moves the check with it.
- **Not a demotion:** the small model a CLI uses for its own chores — titles, summaries — is a
  different family, and a run launched by a short model name cannot be told here what the CLI resolved
  it to, so neither is read as a fall.

### First catch: notify and heal

When a demotion first catches — when a healthy session first drops — two things happen.

- **Notify:** the owner's private chat is told which room dropped to which model. The room itself is
  not told. (Agency raises the fact; the chat agent is what tells the owner.)
- **Heal:** that room's session is discarded so the next turn opens a new one, which takes a sanitised
  summary as its seed and so restarts on a context the safeguard does not catch.
- **Once only:** the notice and heal happen on the turn a room started dropping, not on every turn it
  stays dropped, or a room that keeps touching the safeguard would be reset every turn to no effect.
- **The latch:** once dropped and then cleared, the latch releases, so the next fresh drop notifies
  again.

### Limits

- **The safeguard is probabilistic** — the same context does not catch on every run, but catches on
  most. Detection catches every run that actually fell, after the fact, so it works whatever the catch's
  probability.
- **A place where such work is discussed keeps needing healing,** since the detect-heal loop reverts a
  drop but a conversation that keeps returning to attack material keeps re-contaminating itself.

### Job workers stand on the same safeguard

- **Same position as a room:** a worker runs on with one session per model, so it sits where a room
  sits. A job's input passes sanitising, but words already stuck in the session transcript reload as
  they are, and then one job quietly answers on a sibling model and ends — nobody the wiser, and the
  next job inherits the same session.
- **One function for both:** detection uses one function for rooms and workers — compare the model a
  run reported against the model chosen at launch; if tokens went to a different same-family sibling,
  it was demoted.
- **The one difference** is the comparison target: for a worker it is the model that job launched. A
  job launched on one family that answered on another is not a fall.

#### Healing differs for a worker

- **A room** hands a sanitised summary as a seed to its new session so conversation continues.
- **A job** has nothing to hand over — it brings its own instructions and needs no memory of past jobs
  — so a worker's healing is only the other half: delete the refused session's record, and publish
  which worker dropped to what as `worker-downgraded` (`docs/api.md`, 「Events」). The consumer that
  owns the owner's room reads that fact and explains it to the owner. The next job opens a new session.

#### A worker the owner keeps on its fallen model

- **The switch:** each worker row carries whether its downgrade heal is on. It is on by default; the
  owner turns it off per worker from the console's worker editor or with
  `bmo-agency agent --set <worker> --downgrade-heal off`.
- **What off means:** the fall is still read and the owner is still told which worker fell to which
  model, but the session is kept and recorded as any answering session is. The next job on that worker
  and model resumes it and goes on answering on whatever model the safeguard allows.
- **The record says it fell:** the kept session's record stays under the model it was launched on — the
  key every later run asks for — and carries beside it the model that actually answered.
- **A turn on it goes out on the fallen model:** every later turn that resumes that session — the next
  job, the next stage of a co-work, a keepalive ping, a word to the worker, a fold — is launched on the
  model that answered, not the one its tier or key names, since launching the tier's model again only
  puts the same context back in front of the same safeguard. A turn that answers on the fallen model
  keeps the mark; one that falls further marks the lower model.
- **The session's held state follows it:** the model and prefix held for the session are those of the
  model that answered, so the next turn on the fallen model is read as the same road and resumes rather
  than being replaced with a summary.
- **A fresh session carries no mark:** where a session is opened anew — a job's handover, a lost or
  expired record, a prefix that moved — it is launched on its own model again and falls only if the
  safeguard refuses it afresh.
- **Heal on ignores the mark:** a worker whose heal is on launches on its own model whatever a record
  says, and a fall there drops the session as before.
- **Only a worker's own:** the switch is the worker's alone, and a room's heal is not touched by it.

#### When to delete

- **At the point a refused run's answer is received,** that session is deleted instead of recorded.
- **Not while a job runs:** deleting then would be undone, since the job writes its own session id again
  at the end. For the same reason a deleted session is not compacted.
- **Keepalive pings on that session are runs on it too** and can be refused likewise; a refused ping
  deletes the session instead of warming it.

#### What is not touched

- **A job's ending is unchanged** (the job unit decides its ending branches). Work done is done, work
  not done is not, and a session splitting underneath is not an attempt on the item.
- **Other sessions:** the worker's other-model sessions, and other workers' same-model sessions, are
  left alone.

#### The cost

- **Deleting makes the next job fill its context from scratch,** so the only condition to delete is a
  run reporting a fall in its own spend — no clock and no reading of the transcript deletes a session.
  Only a run that was actually refused tells you its context is the problem.

#### Limit

- **A worker can launch co-workers,** and a run's spend catches the models they used too, so a
  co-worker explicitly given a different version of the job's own family is read as a demotion though it
  is not a fall. In practice agents pick models by family name, so the case does not arise, and even a
  misread costs only one new session — the answer is not wrong — where a job quietly answering on a
  lower model and ending costs more.

## When no account answers at all — the account order and the provider fallback order

- **Above demotion, one more layer.** Demotion is an account still answering, at a lower sibling. This
  layer is an account answering nothing at all: the road then goes to the next account, and once no
  account is left, down the provider fallback order.

### One order, set by the owner

- **The list:** one list laying every provider agency carries — Claude Code, Codex, OpenRouter,
  registered models — out once each, used together by every work key and every worker, with an on/off
  beside each. The Claude Code entry is itself a road with a list of accounts inside it (「The Claude
  Code road is a list of accounts」).
- **The default:** all on, Claude Code → Codex → OpenRouter → registered models, held in code only. A
  key's walk asks only the providers that carry its line, and the registry's provider only where the
  registration the key's tier maps to is walked (`docs/own-llm.md`).
- **What is saved:** the order the owner changed and the providers turned off are one value in the
  store — a list pairing a provider name with its on/off — so the two never disagree, a turned-off
  provider keeps its place, and re-enabling it returns it to where it was. When the value equals the
  default, it is deleted. A value not holding every provider the build carries, once each, is not saved.
- **A saved order that lacks a newer provider** keeps every choice it holds and gains the missing
  provider, on, at the end; a value of any other shape is read as the default. Every reader reads it so,
  including the check a pane makes just before it starts a model.
- **Outside the console:** `bmo-agency agent --provider-order <provider,…>` sets the order,
  `--provider-on <provider>` and `--provider-off <provider>` change on/off, and with no argument the
  order shows at the top of the listing.

### The Claude Code road is a list of accounts

- **Agency holds several Claude logins, and each is an account:** a name the owner gives it, a place in
  the account order, an on/off, and where its credential is reached.
- **What an account is known by is not what it is called.** Every account carries an identifier of its
  own that never moves, and that is what its sessions, its usage windows, its rest and a key's choice of
  account all name; the name is the owner's to rewrite at will, and rewriting it changes nothing the
  account already holds. Two accounts share neither, so a list never holds two that cannot be told
  apart.
- **A credential is reached one of two ways.** An account either names an absolute configuration
  directory, whose login the CLI keeps in that directory's own credentials file, or names a file
  holding the long-lived token the CLI issues, which is handed to a run in the environment. That token
  lasts a year, carries no refresh of its own, and is inference-only.
- **What is kept is where a credential is, never a credential.** No token and no file body enters the
  store, a log line, a document or a report; the reader that opens a credential file keeps only the
  moment it expires.
- **The account is handed over in the environment, never in a flag.** Every line's flags, its system
  prompt and the tools it is shown are what they were, so a run moved to another account stands on the
  line it always stood on and only the cache it is warm in changes. A directory account is handed its
  configuration home, and any token the launching process carried goes with it, since a token left in
  place would answer for the run instead of the login the owner chose; a token account is handed its
  token alone.
- **An account carries its own everything.** Its sessions, transcripts, usage windows, spend and rest
  belong to it, because a login is what they belong to: a session opened under one account cannot be
  resumed under another, one account's windows say nothing about another's, and each account's cache is
  warmed on its own.
- **The order holds the providers; the Claude Code slot holds the accounts.** The account order is its
  own list beside the provider order, and it is what the Claude Code entry walks when its turn comes.
- **The account order is each account's own place and on/off, held together,** so the two cannot
  disagree and a turned-off account keeps its place. An order is refused unless it holds every
  registered account once each with at least one on.
- **With nothing said, the order is the order the accounts were added in, all on** — what a fresh
  installation has and what the owner resets it to. Unlike the provider order there is no stored value
  to remove, since the accounts are the owner's own list from the start.
- **A fresh installation has one account,** standing for the login the machine already holds, so
  nothing answers differently until a second is added.
- **Outside the console,** an account named by its id or its given name:
  - `bmo-agency agent --account-order <account,…>` sets the order
  - `bmo-agency agent --account-on <account>` and `--account-off <account>` change on/off
  - `bmo-agency agent --account-reset` puts the order back to the default
  - with no argument the account order shows beside the provider order, marked where it is still the
    default

### The login in use is used to the end

- **One login is in use, for the whole road.** Every automatic run — a chat round, a job, a gate
  judgement, an editor batch, a research branch, a session opened after a restart — starts its walk on
  that login for as long as it can answer, ahead of the chosen account and the order.
- **A login coming back moves nothing.** An earlier or chosen account answering again is no reason to
  leave the login in use; the road moves only when that login stops answering, or when the owner
  chooses an account. A session and its warm cache belong to the login that opened them, and the
  owner spends one account's window before the next, so moving off a login that still answers throws
  the sessions away for nothing.
- **What is kept:** one setting naming the login in use, so a restart or a deployment starts every run
  where the road stood. It is written by the first run that answers while no login is in use or while
  the one in use cannot answer, and by the owner's choice.
- **A login in use that cannot answer is walked past like any other** — off, removed, at its limit or
  waiting for a person — and the walk goes on from the chosen account and then the order from the top;
  the login that answers there becomes the one in use. A key pinned to an account is offered that
  account alone and never moves the login in use.

### Running on one account now

- **The owner can name the account the road runs on now,** from a row on the console or with the
  command below. That account is the chosen one, and choosing it is a switch: it becomes the login in
  use at once, so the next run of every key goes out on it. Whenever the login in use cannot answer,
  the walk goes on from the chosen account first, then every other account that is on, from the top.
- **The choice leaves the order alone.** It is one setting beside the accounts' rows, naming one account
  by its id; neither the order nor any switch is rewritten to express it. With nothing chosen the walk
  is the account order as it stands.
- **Only an account that is on can be chosen.** Choosing one that is off is refused; turning the chosen
  account off, removing it or resetting the order lets the choice go with it.
- **A chosen account that cannot answer is walked past like any other,** and the account that answers
  becomes the login in use. The choice stays, but the login in use is used to the end first and the
  chosen account is only where the walk starts again once that one stops answering.
- **What moves, and when.** The choice changes the head of the walk and nothing else, so everything that
  already happens when the head moves happens at the next run, not at the click: a run already going
  finishes on the login it went out on; the next chat round summarises its old session and opens a
  fresh one on the chosen login; the next gate, editor and research branch go out on the chosen
  account; the next job resumes the worker's session kept under that login, or opens one. A turn on a
  session already held stays on that session's own login, since no other login can reach it.
- **A key pinned to an account is not moved** — the choice decides where automatic keys go.
- **Outside the console:**
  - `bmo-agency agent --account-use <account>` chooses an account by its id or name
  - `bmo-agency agent --account-use auto` lets the choice go, back to the order
  - with no argument the listing names the chosen account and the one runs go out on now

### A login that is about to end

- **A login does not last.** A directory account's login is an OAuth pair the CLI renews inside its own
  run; the long-lived token an account may point at instead stands for a year and then simply dies,
  carrying no refresh. No command renews either from outside a run.
- **The moment a credential states is not the login's end.** A subscription login's file states when
  the access token in it stops being accepted, about an hour away at any moment, which the CLI renews
  for itself out of the refresh token beside it. So a file that holds something to renew with is a login
  with no end this side can read, and the two questions are kept apart: the moment the file states is
  what a renewal is timed against, and the login's own end is what a person is told about.
- **What does end:** a long-lived token, which holds no refresh and dies a year after it was issued;
  and a login whose refresh has stopped being taken, which no file says and only a run that went out on
  it can tell.
- **Only moments are read, never the credential.** A credential file is opened to learn two things —
  when its access token stops, and whether it holds anything to renew with — and nothing else. No token
  and no part of the file's body enters a row, a log line, a document or a report. Both are looked for
  as shapes rather than fields, since the file's shape is the CLI's and free to move. A token file
  states no ending, so what stands for its year is the day the file was written.
- **A login whose end cannot be read is left standing and nothing is said about it.**
- **One renewal, not several.** Runs of one account share that account's configuration directory, and
  two of them renewing at once would leave each holding a credential the other has replaced. So a run
  whose login is near its end takes that account's own lock on the way out: the holder is the run whose
  CLI renews, and the others wait and then go out on what that renewal wrote. The lock is held only
  while the login is inside the last half hour of its life, and a wait that cannot be had in time lets
  the run go out rather than losing the work.
- **A long-lived token removes the renewal:** an account pointed at one needs no browser for a year, so
  none of the above happens for it; it is warned about before that year is out like any other account.
- **An account's token for the work nodes is a login of its own** (`docs/work-nodes.md`, 「The logins on
  a node」), counted and warned about separately; a node login found gone is marked for the nodes alone,
  and the account's login here is neither marked nor passed over for it.
- **A login that has ended is marked, and the mark is the walk's instruction:** the account's row says
  it waits for a person, the walk passes it over, and its work moves to the next account and then the
  next provider.
- **The mark is lifted by the credential, not by anybody's word.** Signing in again writes a moment in
  the future, and the walk's own reading of that credential takes the mark off, so no console action is
  needed to put the account back in the order.
- **The owner is told before it ends, once — and never about a login that renews itself.** Agency
  publishes `account-login-notice` (`docs/api.md`, 「Events」), carrying the account, how long is left,
  and the one command the owner runs to be done with it, with that account's own configuration
  directory in front of it; the consumer that owns the owner's private chat says it there. The command
  is passed over verbatim rather than reworded, because a command reworded is a command that does not
  run.
- **Once is per ending, not per login.** The warning is recorded under the moment it is about as it is
  published, so a later pass over the same ending publishes nothing; a login signed in again is a new
  moment and is warned about again. Delivering it is the consumer's.
- **Recognising that a login needs a person rests on more than one word** — the run's ending read
  through the program's own failure reading, and the credential's moment — asked together in one place,
  with a credential plainly past its end enough on its own.
- **The watch runs on the pass that keeps accounts warm,** so the marking, lifting and warning happen
  without anything being launched for them. Its wording, how long before the end it speaks and the year
  a token stands for are read from agency's policy (`policy/`).

### Availability

- **Order sets only order and on/off.** Whether an on provider is usable now, each provider answers for
  itself:
  - Codex — its executable is present
  - OpenRouter — a model is registered for that key and the environment key is present
  - registered models — the gateway's key file is present (`docs/own-llm.md`)
  - Claude Code — the account judgement below, asked of each account on its own
- **An unusable provider is skipped** in its turn, and so is an account the judgement closes.

### The account judgement

- **One judgement says whether a login answers now, and every Claude run is offered a login by it** —
  chat rounds, jobs, a word said to a worker, folds, gates, editors, search and browse, the warm pings,
  a worker's standing program and a companion on Claude Code's own road. No line keeps a judgement of
  its own, and none falls back to the machine's own login in place of one.
- **A login is open while all three hold:** it is on; no window of its own is full (neither a limit rest
  standing under it, nor a fresh reading of its own windows showing one at its ceiling that has not
  reset — `usage.md`); and its login has not gone (「A login that is about to end」).
- **A fresh window read full closes its login** whether or not any run has met the refusal yet. The
  windows are read on a clock from the account's own login, so a login spent elsewhere is closed by the
  reading, and a rest removed by hand does not reopen a login whose window is still full.
- **A full reading is fresh for a bounded window and no longer,** since a quota can come back before the
  reset its reading names and nothing tells this side but the next reading. A reading the reader cannot
  renew stops standing for the window, so the walk neither sends a run onto a login known spent nor
  leaves a login shut forever on a reading no one can refresh.
- **A stale full reading closes nothing, and a run settles it:** the next run goes out on the login — one
  that answers renews its own token and takes a fresh reading, one still spent meets the provider's
  refusal, which rests it as any refusal does.
- **A rest written from a run's refusal or its own usage is not a reading:** it closes the login until
  its moment whatever the reader later finds, so a login a run found spent is never sent another run
  before the refusal's own moment.
- **A closed login is never offered.** The walk offers the open logins in its order and none other; a
  key pinned to a closed login is offered none.
- **A run none of whose logins answers goes out on none.** It ends without launching anything, and
  whatever receives that failure reads it as it reads any limit or lost login, beginning no rest, since
  the run went out on no login.

### Automatic walks only on providers, in order

- **The Claude turn:** if Claude Code is on and its turn is reached, the login in use answers where it
  can (「The login in use is used to the end」); otherwise the account order is walked from the top — from
  the chosen account where one is chosen — and the first open account answers. The Claude Code turn is
  closed only when every account is.
- **A run that fails on the account rather than on the work** — its limit reached, its login gone — moves
  to the next open account before the walk leaves the Claude Code turn, and the work that asked never
  learns the first account was tried. A failure of the work itself moves nothing, and neither does the
  service being down, since no other account would answer that either. A login passed over for its limit
  is put to rest as it is passed, under that login, so nothing that reads the failure afterwards writes a
  rest under another. Where that rest closes the last open login, agency publishes it once as
  `deployment-resting` (`docs/api.md`, 「Events」), and the consumer holding the owner's chat says it.
- **A login gone, or its token for the work nodes, is told the way the login watch tells it:** marked as
  waiting for a person and published once as `account-login-notice`. Agency says nothing in a room
  itself.
- **A standing program left on a closed login is taken down.** The loop in an agent's pane starts the
  program its place holds again on the login written into it, so the login watch takes down every
  standing program whose login the judgement closes, unless a turn is in it; the agent's next turn stands
  its program again on the login the walk offers then. A program that names no login of its own is the
  machine's and is left alone.
- **Which of the two a failure is, is one judgement,** read off the run's ending through the program's
  own failure reading, in one place every run asks, so no two lines read the same failure differently.
- **A line that spawns once takes the first open account rather than walking.** Search and browse runners
  are each one process given the account the key is offered first; a worker's standing program and a
  companion's turn on Claude Code's own road take the first open account the same way. Everything that
  can be launched again is carried account by account.
- **Walking down:** if a turn's provider is not run or ends with no answer, the walk goes to the next
  turn. With Claude Code off, an automatic key starts straight from the next enabled provider, and if
  nobody answers to the end the run fails and does not fall back to Claude.
- **Who receives that failure, each in its own place:**
  - **Chat** reads it like the API not answering, enters a rest state, and notifies the owner once; with
    a usable substitute left, the next message tries the walk again.
  - **An editor** records that its batch failed.
  - **A job** tries every enabled provider once and ends saying every usable provider is blocked. A road
    that did not answer is passed over by the jobs after it for a while — a minute for an outage, five
    for a refused access, fifteen or until its reset for a limit. On a provider whose models are
    services of their own, that road is the one registration that did not answer, and one registration
    going quiet leaves the others open (`docs/own-llm.md`).
    - **A turn stopped from outside closes no road:** a turn that ends without an answer while its job has
      been stopped is the stop, on every road.
    - **A request the road refused closes no road:** a registered model's turn that ends on a 400, 413 or
      422 — a conversation grown past the registration's window among them — is the backend answering the
      request, not the road gone (`docs/own-llm.md`, 「A turn at the input limit」).
    - **A registration closed for an outage is asked again before it is passed over:** a job's walk asks
      the engine behind the registration for its health, and an engine that answers opens the road again
      at once, while a registration whose engine gives no health waits out its minute.
  - **A gate** is not in this walk at all — a gate runs on its one provider and, on failure, ends as a
    gate with no answer (`docs/own-llm.md`, 「A gate has one road」).
- **Re-checked just before running:** on/off is checked again just before a run starts. A provider turned
  off while waiting for a lock is not launched anew, a request already running is left to finish, and
  Claude's run line is unchanged. While Claude is off, chat and worker context folding is deferred and
  session records preserved, and branch-prefix warming is not sent.
- **A job whose worker key is set to one provider** runs only on that provider while it is enabled, and
  recovery has no other to move to; if there is no candidate at all, it ends as an explicit cannot-run. A
  work order's turned-off model is matched to the role tier of the next enabled provider, and recovery
  and the next stage's place reservation use the same tier.

### The per-key choice is automatic or one provider

- **Run modes:** a key's run mode is automatic or one provider that carries its line — Claude Code,
  Codex, OpenRouter, the registry's provider — and, for a gate whose profile offers it, a dedicated gate
  provider (never a turn of the walk). Automatic is the walk above; a single provider runs that key on
  that provider only and, with no answer, fails rather than walking on. While a chosen provider is turned
  off, the key walks like automatic.
- **A key names its account as well as its provider.** Beside the provider, every work key says which
  Claude account its runs go out on: automatic walks the account order, and a named account runs that key
  on that account alone. A key pinned to an account fails on it rather than moving, and while that account
  is off the key walks the account order as automatic does. So the chat agent, each gate, each editor and
  each worker can sit on an account of its own.
- **Why order alone cannot express it:** putting a provider or an account at the front of its order moves
  every automatic key at once, so a key that must stay on one provider or one account says so on its own
  row.
- **Refusal:** a provider whose program cannot carry the key's run line is refused when it is chosen, with
  the reason the console shows beside it.
- **The canonical name of a road is the work key,** not a Claude model name — `bmo`, `job-worker`, one
  per gate, one per editor — the same key in settings, the store and the console. If a key's work moves
  from one Claude model to another the substitute choice does not move on its own, and each provider's
  choice is kept on the key when its mode moves elsewhere.

### How a job worker, a gate, a chat and an editor cross over

- **A gate** carries one system prompt and one request as is.
- **A chat** carries the request its caller assembled with recent conversation, and its tool presence
  and write rights, as a common run contract.
- **A job worker** carries the whole job prompt and the checkout, the repository instructions, the
  worker's rules, the web MCP and work rights. The rules go where each program keeps them beside its own
  default prompt (`docs/launch-lines.md`).
  - **Codex** keeps the thread id `codex exec` produced, per run scope, model and effort, and continues
    with `codex exec resume <id>` under a cross-process lock, one request at a time. A dsh headless run
    has no resume interface and is stateless.
- **An editor** is a single run carrying file tools and the checkout as is, so it keeps no session — on
  Codex as the shell in a workspace-write sandbox, on dsh with filesystem tools and no shell — and the
  only writable places are the checkout and this machine's temporary directories.
- **Search and browse runners do not cross over:** the runner is outside the order, so even with Claude
  Code off a search or browse request runs on Claude, but its keepalive warms Claude's prefix only while
  Claude Code is on.

### A turned-off provider is not used on fallback keys

- **What is skipped:** an automatic key, a key set to one provider, and even a job launched with an
  explicit provider and model are not run on a turned-off provider, and its session is not warmed, since
  turning off is the owner saying not to use that provider and one key's setting does not override that.
- **A turned-off account is passed over the same way:** the Claude Code entry does not walk it, its
  sessions are not warmed, and a key pinned to it walks the account order as automatic does while it is
  off.
- **No per-model off:** a key uses one model per provider, so turning off a provider removes those models
  from fallback, and changing one key's model is done by that line's model selector.

### Warming only enabled providers

- **The persistent switches.** Two stand and both hold until thrown back: the whole round, which stops
  every provider's warming at once; and each agent's own, on that agent's row. A worker whose own switch
  is down is passed over by every warming of its sessions and takes its jobs exactly as it did — warming
  is upkeep, never the road work runs on — and nothing but the switch coming back up starts its warming
  again. Set it with `bmo-agency agent --set <worker> --keepalive off`.
- **A worker turned off until its next job.** Besides the persistent switch, a worker can be turned off
  once with `bmo-agency agent --pause-keepalive <worker>`: a mark on the worker's row that every warming
  of its sessions passes over while it stands, cleared by the next job of that worker as it starts. It is
  not a switch, and the persistent switches still win while they are off.
- **Claude off:** while Claude Code is off, no Claude keepalive goes out. **One account off or resting:**
  its own sessions are not warmed and its prefixes are left to cool, while every other account's are
  warmed, since a prefix belongs to the account it was built under. **Codex off / OpenRouter off:** that
  provider's sessions are not warmed. An unwarmed session's record is left in place, and re-enabling a
  provider judges the next warming from the time that record names.

### The Codex substitute is the Codex CLI

- **How it runs:** `codex exec` non-interactively, using the `codex login` already done on this machine;
  agency stores and passes no OpenAI API key, and does not load Codex's user settings or exec rules.
- **Chat and gates** replace the default coding-agent instruction with an agency-made system prompt file
  via Codex's official `model_instructions_file`, and turn off project instructions, web search, apps,
  memory, goals and Codex subagents. The prompt body is passed via stdin so a large document does not
  stand in the process list; gates have no shell, and chat holds a shell only when the original run held
  tools. A privileged chat allowed all tools uses `--dangerously-bypass-approvals-and-sandbox` so it
  makes no sandbox and reaches agency's loopback scripts.
- **A job worker** reads Codex's coding-agent default instruction and the checkout's `AGENTS.md`,
  attaches the MCP the repository hands it, and with the same bypass rights as the primary worker
  finishes commits, pushes and deploys. The worker's rules go in as Codex's `developer_instructions`,
  standing beside the default instruction rather than replacing it.
- **The system prompt file** is written freshly by the launch that builds the run, just before running;
  a running run is never cut off by a file-not-found, and a file no launch uses any more disappears with
  the old document tree on the next deployment.

### The OpenRouter substitute is the dsh road

- **When:** in the default order, dsh headless is launched only when the Codex process is absent, ends
  with an auth, limit or model error, or gives an empty answer.
- **Only when:** this road stands only when an OpenRouter model is registered for that work key and the
  OpenRouter environment key is present. The key is in the run environment only and reaches neither the
  store, the temporary overlay, logs, the control API nor the browser, so the other substitutes are
  usable even without OpenRouter configured.

### The registered models are the last road of the default order

- **Only when:** the registry's provider stands only when the gateway's key file is present, and answers
  with the registration saved for that key's tier.
- **How:** a tool-less run is one request to the gateway; a run with tools is Claude Code on the gateway
  (`docs/own-llm.md`). Whether thinking is on and the ceiling of answer tokens are saved on the registry's
  mapping of the key's tier.

### The default substitute models

- **Not a fixed launch line:** the console reads the installed CLI's model list and each model's
  supported efforts, lets both be changed per work key, and offers a bench-default restore; OpenRouter
  models are chosen separately on the same key. So changing one key's substitute model and effort moves
  no other key's.
- **The grounds for a default:** defaults are set per work key to the closest same-method match for that
  key's primary Claude model, read from an independent benchmark the console cites, never a claim of
  exact equivalence. A single selector to set every key's mode at once is not offered; setting an
  automatic key's preferred provider is done by putting that provider at the front of the order and, if
  needed, turning off the ones before it.

### Keeping the mode across cache, session and usage

- **The roads answer differently and do not pretend to be the same.** Every common answer carries the
  mode that actually answered (`claude-code`, `codex`, `openrouter`, `registered`) with its model and
  effort, and a measured usage row keeps mode and model as a pair, so models of the same name on
  different roads are not merged. Agency reads those figures off each road — Codex's JSON events,
  OpenRouter's stream usage read through a loopback tab opened only during the run, Claude's own answer —
  and hands them to the usage collector, which splits and shows them (`usage.md`). A run whose stream
  carried no usage gets no invented figures.
- **Each window and run belongs to a slot:** the provider that reported it, and the account too where
  that provider holds logins, built by one constructor so no writer decides for itself whether an account
  belongs on a row. Two accounts report windows of the same names, so neither can overwrite the other.
- **Which provider and account serve now is answered in one place** — the one the road answered on (a
  recorded stand-in while a fallback stands, otherwise Claude Code, and the pinned provider for a keyed
  run) and, where that provider holds logins, the login the provider walks to now. A stand-in is recorded
  only together with the moment its fallback began, so a key answering on the provider it is set to never
  moves what every other reader takes for the agent serving now.
- **Sessions follow the same principle:** Claude Code sessions and Codex threads are shown with their own
  mode and model, a Claude session with the account it was opened under; only OpenRouter answers are
  stateless with no session id. A session is offered back only to a run on the account that opened it.
- **Cache lifetime and keepalive follow the run mode's policy.**
  - Claude Code reads a one-hour cache regardless of model and pings at forty minutes idle.
  - Codex reads a thirty-minute cache for models that announce one and resumes its thread after twenty
    minutes idle; for earlier models that promise no minimum lifetime it invents none and sends no ping.
    The lifetimes stand in Codex's provider declaration, by the longest model name each begins.
  - OpenRouter warms only when the actual upstream it rode has a cache: the tab reads the upstream that
    answered from the stream, and judges a cache by whether an endpoint of the same provider in that
    model's public endpoint list prices a cache read. The ping goes only to the recorded upstream, with
    fallback off, since a ping warming another provider only spends money. The interval is the same
    two-thirds-of-lifetime ratio the other roads use.
  - The own LLM services require neither cache nor keepalive: no registration announces a lifetime, so a
    registration gets no ping, and while the registry's provider is on a key set to it warms no session of
    another provider.
- **A worker thread and session follow the same record-lifetime setting,** so a thread or session idle
  longer than that is neither resumed nor warmed but opened anew. Every ping obeys the global and
  per-agent keepalive stops, skips a key or worker that chose an own LLM service, and, if a real turn is
  running in the same scope, skips that round without waiting; a worker's ping runs only inside the worker
  lock.

### While a fallback stands, the room is not silent

- **A rest is one account's,** so a room falls quiet only when no account and no provider will answer it.
  Even where every account rests, if the walk meets a usable substitute before the Claude turn it keeps
  taking the room. For this walk a login is closed where its own limit rest stands and its week — the
  window its provider counts as its week, among the windows the cache siren reads for it (`docs/api.md`,
  「The cache siren」) — is used past the rest's weekly ceiling; a login the siren holds no reading for is
  not closed by it.
- **Which model it switched to is operational information,** so it is not told to other rooms. The owner
  is not told before the first answer either; only when a substitute first answers, or later drops to
  another, does agency mark the road answering with the moment the fallback began and publish
  `fallback-answering` with the owner, the substitute that answered before where a fallback already
  stood, and the provider and model answering now (`docs/api.md`, 「Events」). Telling the owner's
  private chat, and in what words, is the consumer's. A key set to one provider answering on that
  provider is a chosen road, not a fallback, so it is neither published nor marked, while an enabled
  substitute answering while Claude Code is off is published as a fallback, since the fact that Claude
  is not answering still holds. When Claude produces an answer with a real session id, the fallback mark
  is cleared and the owner is told it is back.

### The response options each road receives

Each response option the Claude Code line receives is set out below, saying whether a substitute road has
the same. "None" means that program has no such concept, with the reason.

| Option | Claude Code | Codex | OpenRouter (dsh) | Registered models |
|---|---|---|---|---|
| Process time limit | A gate that gate's limit, an editor the editor limit, a chat and a job no clock | Takes the same value | Takes the same value | A gate gives its whole limit to its one request; a run with tools is Claude Code on a registered model |
| Cache lifetime · ping interval | 1 hour · 40 minutes | 30 minutes · 20 minutes for models that announce it, else none | Follows the answering upstream, none if not listed | None. No announced lifetime |
| Ping time limit | Chat 90 seconds, a gate that gate's limit, worker 2 minutes | Same | Same | No ping |
| Keepalive stop · rest | Obeys global and per-agent stops, pauses during its own account's rest | Obeys stops but warms during a rest, since a rest is one Claude account's affair | As Codex | Not applicable |
| Session continuation | Resumes chat and worker sessions | Resumes a thread per run scope, model and effort | None. Headless has no resume interface | None for a tool-less request; a run with tools is Claude Code's |
| Context folding | The caller folds with the CLI's compaction | The Codex CLI compacts its own thread | Not applicable. A fresh context per run | Not applicable |
| effort · thinking | The effort set per line | Passes effort, thinking-off is low | Passes no effort; only thinking-off passes off | Thinking on or off; a run with tools sends medium or low |
| System prompt | Replaces the CLI instruction with a file; worker the CLI default with the worker's rules appended | Replaces via `model_instructions_file`; worker the Codex default and `AGENTS.md`, rules as `developer_instructions` | Goes in as one part of the dsh prompt, no replacement | Carries the gate system prompt in the request; a run with tools as on Claude Code |
| Tool · permission isolation | Tools shown per line and an allow list, settings sources blocked | Moves to a shell and sandbox, loads no user settings | Moves to filesystem/shell rows and the dsh sandbox, no per-command allow list | A run with tools is Claude Code's line |
| Output schema | None. A structured answer is asked in the prompt and validated by the caller | None, for the same reason: a schema option moves the cached prefix | None | None |
