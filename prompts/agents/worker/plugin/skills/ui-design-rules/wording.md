# Wording

## How this document stands

- This is the one place that says how the chat agent and the workers spell words in Korean text, and the one
  list of purified words that must not be used.
- The banned-term list and the allowed uses below are read by the wording check, a test in the
  repository suite (`repo/checks/wording.test.ts`) that also runs among the quick checks ahead
  of it (`bmo check quick`); a line added here is enforced from the next run.
- A term the owner points out as over-purified is added here as one more line, together with any use
  already in the tree that the check then reports.
- `bmo check wording` hands this list to a program outside the bot: `table` prints the banned
  terms, `scan` names the terms each text on its input uses, matched as the check matches them, and
  `add` writes new banned lines at the end of the list.
- The same list is read again where words are written rather than committed, as
  「Where it is held as words are written」 says.
- A chat turn is given this document by the system-documents gate (`docs/reference-index.md`); a
  worker reads this list through `AGENTS.md` and the principle in its own trailer, and the
  `ui-design-rules` skill carries it as `wording.md`
  beside itself.

## The principle

- A Korean sentence uses the word people most commonly say for the thing, whatever its origin: a
  Sino-Korean word, a loanword in its usual Hangul spelling, or a native word, whichever is the one
  in everyday use.
  - Right: location, question, word, reason, toolbar, built-in, edit, revise.
  - Wrong: a native word or coinage put in place of the common one — "place" for location, "asking" for
    question, "term" for word, "cause" for reason, "tool strip" for toolbar, "fixed" for built-in, "fiddling" for edit.
- A native word is written where it is the one people say — open the file, break the line — and never
  because it is native. Plain writing (`prompts/rules/plain-writing.md`) is about how sentences
  read; it is not a preference for native words.
- The English documents a writer reads are written in English idiom, not in terms: a thing
  "stands" somewhere, holds "a place", is reached by "a road", leaves "a strand". A Korean text
  renders the meaning in the common Korean word, never the English word's native Korean twin.
  - A place is location, a queue's order or a pool's slot; to stand somewhere is to be or to be shown; a
    road is path or method; a strand is one entry of a list.
- English is not dropped into the middle of a Korean sentence.
  - Right: "press the toolbar". Wrong: "press toolbar".
- English stays English only where no Hangul spelling is in common use:
  - Code identifiers, commands, file and path names — `max_tokens`, `bun run test`, `WORDING.md`.
  - Names of products, libraries and protocols — Nuxt UI, SSE, herdr.
- **A job is Job or work in Korean text, never a transliteration of Job.** This is the owner's own choice of the word, and it
  holds wherever the chat agent or a worker writes: an answer, a job's report, a notice, a console label, a
  document's Korean example.
  - Job where the text names this system's unit of work — Job #123, the Job worker, the Job report, the next
    Job, the running Job — and wherever work would read as the TODO item or as a work order.
    A particle follows it as it followed the transliteration: Job-i, Job-eul, Job-eun, Job-euro.
  - work where the text means the work itself rather than the unit that runs it.
  - Spoken, Job is said as job: the voice name table reads it so in a briefing (`docs/voice-plan.md`,
    「How names are said」), so a line written with Job is heard the way the owner says it.
- The list below names the swaps already caught; a word missing from it is held to the same
  principle.

## Console text

- The web console, its language pack and every message a control route hands it to show are
  written by this glossary as well as by the principle above.
- Each line is `<English concept> → <the Korean UI word>`, with the forms it takes and the forms it
  replaces. The word stands in every inflection: where the glossary says delete, a past tense is
  deleted and a failure could not delete.
- delete → delete: delete, deleted, could not delete, deleted, delete it?. Never erase, erasing, erased,
  erased, erasing; taking one entry out of a list is delete or remove, never subtract.
- retire, take down → archive; bringing one back → restore. Switching a piece in or out of use → on, off.
  Never take down, bring up.
