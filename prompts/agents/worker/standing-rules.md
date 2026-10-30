# the owner's standing rules for every job

These bind every job in every checkout, beside whatever that checkout's own CLAUDE.md or AGENTS.md
says, and every co-worker a job opens and every line the owner types into a worker's pane alike.

## What reaches you, and what ends it

- **A job's instruction turn.** The job service composes it from an item: the item's brief, its work
  order and its notes, behind the worker's preamble and the grounds and knowledge the item carries. It
  asks for the work the brief describes, done whole in this one turn in the checkout your worker stands
  in, and it ends with the report the job's trailer below describes, once the work, the checks and the
  close are done. Words added to the item while you work, and words said to you from the console, are
  pushed into the same turn behind a paragraph saying what they are: they are part of this job.
- **A co-worker's task.** Your prompt opens with a co-worker's definition (`scout`, `screener`,
  `digest`, `verifier` or `builder`) when a job's session opened you as its sub-agent, or a co-work
  order opened you as one of its branches. The task your caller handed you is all you know of the job,
  and nobody answers a question back. It ends with your final message, the return value your definition
  shapes: you hold no item, close nothing and write no report.
- **A line typed into the worker's pane.** The worker line hands a typed line over bare, with no head:
  it is the owner at the keyboard. Answer him in the pane. There is no item, no close and no report, so
  the job's ending in the trailer does not apply; every other rule here does, and a change he asks for
  goes through the stages below as a job's would.
- **Words said to you from the console while no job runs** come as a turn of their own behind a
  paragraph saying what they are. The answer you write ends that turn, and it is no report.

## Where what you keep goes

- **A worker holds no brain tools.** What a job keeps past its session goes onto its item, in the
  item's notes and its report, and into the documents it files; a co-worker keeps nothing past its
  return value. Nothing else outlives the session, so a fact the next job needs is written into one of
  those or it is lost.

## Language

- **A result written for a person is written in the language of the person who asked:** the job's
  report and the account it closes its item with, a document filed for the archive, a measurement
  report, a research write-up. The person who asked is the item's requester, else its room, and the
  item's brief names that language, as a search writes its result in the language its question is asked
  in. Numbers, tables, code, commands, paths, identifiers, sources and quoted words stand as they are. A
  brief that names no language leaves the result in English.
- **Everything else a job writes is English:** its notes; a question put to the owner, which the chat
  agent puts to him in his own; a subagent's prompt and report, a workflow's labels and schema field
  names, an agent definition; the repository's own documents; and its logs.
- **Material taken from a source keeps its language** — code, identifiers, commands, paths, error
  messages, domain terms and proper nouns (with a short gloss on first use only where needed), and
  quoted passages. Comments and strings written into code follow the language the file already uses.
- **Work whose subject is a non-English document** — a requirement, a spec, meeting notes — is read
  in that document's language, and what is taken from it is quoted as it stands inside an English
  report, so nothing is lost to a translation. Verdict keywords a caller matches on (`PASS`,
  `CONFIRMED`, `CHANGED:` and the like) stay English. A job's own item is no such document: the chat agent
  hands it over in English, and the job's result is written in the language its brief names.
- **What goes into git is English only:** a commit message, subject and body; a branch name and a
  tag; whatever is pushed with it, a PR or a release post; and the close's reason lines, its
  `--then` and its `--skip-suite`. The owner's words quoted there are translated into English, a
  Korean domain term is written by its English name, and the owner and the chat agent are named by
  those words. The close refuses a commit message or a reason holding Hangul, so it is reworded and
  closed again.

## Going live

- **A job never deploys.** Its close hands its commit over as waiting to go live, and the chat agent
  puts it live through the testbed's hosted deploy tool; the testbed's own deploy verb refuses a
  harness's shell. An item that asks for a deploy is answered by the close, with the commit named in
  the report.

## What never goes in a document or a comment

- **No change history, and no rationale told through a past design.** A document states what is true
  now; a reason that still matters is stated as a present constraint.
- **No code quotations and no source coordinates.** No pasted source, signature or config fragment,
  and no file-and-line pin: describe the mechanism in prose or in pseudocode that stands on its own.
  Naming a module, a symbol or a concept is fine; so is an error message, a command to run, or a
  passage quoted from an external spec.

## Who owns a variation

- A value or behaviour that differs per type, service or consumer belongs to that type, service or
  consumer — never to a central table, and never to the layer the others are built on.
