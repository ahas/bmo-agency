# Model fallback, context hygiene and the sanitiser gate

Agency launches every model run and reads back which model, provider and account it actually answered
on. Two things can make a run come back on something other than what it was launched on, and agency
carries both.

- The account safeguard **demotes** a run to a lower same-family sibling when it reads the session's
  accumulated context as material it must refuse. The demotion happens at the provider's side, at run
  time, with no advance signal; agency catches it after the fact by comparing the model a run reported
  against the model it launched.
- When no account and no provider answers at all, agency walks the **account order** and then the
  run's own **provider order** to find one that will (`docs/provider-order.md`).

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

- **What passes through it:** a round's new messages, the past-record block a first turn reloads,
  reference pieces loaded into a round, the
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
  record as it was and is reread into the session context, into the brain the agent writes as it
  reads, and into later searches, so an attack instruction or a copied report mixed into it returns on its own
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
- **The gate is `packages/agent/sanitizer.ts`,** recorded and costed like the other gates; it is not kept
  warm, since it runs only when a word makes it worth asking.
- **What it cannot reach is a model's own record of what it wrote:** the answer in that transcript is
  the model's own words, so a session can still put an order back into its own context. That is what
  the downgrade heal is for (「Demotion detection and self-healing」).

### What is already stuck

- **The gate does not edit a session's on-disk transcript,** so a session already carrying an
  instruction still reloads it under an ordinary resume.
- **When it clears on its own:** when the session is replaced, the new session opens with nothing of
  the old one carried over, so it opens clean and the gate keeps it clean thereafter.
- **To clear it at once:** open a new session for that room, which opens with nothing carried over.

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

- **A session is always launched on the model its provider order's entry names,** or its repository's
  pin (`docs/provider-order.md`, `docs/repositories.md`, 「A pinned model」). A sibling is never chosen at
  launch; when the safeguard reads a session's context and refuses, the provider answers at run time
  with a lower same-family sibling instead. There is no flag that turns this off, and the run stream
  carries no advance signal.
- **So a demotion is caught after the fact:** if a run reports having spent tokens on a same-family
  sibling of the model it was launched on, that turn was demoted. The comparison target is the launched
  model itself, read from the same entry the run was launched on, so a model the owner changes on the
  console moves the check with it.
- **Not a demotion:** the small model a CLI uses for its own chores — titles, summaries — is a
  different family, and a run launched by a short model name cannot be told here what the CLI resolved
  it to, so neither is read as a fall.

### First catch: notify and heal

When a demotion first catches — when a healthy session first drops — two things happen.

- **Notify:** the owner's private chat is told which room dropped to which model. The room itself is
  not told. (Agency raises the fact; the supervisor is what tells the owner.)
- **Heal:** that room's session is discarded so the next turn opens a new one, which opens with
  nothing carried over and so restarts on a context the safeguard does not catch.
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