- fold (a work order) → end. Folding is only ever a panel folding away, and is written as below.
- file, stand up (an item) → create: create item, created item, could not create. Never stand up, bring up.
- ask (a job's question) → question: the Job's question. Never query, asking.
- machine → device: this device, device headroom. Never machine.
- render → rendering: the period without rendering. Never draw.
- message, words → message: send a message, the message to send. A conversation is a conversation, an instruction added
  to an item is an instruction. Never talk, words to say, story, strike up a talk.
- filter, sift → filter. Never sifting.
- page → page. Never leaf.
- table → table; row → row; query → query, wherever the text is about a database. Never grid, line, inquiry.
- default → default. Never first value.
- section → section. Never area.
- health check → health check; the one run before anything changes → pre-change health check. Never survival check.
- rollback → rollback. Never undo.
- template → template; coverage → coverage; cap → cap; cooldown → cooldown. Never frame, covering, ceiling, rest.
- output tokens → output; context window → context. Never answer, window.
- collapse, expand a panel → close, open; collapse, expand in place → shrink, expand. Never fold, spread, unfold.
- reach a limit → reach; a match → match; tied to → connected. Never caught, caught place.
- reach a service → connected: not connected to the bot. Never touch.
- request → request. Never ask.
- choose, pick → choose: please choose, undo the choice. Never picking, please pick, undo.
- enter text → enter: please enter, enter within {n} characters. Never please write.
- launch, start a run → run. Never float, rise, fire.
- stop a run → stop; pause and resume a timer → pause, resume; run out of time → time out. Never stopping,
  flowing again, missing the moment.
- leave the queue → remove from the queue. Never fall out of the queue.
- set by hand → set by hand. Never set by hand literally.
- **One screen speaks in one style.** A notice, a guidance line and an error end in the same polite
  form throughout; a casual form is never mixed into them.
- **No dash between words.** A range is written `{min}~{max}`; a caveat on a title stands on its
  own smaller line, or after a ` · `.

## Banned terms

- Each line is `<language code> · <banned term> → <what to write>`, optionally followed by
  ` — <when it applies>`.
- The language code is the language of the text the entry governs (`ko`, `ja`, `en`); an entry is
  checked only against text written in that language.
- The banned term is matched as written, spaces included, so a spacing variant is a line of its
  own.
- A Korean term is matched only where it stands at the head of its word: a syllable running into
  it from the left makes it part of another word, so rewind is not wind and limit is not it.
  What follows it is not asked about, since a particle or an ending there leaves the word its own.
- One line per ending in use, rather than a stem standing for a family, so that the line can say
  what to write in that ending's place.
- running is not listed: return and send back are their own verbs and share the spelling. Where it
  means running something, write run — left running, run twice, run just one line.
- money and return are not listed either: money is money and come back and go back are their own verbs. Where
  either means running something, write run — the Job that ran, a run that never started.
- tree alone is not listed: Namuwiki and tree house are names. Where it stands for a tree of files, folders
  or nodes, write tree, and each such phrase is a line below.
- node is listed alone, since one word is its own word and the word-head rule leaves it be; it is
  written without a space.
- place is listed alone and banned where it stands for a position, a slot or a part of a text or of
  code; the phrases in which it keeps its own sense, a seat or a digit, are listed under
  「Uses in the word's own sense」. In place, empty place and workplace are words of their own, and the
  word-head rule leaves them be.
- way is not listed alone: length and long share it. Where it means a route or a way of doing
  something, write path or method, and each phrase in use is a line below.
- job is listed alone, since a job written as the transliteration takes the same particles the verb to catch takes. The
  verb's own endings, the words that open with it (noise, chat, magazine) and the transliteration following an object
  (catching it, catching it) are listed under 「Uses in the word's own sense」; handle and grasp are left be by
  the word-head rule. The transliteration with a particle and with no object before it reads as a job, so the verb is
  written there with its object.
- version is not listed alone: judgement, decision and sale open with it, and a round of a game is one round. Where it
  means a version, write version — the console labels an installed commit a version — and each phrase in
  use is a line below. A phrase line matches judgement, decision and sale after it too, so each of those it
  meets in use is listed under 「Uses in the word's own sense」.

- ko · tool strip → toolbar
- ko · toolstrip → toolbar
- ko · fixed → built-in
- ko · fiddling → edit, revise, correct
- ko · fixing → edit, revise, correct — when it means editing; where it means a repair, the use is written under the allowed uses
- ko · folding a work order → ending a work order — folding means folding a panel away, never ending a thing
- ko · skip and fold → skip and end
- ko · work order being folded → work order being ended
- ko · being folded → being ended
- ko · still folding → still ending
- ko · re-read → refresh — a control or notice that fetches a screen's data again
- ko · re-reading → refreshing
- ko · running → run — running a thing is run: running as co-work is run as co-work, running again is run again
- ko · turning → run — when it means running something
- ko · running (form) → running (form)
- ko · not running (form) → not running (form)
- ko · and running (form) → and running (form)
- ko · while running (form) → while running (form)
- ko · so as to run → so as to run
- ko · or run → or run
- ko · to run → to run
- ko · ran → ran — with it
- ko · to run (future) → to run (future)
- ko · runs (polite) → runs (polite)
- ko · ran (polite) → ran (polite)
- ko · being run → being run — co-work being run is co-work run
- ko · running (progressive) → running, being run
- ko · and running (progressive) → running, and being run — is running is is in progress
- ko · runs → runs
- ko · ran (plain) → was run
- ko · if it runs → if it runs
- ko · search document → archive — the space every kept document stands in, whoever wrote it; one document in it is an archive document, or document
- ko · cause → reason, ground, cause — reason for why a thing is or was so; ground for the reason a person gives for stopping, skipping, ending or taking down a thing, so the cause of folding is the ground for ending and the cause of stopping is the ground for stopping; cause for what made a thing fail
- ko · file tree (native) → file tree
- ko · folder tree (native) → folder tree
- ko · tree structure (native) → tree structure
- ko · node (native) → node, turn — node for a node of a graph or a tree, turn for a turn of a conversation: {n} nodes is {n} turns
- ko · branch (native) → branch, kind, group, number — branch for a branch of a search, a co-work or a workflow; kind for a kind of thing, as in <kind>:<id>; group for a group things are counted under; number for one numbered part of an unfinished note, as in unfinished note 2; where it means splitting, split in two
- ko · offshoot → co-worker — a co-worker's run out of the pool is a co-worker's, whichever road it came by: a co-work task, a search's branch or a job's own call
- ko · place → location, part, point, slot, order, at once — location for where a thing is or goes, so this place is this location and the original place is the original location; part or code for the part of a text or of code that does something, so the reading place is the reading part; point for where a run stopped; slot for a place a pool or a worker holds, so a co-worker place is a co-worker slot; order for an item's place in a queue; at once or immediately for on the spot meaning at once; where it means a matter that is someone's, as in it is the owner's place to decide, matter
- ko · term (native) → word
- ko · asking → question
- ko · head word → header, opening
- ko · tail word → footer, end
- ko · chunk → part, paragraph, section — a three-chunk form is a three-part form
- ko · keeping → capture, save
- ko · route point → path, point
- ko · the same way (native) → the same path, the same method
- ko · the same way, object (native) → the same path, the same method
- ko · another way (native) → another path, another method
- ko · another way, object (native) → another path, another method
- ko · threshold (native) → threshold, criterion — a 95% threshold is a 95% threshold
- ko · worth → value
- ko · weave → structure, composition
- ko · strand → one, item — 3 unfinished strands is 3 unfinished items
- ko · console letters → console wording
- ko · screen letters → screen wording
- ko · job (transliteration) → Job, work — a job: Job where the text names the unit of work, so job #123 is Job #123, job worker is the Job worker and the running job is the running Job; work where it means the work itself
- ko · data set (native) → dataset, data set — a set of data
- ko · irreplaceable token → NFT — a non-fungible token
- ko · upward selling → upselling, upsell — selling a customer a costlier or added product
- ko · latitude pulldown → lat pulldown — the gym exercise, lat read as latitude
- ko · sub-reddit (native) → subreddit — a community on Reddit
- ko · put up a new version (native) → put up a new version — a version: putting up a new version is putting up a new version
- ko · put the version up → put the version up
- ko · the version went up → the version went up
- ko · up to the version → up to the version
- ko · the same version (native) → the same version
- ko · the previous version (native) → the previous version
- ko · the commit's version (native) → the commit's version
- ko · the received version (native) → the received version
- ko · version number → version number
- ko · fire → run, start — launch read word for word: launching a unit, a run, a search or a session is run, and its first moment start

## Uses in the word's own sense

- A term banned in one sense keeps its own sense in the phrases listed here, and a use inside one
  of them passes wherever it is written: in a file, an answer, a report or a row.
- Each line is `<language code> · <banned term> · <a phrase holding the term>`, followed by
  ` — <the sense it keeps>`. The phrase is matched as written, spaces included, around the term.
- A phrase that does not hold its term, or names a term not banned above, is refused.

- ko · place · leaving a place — a seat left empty: leave a seat, left a seat
- ko · place · a place left — a seat left empty: a seat left
- ko · place · sitting at a place — a seat
- ko · place · a place to sit — a seat
- ko · place · a place one sat — a seat
- ko · place · a place one sits — a seat
- ko · place · get up from a place — a seat
- ko · place · leaving a place — leaving where one sat
- ko · place · keeping a place — staying where one is
- ko · place · keeping a place (form) — staying where one is
- ko · place · keep the place — staying where one is
- ko · place · seat reservation — a seat booked
- ko · place · window place — a seat
- ko · place · meal gathering — a gathering
- ko · place · company gathering — a gathering
- ko · place · meet-up — a gathering
- ko · place · place digit — a digit
- ko · place · place count — a count of digits
- ko · place · place code — a code of so many digits
- ko · place · place number — a number of so many digits
- ko · place · -th place — a decimal place
- ko · place · positioning — its own word
- ko · asking · question mark — the question mark
- ko · chunk · a nap — its own word
- ko · threshold · a low threshold — a barrier to entry
- ko · threshold · a high threshold — a barrier to entry
- ko · threshold · crossing the threshold — a threshold crossed
- ko · strand · getting the strand — getting the gist
- ko · strand · the strand is got — getting the gist
- ko · job · catching it — the verb to catch after its object
- ko · job · catching it (form) — the verb to catch after its object
- ko · job · catch — the verb to catch
- ko · job · caught — the verb to catch
- ko · job · catching — the verb to catch
- ko · job · catching (form) — the verb to catch
- ko · job · catching up — the verb to catch
- ko · job · to catch — the verb to catch, and trivial
- ko · job · so as to catch — the verb to catch
- ko · job · catching (noun) — the verb to catch
- ko · job · let's catch — the verb to catch
- ko · job · was catching — the verb to catch
- ko · job · catches (polite) — the verb to catch
- ko · job · if it catches — the verb to catch
- ko · job · so as to catch (form) — the verb to catch
- ko · job · while catching — the verb to catch
- ko · job · since it catches — the verb to catch
- ko · job · as it catches — the verb to catch
- ko · job · please catch — the verb to catch
- ko · job · can catch — the verb to catch
- ko · job · will catch — the verb to catch
- ko · job · shall I catch — the verb to catch
- ko · job · is caught — the verb to be caught
- ko · job · being caught — the verb to be caught
- ko · job · caught (passive) — the verb to be caught
- ko · job · will be caught — the verb to be caught
- ko · job · was caught (passive) — the verb to be caught
- ko · job · being caught (noun) — the verb to be caught
- ko · job · does not catch — the verb to catch, and a magazine
- ko · job · noise — noise, and a slot held
- ko · job · chat — its own word
- ko · job · chores — its own word
- ko · job · odd jobs — its own word
- ko · job · idle thought — its own word
- ko · job · weeds — its own word
- ko · job · sundries — its own word
- ko · job · odds and ends — its own word
- ko · same version · same judgement — a judgement
- ko · same version · same decision — a judgement
- ko · same version · same sale — a sale
- ko · previous version · previous judgement — a judgement
- ko · previous version · previous decision — a judgement
- ko · previous version · previous sale — a sale
- ko · received version · received judgement — a judgement
- ko · received version · received decision — a judgement
- ko · the commit's version · the commit's decision — a judgement

## Allowed uses

- A use that is right in its context is written here, so the check passes it and nothing else.
- Each line is `<language code> · <banned term> · <file path> · <an exact excerpt of the line>`,
  followed by ` — <why it is right there>`.
- An allowed use that no longer matches any line is itself reported, so the list does not outlive
  the text it excused.

- ko · tool strip · prompts/agents/worker/trailer.md · "tool strip"; built-in — the worker's statement of this principle, naming the wrong form
- ko · fixed · prompts/agents/worker/trailer.md · built-in, not "fixed" — the worker's statement of this principle, naming the wrong form
- ko · fiddling · prompts/agents/worker/trailer.md · not "fiddling" — the worker's statement of this principle, naming the wrong form
- ko · re-read · graph/docs.json · re-read should be refresh — the owner's own words kept as a graph alias, naming the wrong form
- ko · tool strip · prompts/agents/chat/manual.md · toolbar, not "tool strip" — the chat's statement of this principle, naming the wrong form
- ko · fixed · prompts/agents/chat/manual.md · built-in, not "fixed" — the chat's statement of this principle, naming the wrong form
- ko · fiddling · prompts/agents/chat/manual.md · not "fiddling" or "fixing" — the chat's statement of this principle, naming the wrong form
- ko · fixing · prompts/agents/chat/manual.md · not "fiddling" or "fixing" — the chat's statement of this principle, naming the wrong form
- ko · toolstrip · prompts/agents/worker/plugin/skills/ui-design-rules/SKILL.md · 'tool strip', — the skill's statement of this principle, naming the wrong form
- ko · place · prompts/agents/worker/plugin/skills/ui-design-rules/SKILL.md · wrong: 'place', — the skill's statement of this principle, naming the wrong form
- ko · place · prompts/agents/worker/trailer.md · location, not "place" — the worker's statement of this principle, naming the wrong form
- ko · asking · prompts/agents/worker/trailer.md · question, not "asking" — the worker's statement of this principle, naming the wrong form
- ko · place · prompts/agents/chat/manual.md · location, not "place" — the chat's statement of this principle, naming the wrong form
- ko · asking · prompts/agents/chat/manual.md · question, not "asking" — the chat's statement of this principle, naming the wrong form
- ko · place · prompts/rules/plain-writing.md · "place" for location — the rulebook's statement of this principle, naming the wrong form
- ko · asking · prompts/rules/plain-writing.md · "asking" for question — the rulebook's statement of this principle, naming the wrong form
- ko · term (native) · prompts/rules/plain-writing.md · "term" for word — the rulebook's statement of this principle, naming the wrong form
- ko · cause · prompts/rules/plain-writing.md · "cause" for reason — the rulebook's statement of this principle, naming the wrong form
- ko · tool strip · prompts/rules/plain-writing.md · "tool strip" for toolbar — the rulebook's statement of this principle, naming the wrong form
- ko · fixed · prompts/agents/worker/plugin/skills/ui-design-rules/SKILL.md · 'fixed'. — the skill's statement of this principle, naming the wrong form
- ko · runs · graph/docs.json · the wording runs should be written as run — the owner's own words kept as a graph alias, naming the wrong form
- ko · running (noun) · markup/markup/wording.ts · rewind is not running — the check's own statement of the word-head rule, naming the two forms it tells apart
- ko · and running · graph/bot.json · three jobs running at once — the owner's own words kept as a graph alias, in the spelling he wrote
- ko · so as to run · graph/scripts.json · rather run only one test at a time — the owner's own words kept as a graph alias, in the spelling he wrote
- ko · search document · graph/bot.json · read it as is and put it in the search document — the owner's own words kept as a graph alias, from before the space was renamed
- ko · search document · graph/bot.json · the search document must be saved separately — the owner's own words kept as a graph alias, from before the space was renamed
- ko · search document · graph/console.json · the tables in the search document — the owner's own words kept as a graph alias, from before the space was renamed
- ko · search document · graph/console.json · the search document page — the owner's own words kept as a graph alias, from before the space was renamed
- ko · search document · graph/bot.json · "search document" — the old name kept as a graph alias of the archive document, so a search in the owner's earlier word still lands on it
- ko · search document · graph/console.json · "search document" — the old name kept as a graph alias of the archive screen, so a search in the owner's earlier word still lands on it
- ko · offshoot · graph/bot.json · "offshoot" — the old name kept as a graph alias of the co-work runner, so a search in the owner's earlier word still lands on it
- ko · branch · graph/bot.json · "branch breakdown" — the owner's own words kept as a graph alias of the job spans, so a search in his earlier word still lands on it
- ko · branch · graph/console.json · "choosing candidate branches" — the owner's own words kept as a graph alias of the documents viewer, so a search in his earlier word still lands on it
- ko · branch · agents/agent/job-spans.ts · { branch: branchWord } — the word spans already kept in an archive open with, named so the archive can rewrite them to the branch word
- ko · job · graph/agents.json · "job body" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/agents.json · "job worker" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/bot.json · "job in progress failed" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/bot.json · "three jobs running at once" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/bot.json · "the job queue stopped" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/bot.json · "job guard" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/bot.json · wasn't there a job to attach the translator — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/bot.json · "the n-th run in the running job — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/bot.json · "job worker" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/bot.json · joining together into several jobs — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/crates.json · talking is not a job — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/console.json · other job workers are individual chats — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/console.json · "job worker in the left panel" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/scripts.json · "deploy once at the end of every job" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/scripts.json · "a test suite per job — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/scripts.json · "the job ended without finishing the work" — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/scripts.json · "the voice briefing is job completion and such — the owner's own words kept as a graph alias, from before Job was the written word
- ko · job · graph/bot.json · "but read the voice briefing as job" — the owner's own words kept as a graph alias, asking for Job to be said job
- ko · job · graph/docs.json · "the Korean spelling of job is work, or Job" — the owner's own words kept as a graph alias, setting this rule
- ko · job · prompts/rules/plain-writing.md · Job or work, never a transliteration of Job — the rulebook's statement of this rule, naming the wrong form
- ko · job · prompts/rules/plain-writing.md · says Job as job — the rulebook's statement of how a briefing says the word
- ko · job · docs/voice-plan.md · | `Job` | job | — the voice name table's row saying Job as job
- ko · job · docs/voice-plan.md · the owner says it job — the reason for that row
- ko · job · bot/bot/voice.ts · the owner says it job — the reason for the seed row saying Job as job
- ko · job · bot/bot/voice.ts · ko: "job" — the seed row saying Job as job
- ko · job · docs/voice-lines.md · (job for Job) — a voice script's reading may say Job as the voice name table does
- ko · job · tools/voice/bmo/voice-lines.ts · Job is read job — why a voice script's reading may say Job as the voice name table does
- ko · job · lang/bmo-lang/lang/ko/agent-catalogue.yaml · "job worker", "worker", "worker job" — the names the owner calls the worker by, kept so his words still find it
- ko · job · policy/bmo-policy/policy/common-words.toml · "job", "job", — a word every turn carries, listed so a description leaning on it is told from one saying what its piece is about; the older word is still in what is written
- ko · job · policy/bmo-policy/policy/closing.toml · (Job|job|item — a report's deferral is read in the older word as much as the written one

## Where it is held as words are written

- **A TODO row:** filing one and rewriting one refuse a title, and the parts the chat agent writes around the
  conversation — what the work is, what to do, the conclusion, a judgement — that use a banned term,
  and name each term beside what to write. This holds for `bmo todo --add` and `--edit`
  and for the chat's `todo_add` alike. The conversation copied into a row is what was said and is
  not read.
- **The chat agent's answer,** in every room and the owner's own chat with them: an answer that uses a banned
  term is asked for once more, with the terms and what to write named, and the answer that comes
  back goes out in its place where it is an answer in the room's language.
- **A worker's record put into Korean:** a rendering of what a job wrote onto an item that uses a
  banned term is asked for again, up to twice, with the terms named, before it is written on the row
  (`docs/worker-language.md`, 「What a worker writes onto an item」).
- **A job's report:** the worker's turn is held at its end while its final answer uses a banned term,
  within the holds a stop is given, with the terms named. A report that still uses one goes out
  with the terms named under it, where the owner and the next job both see it.
- **A job's question to the owner:** `bmo job ask` refuses a question whose question, options or why use
  a banned term, and a talk whose answer or changed question does, naming each term beside what to
  write, before its row is written or the chat is told. A job writes its question in Korean itself,
  with no rendering between it and the chat, so this is the one place its words are held
  (`docs/jobs.md`, 「A job's question to the owner」).
- **Quoted words are not read there.** A span inside backticks or quotation marks is how a text
  names a wrong form — toolbar, not "tool strip" — or quotes a command or somebody's words, and it passes.
  The file check has no such exception: a quoted use in the tree is written under the allowed uses.

## Where it is checked

- **Files:** every tracked file of these kinds, test files and this document excepted.
  - The web console's screens and code under `client/app`.
  - The bot's code under `server` and `scripts`, whose strings reach the chat.
  - **Source is source whatever it is written in.** Under those three roots the check reads
    TypeScript and Vue, and equally a python script, a shell script and a stylesheet: Korean
    reaches a person out of any of them, and a rule naming only the two languages the bot happens
    to be written in let a script's own printed messages past.
  - The documents: `docs/`, `README.md`, `AGENTS.md`, `STRUCTURE.md`, `prompts/rules/`, `prompts/agents/`,
    `prompts/persona/`, and the dependency graph under `graph/`.
- **Language of a line:** a line holding Hangul is Korean (`ko`); a line holding kana and no
  Hangul is Japanese (`ja`); any other line is English (`en`). An entry is matched only against
  lines of its own language, so a mixed English document is checked for a Korean term only on the
  lines that are Korean text.
- **Test files are left out** because they carry made-up titles and quoted input, which are not
  anybody's wording.
- **Data is left out for the same reason, however much Korean it holds.** The sentences a voice
  measurement scores itself against, a recorded output kept under a `fixtures/` directory so a
  test can compare against it, a third-party font's licence, a list of package versions — none of
  it is anybody's wording to fix, and a check that reported it would teach its readers to ignore
  it. A fixture is left out by the directory it stands in, whatever it is written in.