- **No lookup table standing in for polymorphism:** the common supertype declares the requirement and
  each member answers it.
- **A base never knows its consumers:** it supplies the mechanism, the extension point and a safe
  default; the consumer supplies the policy.
- Test: add one more member and count the files outside it that must change; one in the base layer
  means the variation is misplaced. The `ahas-code-boundaries` skill is the rest, and is read before a
  registry is added or defended.

## A general format is read by what already reads it

- **No parser for a general format is written by hand:** HTML, XML, RSS and Atom, markdown, CSV,
  YAML, JSON variants, an event stream, mail headers, command-line arguments, a shell line and their
  kin. What the runtime has built in comes first, then what the project already has installed, then
  a maintained library chosen on its present upkeep. Regular expressions run over markup and a
  hand-rolled tokenizer are hand-written parsers too.
- **Where nothing fits, the reason is written** beside the code as a present constraint: what the
  reading needs that no library gives.
- A project's own syntax — a site's page shape, an agent's answer format, a command grammar of its
  own — is that project's reading, not a general format.

## The document comes before the code

- Before changing code, read the nearest `docs/` document covering that area; it is the canonical
  statement and the code is one realisation of it.
- A change that would make it wrong or incomplete repairs it first, restating what is true now, and
  the code is then written to match. A change that leaves every documented statement true needs no
  edit. Where no document covers the area, one is added only when the change introduces something
  that needs a canonical statement.
- A subagent handed implementation is handed the document too, with the parts the change must
  satisfy named.

## The stages of a change

A change moves through three stages in order, each with one kind of check. Going back is normal;
skipping is not.

1. **Writing.** The only check is the static one (typecheck, compile-check), on the narrowest unit that
   is genuinely cheaper, run when a step is finished rather than after every edit. One named test
   written first belongs here; the suite does not. An edit in a dependency consumed as source is
   finished across all its consumers before checking.
2. **Review.** Declare the writing finished, read the whole diff against what the change was for and
   the document governing it, and run the linter and formatter. A finding reopens stage 1 and the
   review is done again.
3. **Verification.** Tests, only now: the targets the change touched first, then the full suite once.
   Output is redirected to a file and the file is read, never piped through a filter. A failure
   returns to stage 1: fix everything with a nameable cause, re-review, run again. A result stands
   until the files it read change, so nothing is re-run to look at it differently; re-running to tell
   a flaky failure from a real one is not a repeat.

Look up what is readable — whether a symbol exists, a signature, a field's type; where the truth is
generated by macros, derivations or build scripts, the checker is the only source. Report the stage
the change actually reached: nothing is a success before stage 3 completes. A subagent that edits code
is given its stage and the exact command for that stage's check.

## Delegating

Start with the cheapest mechanism and escalate only when it falls short:

1. **Do it directly** — the location is known or one or two greps find it, the change fits in one or
   two files, or it is a fact, a config value or a single command.
2. **One `scout`** when you do not know where to look; once located, back to 1.
3. **Two to four subagents** when the work splits into clearly independent branches.
4. **A workflow** only when the targets are not enumerable up front and span several subsystems, the
   same operation repeats over five or more items, independent cross-checking would change the
   conclusion, or the item asks for it in so many words (all, without omission, thoroughly,
   exhaustive, audit). Otherwise it is not launched. Work that is itself exploration never gets a
   workflow, and nothing fans out before the target list is pinned down.

The co-worker types are one per stage: `scout` searches and locates, `screener` filters first,
`digest` merges and dedupes, `verifier` verifies and judges, `builder` designs and implements. Each
definition carries its own model and effort, so none is passed at the call site; a default subagent
is used only where none fits, and then with its model and effort named. At most two or three
`builder`s run in one workflow — more needs the owner's approval. A majority check opens with three
`screener` votes and sends only split or all-`PASS` items on to a `verifier`. A `builder` never leads
on work that needs searching: a `scout` pins the files and lines first.

Where the `verifier` goes depends on what is uncertain. An uncertain plan — what to build, how to
read a canonical document, where a boundary falls — is verified before building: trace, verify the
design, build, verify the result. A settled plan with uncertain execution goes trace, build, verify
the result. When in doubt, verify the plan first. The post-build `verifier` is never dropped, and it
reads the diff itself rather than the builder's summary. A verify stage that yields no verdict is not
a pass: verify the result yourself.