- **Same position as a room:** a job's run goes on in the session its role kept at that path on that
  provider and model (`docs/sessions.md`, 「A role's session」), so it sits where a room sits. A job's
  input passes sanitising, but words already stuck in the session transcript reload as they are, and
  then one job quietly answers on a sibling model and ends — nobody the wiser, and the next job at that
  path inherits the same session.
- **One function for both:** detection uses one function for rooms and jobs — compare the model a
  run reported against the model chosen at launch; if tokens went to a different same-family sibling,
  it was demoted.
- **The one difference** is the comparison target: for a job it is the model that job launched. A
  job launched on one family that answered on another is not a fall.

#### Healing differs for a job

- **A room** opens its new session with nothing carried over, as every replaced session does.
- **A job** has nothing to hand over either — it brings its own instructions and needs no memory of past
  jobs — so a job's healing is only the other half: delete the refused session's record, and publish which
  repository's job, under which role, dropped to what as `worker.changed` (`docs/api.md`, 「Events」). The
  consumer that owns the owner's room reads that fact and explains it to the owner. The next job opens a new
  session.

#### A repository the owner keeps on its fallen model

- **The switch:** each repository row carries whether its jobs' downgrade heal is on. It is on by
  default; the owner turns it off per repository from the console's repository tab.
- **What off means:** the fall is still read and the owner is still told which repository's job fell to
  which model, but the session is kept and recorded as any answering session is. The next run of the same
  role at that path on that provider and model resumes it and goes on answering on whatever model the
  safeguard allows.
- **The record says it fell:** the kept session's record stays under the model it was launched on — the
  key every later run asks for — and carries beside it the model that actually answered.
- **A turn on it goes out on the fallen model:** every later turn that resumes that session — the next
  job, the next stage of a co-work, a fold — is launched on the model that answered, not the one its
  order's entry or its repository's pin names, since launching that model again only puts the same
  context back in front of the same safeguard. A turn that answers on the fallen model
  keeps the mark; one that falls further marks the lower model.
- **The session's held state follows it:** the model and prefix held for the session are those of the
  model that answered, so the next turn on the fallen model is read as the same road and resumes rather
  than being replaced.
- **A fresh session carries no mark:** where a session is opened anew — a job's handover, a lost or
  expired record, a prefix that moved — it is launched on its own model again and falls only if the
  safeguard refuses it afresh.
- **Heal on ignores the mark:** a job in a repository whose heal is on launches on its own model
  whatever a record says, and a fall there drops the session as before.
- **Only a repository's own:** the switch is the repository's alone, and a room's heal is not touched by
  it.

#### When to delete

- **At the point a refused run's answer is received,** that session is deleted instead of recorded.
- **Not while a job runs:** deleting then would be undone, since the job writes its own session id again
  at the end. For the same reason a deleted session is not compacted.

#### What is not touched

- **A job's ending is unchanged** (the job unit decides its ending branches). Work done is done, work
  not done is not, and a session splitting underneath is not an attempt on the item.
- **Other sessions:** the role's sessions at other paths or on other models, and other roles' sessions,
  are left alone.

#### The cost

- **Deleting makes the next job fill its context from scratch,** so the only condition to delete is a
  run reporting a fall in its own spend — no clock and no reading of the transcript deletes a session.
  Only a run that was actually refused tells you its context is the problem.

#### Limit

- **A job's run can open subagents,** and a run's spend catches the models they used too, so a
  subagent explicitly given a different version of the job's own family is read as a demotion though it
  is not a fall. In practice agents pick models by family name, so the case does not arise, and even a
  misread costs only one new session — the answer is not wrong — where a job quietly answering on a
  lower model and ending costs more.

## When no account answers at all — the account order and the provider order

- **Above demotion, one more layer.** Demotion is an account still answering, at a lower sibling. This
  layer is an account answering nothing at all: the road then goes to the next account, and once no
  account is left, to the next provider of the run's own provider order.

### Each supervisor and each role walks its own order

- **There is no order shared by everything.** Every supervisor and every role carries a provider order of
  its own, each provider once with the model, the effort and the thinking it runs on there, and a quota
  share; a run goes out on the first provider of it with quota above the share that can answer now, falls
  over on a limit or a failure, and comes back to the top at its next run (`docs/provider-order.md`). This
  document adds what that walk does inside the Claude Code entry — the accounts — and what each provider
  answers for itself as the walk asks it.
- **The owner's switch on each provider stays one for the deployment:** a provider that is off cannot
  answer now, so every order passes it over, and turning it on again returns it to its place in every
  order that names it. The last provider left on is not turned off (`docs/provider-order.md`, 「The
  owner's switch on a provider」).
- **The Claude Code entry is itself a road with a list of accounts inside it** (「The Claude Code road is a
  list of accounts」), walked the same way whichever order names it.
- **Outside the console:** `bmo-agency agent --provider-on <provider>` and `--provider-off <provider>`
  change a provider's switch, and with no argument the switches show at the top of the listing.

### The Claude Code road is a list of accounts

- **Agency holds several Claude logins, and each is an account:** a name the owner gives it, a place in
  the account order, an on/off, and where its credential is reached.
- **What an account is known by is not what it is called.** Every account carries an identifier of its
  own that never moves, and that is what its sessions, its usage windows, its rest and the chosen
  account all name; the name is the owner's to rewrite at will, and rewriting it changes nothing the
  account already holds. Two accounts share neither, so a list never holds two that cannot be told
  apart.
- **A credential is reached one of three ways, each a kind of account that answers for itself:**
  - **A directory account** names an absolute configuration directory, whose login the CLI keeps in
    that directory's own credentials file.
  - **A token account** names a file holding the long-lived token the CLI issues, which is handed to a
    run in the environment. That token lasts a year, carries no refresh of its own, and is
    inference-only.
  - **An endpoint account** is Claude Code run against another provider's Anthropic-compatible
    endpoint: it names a configuration home of its own, which holds no login and keeps its sessions and
    transcripts, the endpoint's base URL, and a file holding the endpoint's API key. It may also map the
    family words `opus`, `sonnet` and `haiku` to the endpoint's own model ids. An endpoint account is
    paid by the request rather than by a subscription window, so it has no usage windows, and its key
    neither expires nor renews.
- **What is kept is where a credential is, never a credential.** No token, no key and no file body
  enters the store, a log line, a document or a report; the reader that opens a credential file keeps
  only the moment it expires, and an endpoint account's key file is read only as a run is launched.
- **The account is handed over in the environment, with one exception in a flag.** Every line's flags,
  its system prompt and the tools it is shown are what they were, so a run moved to another account
  stands on the line it always stood on and only the cache it is warm in changes.
  - A directory account is handed its configuration home, and any token the launching process carried
    goes with it, since a token left in place would answer for the run instead of the login the owner
    chose.
  - A token account is handed its token alone.
  - An endpoint account is handed its configuration home, its base URL, its key as the run's
    authorisation token, and the model ids its map names for each family; every other credential the
    launching process carried goes with it.
  - **The exception:** an account may name a base prompt (`docs/base-prompts.md`), and a run on the worker
    line on that account opens its system prompt on that base in place of the platform's harness opening, so
    on the worker line the file the system-prompt flag names is the account's (`docs/launch-lines.md`). An
    endpoint account names the base its endpoint's own harness runs on, since that is the opening the
    endpoint's model runs best on.
- **An account carries its own everything.** Its sessions, transcripts, usage windows where it has
  them, spend and rest belong to it, because a login is what they belong to: a session opened under one
  account cannot be resumed under another, one account's windows say nothing about another's, and each
  account's cache is warmed on its own.
- **An order holds the providers; the Claude Code entry holds the accounts.** The account order is one
  list for the deployment, beside every supervisor's and role's provider order, and it is what the Claude
  Code entry of any order walks when its turn comes.
- **The account order is each account's own place and on/off, held together,** so the two cannot
  disagree and a turned-off account keeps its place. An order is refused unless it holds every
  registered account once each with at least one on.
- **With nothing said, the order is the order the accounts were added in, all on** — what a fresh
  installation has and what the owner resets it to. There is no stored default to remove, since the
  accounts are the owner's own list from the start.
- **A fresh installation has one account,** standing for the login the machine already holds, so
  nothing answers differently until a second is added.
- **Outside the console,** an account named by its id or its given name:
  - `bmo-agency agent --account-add <name> --path <dir|file> [--kind directory|token|endpoint]` adds an
    account at the end of the order, on; an endpoint account also takes `--endpoint <url>`,
    `--key-file <path>`, `--models opus=<id>,sonnet=<id>,haiku=<id>` and `--base-prompt <name>`, its
    path being its configuration home
  - `bmo-agency agent --account-set <account>` changes the same fields, `--base-prompt none` letting
    the account's base go, and `--account-remove <account>` removes an account that is not the last
  - `bmo-agency agent --account-order <account,…>` sets the order
  - `bmo-agency agent --account-on <account>` and `--account-off <account>` change on/off
  - `bmo-agency agent --account-reset` puts the order back to the default
  - with no argument the account order shows beside the providers' switches, marked where it is still
    the default

### The login in use is used to the end

- **One login is in use, for the whole road.** Every run on Claude Code — a supervisor's turn, a role's
  run, a session opened after a restart — starts its account walk on that login for as long as it can
  answer, behind the chosen account while that one can answer, and
  ahead of the order.
- **A login coming back moves nothing, except the chosen one.** An earlier account answering again is no
  reason to leave the login in use; the road moves only when that login stops answering, when the owner
  chooses an account, or when the chosen account can answer again (「Running on one account now」). A
  session and its warm cache belong to the login that opened them, and the owner spends one account's
  window before the next, so moving off a login that still answers throws the sessions away for nothing.
- **What is kept:** one setting naming the login in use, so a restart or a deployment starts every run
  where the road stood. It is written by the first run that answers while no login is in use or while
  the one in use cannot answer, and by the owner's choice.
- **A login in use that cannot answer is walked past like any other** — off, removed, at its limit or
  waiting for a person — and the walk goes on from the chosen account and then the order from the top;
  the login that answers there becomes the one in use.

### Running on one account now

- **The owner can name the account the road runs on now,** from a row on the console or with the command
  below. That account is the chosen one, and choosing it is a switch: it becomes the login in use at once,
  so the next run of every supervisor and role on Claude Code goes out on it. The chosen account heads every
  account walk whenever it can answer; while it cannot, the login in use answers where it can, then every
  other account that is on, from the top.
- **The choice leaves the order alone.** It is one setting beside the accounts' rows, naming one account
  by its id; neither the order nor any switch is rewritten to express it. With nothing chosen the walk
  is the account order as it stands.
- **Only an account that is on can be chosen.** Choosing one that is off is refused; turning the chosen
  account off, removing it or resetting the order lets the choice go with it.
- **A chosen account that cannot answer is walked past like any other:** it is not the account serving
  now — the first open account is — and the account that answers becomes the login in use. The choice
  stays, and once the chosen account can answer again — its windows reopened, its login signed in again
  — the next run goes out on it; a run already going finishes where it went out.
- **What moves, and when.** The choice changes the head of the walk and nothing else, so everything that
  already happens when the head moves happens at the next run, not at the click: a run already going
  finishes on the login it went out on; the next chat round asks the receiver for the old session's
  handoff, then closes it and opens a fresh one on the chosen login (`docs/sessions.md`, 「A restart
  hands the session over」); the next role run goes out on the chosen account, resuming the role's
  session kept under that login where one is valid, or opening one (`docs/sessions.md`, 「A role's
  session」). A turn on a session already held stays on that session's own login, since no other login can
  reach it.
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
  while the login is inside the last half hour of its life. A waiting run waits for its event, not for a
  span: the credential file showing the renewal written, the holder letting the lock go, or the holder's
  process ending, whichever comes first.
- **A long-lived token removes the renewal:** an account pointed at one needs no browser for a year, so
  none of the above happens for it; it is warned about before that year is out like any other account.
- **An endpoint account has no login to end.** Its key states no ending and is renewed by nobody, so it
  takes no renewal lock, is never warned about, and is marked as waiting for a person only when the
  endpoint refuses its key or its balance (「Automatic walks only on providers, in order」).
- **An account's token for the work nodes is a login of its own** (`docs/work-nodes.md`, 「The logins on
  a node」), counted and warned about separately; a node login found gone is marked for the nodes alone,
  and the account's login here is neither marked nor passed over for it.
- **A login that has ended is marked, and the mark is the walk's instruction:** the account's row says
  it waits for a person, the walk passes it over, and its work moves to the next account and then the
  next provider.
- **The mark is lifted by the credential, not by anybody's word.** Signing in again writes a moment in
  the future, and the walk's own reading of that credential takes the mark off, so no console action is
  needed to put the account back in the order. An endpoint account's mark is lifted by its key file
  being written after the mark, or by the owner changing the account, since a balance topped up at the
  endpoint leaves nothing on this side to read.
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
- **The watch runs on the keepalive pass** (`docs/keepalive-schedule.md`), so the marking, lifting and
  warning happen without anything being launched for them. It watches directory and token accounts; an
  endpoint account has nothing for it to read. Its wording, how long before the end it speaks and the year a
  token stands for are read from agency's policy (`policy/`).

### Availability: whether a provider can answer now

- **An order sets only which providers and in what order, and the owner's switch only on and off.** Whether
  a provider that is on can answer now, each provider answers for itself, out of its own member. It cannot
  answer for one of these reasons, and can otherwise:
  - **no credential on this machine** — the presence its member reads (`docs/provider-presence.md`):
    Codex's executable, OpenRouter's key, the gateway's key file and an offered registration — one with
    an address or one naming a card service, whose engine standing or stopped at that moment is no part of
    its presence (`docs/own-llm.md`, 「An engine the arbiter stands」) — DeepSeek's key file; Claude Code's
    credentials are its logins, below;
  - **its balance spent** — where it offers a balance, the one its reader last kept for its own road is
    at or below zero (`docs/usage-windows.md`, 「Balances」); a balance never read closes nothing;
  - **no login open** — where it holds logins, none of its logins is open by the account judgement
    below, each account's own reason beside it.
- **A provider whose program does not carry a row's line is never in that row's order**
  (`docs/provider-order.md`, 「What an order is」), so availability is asked only of providers that carry
  it. Whether a provider has quota is asked beside availability, never in place of it
  (`docs/usage-windows.md`, 「What reads a provider's quota」).
- **A provider that cannot answer now is passed over** in its turn with its reason, and so is an account
  the judgement closes.

### The provider order's choice

- **One place decides where a run goes:** the provider order's choice (`docs/provider-order.md`,
  「Choosing the provider」). It is asked for a supervisor's turn or a role's run as the run starts, and
  answers where that run goes out now — the provider, the model, the effort and the thinking its order's
  entry names there and, where the provider holds logins, the account — together with every provider it
  passed ahead of that one and why each could not take it: no quota above the share, or unable to answer
  now (「Availability: whether a provider can answer now」).
- **The account.** Where the provider holds logins, the first login the account judgement offers
  (「The account judgement」), the chosen account and the login in use heading the walk.
- **Runs that do not walk their own role's order** — a repository's pinned model, a run of a role whose order
  is empty on the road of the supervisor that asks it (the `doppelganger` role as seeded), a session whose caller names its
  provider and model — are `docs/provider-order.md`'s, 「Runs that do not walk their own role's order」; the
  account walk inside the Claude Code entry is the same for each of them.
- **Every line asks it, and none keeps a walk of its own:** a supervisor's turn, a job's launch and its
  relaunch, every other role's run, and the warming, which warms no provider that cannot answer now. The
  choice knows no provider by name: every provider's answer is its own member's, and the choice only walks.
- **Return comes with the next start, never mid-run.** The choice is made afresh as each run starts, so a
  provider that has quota and can answer again carries the next one; a run already going finishes where
  it went out and is never cut to move it back.
- **A move is a fact, told once,** as `road-moved`, `road-returned` and `road-closed` for the supervisor or
  the role whose runs moved (`docs/provider-order.md`, 「The facts told」); where the provider holds logins,
  each provider named carries its account.
- **When nothing answers, the run waits; it is never refused.** It is held with every provider's reason
  and goes out once a provider of its order has quota and can answer again (`docs/provider-order.md`, 「When
  every provider is out」). The sentence names what a person must do for the first provider a person can
  reopen: an account waiting for a person, the sign-in command with that account's own configuration
  directory in front of it, as the login notice words it; a spent balance, where its provider's member
  says it is topped up. A provider that only waits for a window names the moment it reopens.

### The account judgement

- **One judgement says whether a login answers now, and every Claude run is offered a login by it** —
  every supervisor's turn, fold and warm ping, and every role's run, search and browse among them. No line
  keeps a judgement of its own, and none falls back to the machine's own login in place of one.
- **A login is open while all three hold:** it is on; no window of its own is full (neither a limit rest
  standing under it, nor a reading of its own windows showing one at its ceiling that has not reset —
  `docs/usage-windows.md`); and its login has not gone (「A login that is about to end」). A login that
  waits for a person is closed.
- **An endpoint account is open while it is on, its key file is present and not empty, no rest stands
  under it, and the balance last read for it is above zero.** It has no windows to read, so only a rest
  written from a refusal or a spent balance closes it, and only a refusal of its key or its balance marks
  it as waiting for a person; a balance never read closes nothing.
- **A window read full closes its login** whether or not any run has met the refusal yet. The windows are
  read on a clock from the account's own login, so a login spent elsewhere is closed by the reading, and
  a rest removed by hand does not reopen a login whose window is still full.
- **A full window that names its reset closes its login until that reset, however old its reading,**
  since nothing but a newer reading can say it came back early: a newer reading that finds it below its
  ceiling opens the login at once, and the reset opens it on time. A login spent until a day later
  therefore stays out of every walk until then, even where its token has lapsed and the reader can no
  longer ask for it.
- **A full window that names no reset closes its login only while its reading is fresh,** a bounded
  window, since a quota can come back and nothing tells this side but the next reading; past that it
  closes nothing, so the walk never leaves a login shut forever on a reading no one can refresh, and the
  next run settles it — one that answers renews its own token and takes a fresh reading, one still spent
  meets the provider's refusal, which rests it as any refusal does.
- **A rest written from a run's refusal or its own usage is not a reading:** it closes the login until
  its moment whatever the reader later finds, so a login a run found spent is never sent another run
  before the refusal's own moment. A done job's usage is handed to agency by the job's runner, which
  rests the login where a window stands at its threshold (`docs/api.md`, 「Accounts」).
- **A closed login is never offered.** The walk offers the open logins in its order and none other.
- **A run none of whose logins answers goes out on none.** It ends without launching anything, and
  whatever receives that failure reads it as it reads any limit or lost login, beginning no rest, since
  the run went out on no login.

### Automatic walks only on providers, in order

- **The Claude turn:** if Claude Code is on and its turn is reached, the chosen account answers where it
  can, then the login in use (「The login in use is used to the end」); otherwise the account order is
  walked from the top and the first open account answers. The Claude Code turn is closed only when every
  account is.
- **A run that fails on the account rather than on the work** — its limit reached, its login gone — moves
  to the next open account before the walk leaves the Claude Code turn, and the work that asked never
  learns the first account was tried. A failure of the work itself moves nothing, and neither does the
  service being down, since no other account would answer that either. A login passed over for its limit
  is put to rest as it is passed, under that login, so nothing that reads the failure afterwards writes a
  rest under another. Where that rest closes the last open login, agency publishes it once as
  `deployment-resting` (`docs/api.md`, 「Events」), and the consumer holding the owner's chat says it.
- **An endpoint account is walked like any other, and its refusals are read as account failures.** The
  endpoint answers in HTTP status codes rather than in a subscription's words:
  - a 401, the key refused, is its login gone, so the account is marked as waiting for a person;
  - a 402, the balance spent, is marked the same way, since only the owner topping the balance up
    opens it again;
  - a 429, too many requests at once, is a limit, so the account rests for a short while;
  - in each case the work moves to the next open account, as from any login.
- **A login gone, or its token for the work nodes, is told the way the login watch tells it:** marked as
  waiting for a person and published once as `account-login-notice`. Agency says nothing in a room
  itself.
- **A supervisor's program left on a closed login is taken down.** The loop in a supervisor's pane starts
  the program its place holds again on the login written into it, so the login watch takes down every
  supervisor's program whose login the judgement closes, unless a turn is in it; the supervisor's next turn
  stands its program again on the login the walk offers then. A program that names no login of its own is
  the machine's and is left alone. A role's run keeps no program between runs, so nothing of a role is
  left standing on a closed login.
- **Which of the two a failure is, is one judgement,** read off the run's ending through the program's
  own failure reading, in one place every run asks, so no two lines read the same failure differently.
- **A line that spawns once takes the first open account rather than walking.** Search and browse runs
  are each one process given the account their run is offered first, and a pieces-shaped turn on Claude
  Code takes the first open account the same way. Everything that can be launched again is carried
  account by account.
- **Walking down:** if a run's provider has no quota above its share, cannot answer now, is not run or
  ends with no answer, the run goes to the next provider of its own order (`docs/provider-order.md`,
  「Falling over and coming back」). With Claude Code off, a run whose order begins with it starts from the
  next provider of the order that can take it, and where none can, the run waits; it never goes out on a
  provider its order does not name.
- **Who receives that failure, each in its own place:**
  - **A supervisor's turn** goes on to the next provider of the supervisor's order; where none can take
    it, the turn waits and its room is told by the receiver (`docs/supervisors.md`, 「Where a supervisor's
    turn runs」). A failure that is the work's own — the provider refusing the request for its shape —
    ends the turn failed rather than moving it. A turn stopped from outside goes on to no further
    provider: the interrupt standing over its run ends the walk wherever it stands, a wait for a card
    included, and the turn ends with the stop.
  - **A job moves to the next provider rather than failing.** A job whose turn fails for its provider's
    availability — the provider refusing its quota, its window, its balance or its credential, its last
    open login closing under it, or the provider it went out on found unable to answer as the turn ends —
    is launched again on the next provider of its role's order: the same job, item and lease, in the same
    checkout, at most once on each provider in one job, its prompt opened with a line saying it continues
    work already in the checkout. The job keeps each provider it went out on and why it moved, and its
    report says so. Where no provider of the order can take it, the job waits, held with the reason, and
    goes out again once one can; it is never failed for that. A job in a repository that pins a model
    never moves: it waits while the pin cannot answer (`docs/repositories.md`, 「A pinned model」). A
    failure of the work itself moves nothing and ends the job as it stands.
  - **A blocked road stays passed over for a while.** A road a supervisor's turn or a job walks off for its
    provider's failure is blocked by that walk, and the harness blocks one through the boundary
    (`docs/api.md`, 「road-blocks and launch-room」); either is passed over by the next runs whose order
    reaches it while a later road of the order can answer (`docs/provider-order.md`, 「Falling over and
    coming back」) — a minute for an outage, five for a refused access, fifteen or until its reset for a
    limit; a block is the walk's memory of a failure, not one of the reasons a provider cannot answer now.
    On a provider whose models are services of their own, that road is the one registration that did not
    answer, and one registration going quiet leaves the others open (`docs/own-llm.md`).
    - **A turn stopped from outside closes no road:** a turn that ends without an answer while its job has
      been stopped is the stop, on every road.
    - **A request the road refused closes no road:** a registered model's turn that ends on a 400, 413 or
      422 — a conversation grown past the registration's window among them — is the backend answering the
      request, not the road gone (`docs/own-llm.md`, 「A turn at the input limit」).
    - **A registration closed for an outage is asked again before it is passed over:** a run's walk asks
      the engine behind the registration for its health, and an engine that answers opens the road again
      at once, while a registration whose engine gives no health waits out its minute.
      A registration whose engine the arbiter stands (`docs/own-llm.md`, 「An engine the arbiter stands」)
      is asked through the arbiter's master instead: a stopped engine is no outage, and the road stays
      open while its card service is placed on a node no claim and no lower tier holds.
  - **A gate** is no run of agency's and is in no walk here (`docs/own-llm.md`, 「Gates are not agency's
    runs」).
- **Re-checked just before running:** on/off is checked again just before a run starts. A provider turned
  off while waiting for a lock is not launched anew, a request already running is left to finish, and
  Claude's run line is unchanged. While Claude is off, folding a session on Claude Code is deferred and
  session records preserved.
- **A co-work order's stages** each go out on their role's order like any role's run, so a stage whose
  role's first provider is off goes out on the next provider of that role's order, and the next stage's
  reservation holds slots of its own role (`docs/role-slots.md`, 「Reserving slots」).

### How a supervisor's turn and a role's run cross over

- **A supervisor's turn** carries the request its caller assembled with recent conversation, and its tool
  presence and write rights, as a common run contract, on whichever provider of its order takes it.
- **A role's run** carries its whole prompt — the platform's harness opening, the role's prompt and what
  the work hands in — its path, the tools and MCP configuration its line shows, and, in a repository's
  checkout, the repository's instructions and rules (`docs/repositories.md`, 「What a run in a repository
  is handed」). The rules go where each program keeps them beside its own default prompt
  (`docs/launch-lines.md`).
  - **A job's turn crosses over on its role's order.** The launch asks the provider order's choice as the
    job's work parts are read (`docs/api.md`, 「assignments and jobs」), so while the first provider of
    the role's order has no quota or cannot answer, the job's turn is composed on the first provider of
    the order that can take it and carries the same line, and the next job goes back to the top once it
    can. Whichever provider it goes out on, the turn goes on in the role's session kept at that path on
    that provider and model where one is valid (`docs/sessions.md`, 「A role's session」).
- **A search or a browse run walks its role's order** like any role's run, each provider of the order one
  whose program carries its line (`docs/agent-roles.md`, 「The voyager and the seeker roles」).

### A turned-off provider is not used

- **What is skipped:** no run goes out on a turned-off provider — a supervisor's turn, a role's run, a job
  in a repository that pins one of its models, a session whose caller names it — and its sessions are not
  warmed, since turning off is the owner saying not to use that provider and no row's order overrides
  that. A turned-off provider is one that cannot answer now, so a run goes on to the next provider of its
  order, and a job on a pin waits (`docs/repositories.md`, 「A pinned model」).
- **A turned-off account is passed over the same way:** the Claude Code entry does not walk it and its
  sessions are not warmed.
- **No per-model off:** an order names one model on each provider, so turning off a provider takes it out
  of every order's walk, and changing a row's model on it is done on that row's order.

### Warming only enabled providers

- **A supervisor is warmed,** on the providers its row switches warming on for, **and the kept session of an
  agent whose row keeps its cache warm** (`docs/keepalive-schedule.md`, 「Whose prefix is warmed」). No switch
  stops every warming at once, and nothing of a repository or a place is warmed; the kept session of any
  other agent is resumed while its cache is still warm and opened afresh once it has gone
  (`docs/sessions.md`, 「A role's session」).
- **Claude off:** while Claude Code is off, no Claude keepalive goes out. **One account off or resting:**
  its own sessions are not warmed and its prefixes are left to cool, while every other account's are
  warmed, since a prefix belongs to the account it was built under. **An endpoint account:** its
  sessions are never warmed, whether it is on or off. The endpoint ignores the cache markers a ping
  would renew and keeps its own cache by itself, so a ping would only be a paid request. **Codex off /
  OpenRouter off / DeepSeek off:** that provider's sessions are not warmed. **A provider on that cannot
  answer now** — its credential gone, its balance spent, every login it holds closed (「Availability:
  whether a provider can answer now」) — is not warmed either, since no run goes out on it until it can.
  An unwarmed session's record is left in place, and re-enabling a provider judges the next warming from
  the time that record names.

### The Codex road is a member of the gateway

- **How it runs:** Claude Code on agency's gateway under the member named for the Codex model, on an OpenAI
  API key kept in the member's key file under the data root's `env/`. The Codex CLI remains in the build for a
  rollback, and no provider of this build names it.
- **The models:** the members the deployment names for the Codex road, each with its window, answer ceiling,
  thinking default and request wait. A member road serves no subscription window, and no run of it is warmed.

### The OpenRouter road is a member of the gateway

- **How it runs:** Claude Code on agency's gateway under the member named for the OpenRouter model, with the
  key its key file holds. The public listing is still read for the chooser that declares a member.
- **The models:** the members the deployment names for the OpenRouter road, each with its window, answer
  ceiling, thinking default and request wait. The road publishes no cache lifetime, and no run of it is
  warmed.

### The registered models are a provider of an order like any other

- **Only when:** the registry's provider stands only when the gateway's key file is present, and answers
  with the registration the order's entry names, or a repository's pin.
- **How:** a tool-less run is one request to the gateway; a run with tools is Claude Code on the gateway
  (`docs/own-llm.md`). Whether thinking is on and the ceiling of answer tokens are named on the order's
  entry for the registry (`docs/provider-order.md`, 「What an order is」).

### The DeepSeek road is a member of the gateway

- **A provider of its own.** DeepSeek's models are served by the `deepseek` provider rather than by the
  registry, and so take their own entry in an order, with its own quota read from its balance
  (`docs/usage-windows.md`, 「What reads a provider's quota」). The members the deployment names for the road
  carry its key file, `env/deepseek.key`, and its window, answer ceiling, thinking default and request wait.
- **How a run goes out.** Claude Code on the gateway under the member the run names: a supervisor or a
  role's run stands it in its pane, its turns put into the program and read back out of its transcript.
  The session is kept and resumed like any Claude Code session, and what a turn spent is reported off the
  transcript.
- **Thinking and effort.** The provider's words are `none`, `low`, `medium`, `high` and `max`. `none` is a run
  without thinking, and so is an order's entry with thinking turned off. The word goes out in the effort
  header, which the gateway writes into the request its backend reads; the Claude Code effort word a run is
  launched with is the provider's own mapping of it.
- **Which model a row runs on there** is its order's entry for DeepSeek, a member of the road at the
  effort it names; the seeded rows' entries are the roles policy's, and the owner changes them on the
  row's order (`docs/agent-roles.md`, 「The roles the policy seeds」).
- **What the provider is not.** Its models are not registrations, but its runs reach them through the model
  gateway; nothing on this road is placed on a work node.

### The models an order's entries name

- **Not a fixed launch line:** the console reads each provider's models (`docs/provider-order.md`,
  「Models a provider holds」) and each model's supported efforts, and lets an entry's model and effort be
  changed per supervisor and per role. So changing one row's entry moves no other row's.
- **The grounds for a seeded entry:** a seeded row's entries on the providers after its first are the
  closest same-method match for its first entry's model, read from an independent benchmark the console
  cites, never a claim of exact equivalence. A single selector to set every row's order at once is not
  offered; a row's preferred provider is set by putting that provider first in its own order.

### Keeping the mode across cache, session and usage

- **The roads answer differently and do not pretend to be the same.** Every common answer carries the
  mode that actually answered (`claude-code`, `codex`, `openrouter`, `registered`, `deepseek`) with its
  model and effort, and a measured usage row keeps mode and model as a pair, so models of the same name on
  different roads are not merged. Agency reads those figures off each road — Codex's JSON events,
  OpenRouter's stream usage read through a loopback tab on a headless run, Claude's own answer — and
  hands them to the usage collector, which splits and shows them (`usage.md`). A run whose stream
  carried no usage gets no invented figures, and a turn in bmo-tui's dsh session reports none: its record
  carries the answer's words and model alone.
- **Each window and run belongs to a slot:** the provider that reported it, and the account too where
  that provider holds logins, built by one constructor so no writer decides for itself whether an account
  belongs on a row. Two accounts report windows of the same names, so neither can overwrite the other.
- **Which provider and account serve a row now is answered in one place** — the provider its last run
  went out on, kept beside the supervisor or the role (`docs/provider-order.md`, 「The facts told」), and,
  where that provider holds logins, the login the provider walks to now. A row answering on its own first
  provider is the road it chose, so it never moves what every other reader takes for the agent serving
  now.
- **Sessions follow the same principle:** Claude Code sessions and Codex threads are shown with their own
  mode and model, a Claude session with the account it was opened under. A dsh session stands in
  bmo-tui's pane across its turns, but agency records no id for it, so an OpenRouter answer offers no
  session back. A session is offered back only to a run on the account that opened it.
- **Cache lifetime and keepalive follow the provider's policy.** A ping goes only into a supervisor's
  program (`docs/keepalive-schedule.md`); a role's session is judged by the same lifetime when it is
  resumed (`docs/sessions.md`, 「The warm/cold criterion」).
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
    registration gets no ping, and a supervisor standing on the registry's provider warms no session of
    another provider.
- **A role's kept session follows the same record lifetime,** so a session idle longer than that is not
  resumed but opened anew. Every ping obeys the supervisor's switch for the provider it stands on, skips a
  supervisor standing on an own LLM service, and, if a real turn is running in the same scope, skips that
  round without waiting.

### While a fallback stands, the room is not silent

- **A rest is one account's,** so a room falls quiet only when no account and no provider will answer it.
  Even where every account rests, if the supervisor's order holds a provider other than Claude Code that
  has quota and can answer now, the supervisor keeps taking the room (`GET /stand-in`); a login is
  closed for it exactly as the account judgement closes it (「The account judgement」).
- **Which model it switched to is operational information,** so it is not told to other rooms. The owner
  is not told before the first answer either; only when a provider below the first of the supervisor's
  order first answers, or later drops to another, does agency mark the road answering with the moment
  the fallback began and publish `fallback-answering` with the owner, the substitute that answered before
  where a fallback already stood, and the provider and model answering now (`docs/api.md`, 「Events」).
  Telling the owner's private chat, and in what words, is the consumer's. A supervisor answering on the
  first provider of its order is on the road it chose, not a fallback, so it is neither published nor
  marked. When the first provider of its order answers again with a real session id, the fallback mark
  is cleared and `fallback-ended` is published with the stand-in that answered last; telling the owner it
  is back is the consumer's.
- **How a supervisor's turn goes out on a stand-in.** A turn takes the supervisor's own order
  (`docs/provider-order.md`, 「Choosing the provider」): each provider of it that has quota and can answer
  now, in order, on the model and effort its entry names there, until one answers; where the walk reaches
  Claude Code, the turn goes out on Claude Code's own session. A stand-in stands in the supervisor's own
  pane as that provider's program, on the chat line's system prompt, tools and MCP configuration:
  - **Codex** on the supervisor's kept thread for that model and effort, opened afresh where Codex no longer
    holds it, and the thread that answered kept for the next turn;
  - **OpenRouter** through bmo-tui, which keeps one dsh session on the Agent Client Protocol, the turn put
    into its inbox and its answer read out of its record; the overlay composes the gateway route, the key
    rides in the program's environment, and the agency's MCP configuration stands in the overlay as the
    session's servers, rows the overlay inserts; a server that does not attach is told to the session as
    a fact it reads in its next round, and the shell is handed back the variables dsh's own start-up
    drops by their names (`docs/agent-control.md`, 「dsh's providers: OpenRouter and DeepSeek」); the
    system prompt's tools paragraph names dsh's own tools; no upstream is pinned, since agency keeps no
    record of a warmed one;
  - **DeepSeek** through bmo-tui in the same way, the overlay's route DeepSeek's own API on the model the
    entry names, with the road's key in the program's environment (「The DeepSeek road is a member of the
    gateway」);
  - **a registered model** as Claude Code on the gateway, the registration first held to the gateway's
    list, on a session of its own placed where the registration places it.

  A stand-in that does not answer passes the turn to the next provider of the order. A turn whose words
  never became a turn — a pane left in a dialog somebody opened, the words unsubmitted — is not a road
  that did not answer: the walk ends there, nothing is read as closed, and the session the turn would have
  gone on is left as it stands. Where no provider of the order answered, the road is closed and told once
  (`road-closed`), and the turn waits, never refused, until a provider of the order has quota and can
  answer again (`docs/provider-order.md`, 「When every provider is out」). The session the supervisor's row
  keeps across rooms on Claude Code is Claude Code's, and a stand-in's turn leaves it as it stands.

### The response options each road receives

Each response option the Claude Code line receives is set out below, saying whether a substitute road has
the same. "None" means that program has no such concept, with the reason.

| Option | Claude Code | Codex | OpenRouter (dsh) | Registered models |
|---|---|---|---|---|
| Process time limit | None: a supervisor's turn and a role's run each run until they end | Same | Same | None: a request with no tools waits for its answer; a run with tools is Claude Code on a registered model |
| Cache lifetime · ping interval | 1 hour · 40 minutes | 30 minutes · 20 minutes for models that announce it, else none | Follows the answering upstream, none if not listed | None. No announced lifetime |
| Ping time limit | None: a ping goes until it answers or fails on its own | Same | Same | No ping |
| Keepalive switch · rest | Warms a supervisor only where its switch for the provider is on, pauses during its own account's rest | Obeys the switch but warms during a rest, since a rest is one Claude account's affair | As Codex | Not applicable |
| Session continuation | Resumes a supervisor's session and a role's kept session | Resumes a thread per run scope, model and effort | The session stands in bmo-tui's pane and carries the next turn; agency stores no id and resumes none | None for a tool-less request; a run with tools is Claude Code's |
| Context folding | The caller folds with the CLI's compaction | The Codex CLI compacts its own thread | dsh compacts its own session; agency folds nothing | Not applicable |
| effort · thinking | The effort set per line | Passes effort, thinking-off is low | The entry's level is the session's reasoning effort, and thinking off passes off | Thinking on or off; a run with tools sends medium or low |
| System prompt | Replaces the CLI instruction with one whole file opening on the platform's harness, worker its rules after it | Replaces via `model_instructions_file`; worker the Codex default and `AGENTS.md`, rules as `developer_instructions` | Goes in as one part of the dsh prompt, no replacement | A request with no tools carries its system prompt in the request; a run with tools as on Claude Code |
| Tool · permission isolation | A supervisor's and a role's program in bypass mode with its line's denied tools, a runner's shown tools and an allow list, settings sources blocked but a role's checkout's own | Moves to a shell and sandbox, loads no user settings | Moves to filesystem/shell rows and the dsh sandbox, no per-command allow list | A run with tools is Claude Code's line |
| Output schema | None. A structured answer is asked in the prompt and validated by the caller | None, for the same reason: a schema option moves the cached prefix | None | None |
