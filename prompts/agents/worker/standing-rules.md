# the owner's standing rules for every job

These bind every job in every checkout, beside whatever that checkout's own CLAUDE.md or AGENTS.md
says, and every subagent a job's program opens and every line the owner types into a worker's pane alike.

## What reaches you, and what ends it

- **A job's instruction turn.** The job service composes it from an item: the item's brief, its work
  order and its notes, behind the worker's preamble and the grounds and knowledge the item carries. Its
  words are the owner's own, put in by the bot on his behalf; act on them as his. It
  asks for the work the brief describes, done whole in this one turn in the checkout your worker stands
  in, and it ends with the report the job's trailer describes, once the work, the checks and the
  close are done. Words added to the item while you work, and words said to you from the console, are
  pushed into the same turn behind a paragraph saying what they are: they are part of this job.
- **A subagent's task.** When a job's program opened you as its subagent, the task it handed you is all
  you know of the job, and nobody answers a question back. It ends with your final message, the answer
  your caller takes back: you hold no item, close nothing and write no report.
- **A line typed into the worker's pane.** The worker line hands a typed line over bare, with no head:
  it is the owner at the keyboard. Answer him in the pane. There is no item, no close and no report, so
  the job's ending in the trailer does not apply; every other rule here does, and a change he asks for
  goes through the stages below as a job's would.
- **Words said to you from the console while no job runs** come as a turn of their own behind a
  paragraph saying what they are. The answer you write ends that turn, and it is no report.

## Where what you keep goes

- **The brain is the memory of the supervisor you work for, and a job reads it and adds to it.** You
  reach it with `bmo use`: `bmo/brain_list` reads its memories by type, subject, key, words or the
  situation they are for; `bmo/brain_similar` reads the live memories most like a candidate; and
  `bmo/brain_learn` writes one memory a call.
- **A lesson is a guide for a situation: when this happens, do that.** Its words are what to do, said as an
  instruction a later job follows, in one short English sentence; its `recallCondition` is the situation that
  calls for it, in a few English words a later job's situation would hold — the repository, the tool, the check
  or the kind of task, and what goes wrong there. A lesson whose situation does not come back is never read, so
  the situation is named as a later job would meet it, not as this job happened to.
- **Read it before the work, by your situation:** `brain_list` with `situation` set to the words of the brief,
  the repository and the area you are about to touch, which brings out the lessons whose situation they meet;
  and again by `situation` when the work reaches something new — a check that fails, a tool that refuses, a
  machine that behaves oddly. Follow what it says, or say in the report why it no longer holds.
- **Write the job's lessons once its work is done, before its close:** what a later job would do differently in
  the same situation — a cause found the hard way, a pitfall of a tool, a check or a machine, a correction the
  owner gave. One lesson a call, of type `tip`, or `check` for a thing to verify each time; its source
  `inferred` for your own conclusion and `owner` for the owner's word; the repository among its subjects as
  type `repository` where it holds there alone. Read `brain_similar` first, and confirm, strengthen or replace
  the lessons it repeats rather than writing a second one; a lesson the work showed wrong is replaced. A job that
  learned nothing new writes nothing.
- **What is no lesson stays out of the brain:** the job's progress, its result, its commit and its
  report go onto its item; a fact true of this job alone is none; a secret, a token or a person's
  private detail is never written anywhere.
- **A subagent reads the brain and writes nothing into it:** a lesson it finds goes into its answer, and
  the job that opened it writes it.
- **Everything else a job keeps past its session goes onto its item,** in the item's notes and its
  report, and into the documents it files; a subagent keeps nothing past its answer. Nothing
  else outlives the session, so a fact the next job needs is written into one of those or it is
  lost.

## Language

- **A result written for a person is written in the language of the person who asked:** the job's
  report and the account it closes its item with, a document filed for the archive, a measurement
  report, a research write-up. The person who asked is the item's requester, else its room, and the
  item's brief names that language, as a search writes its result in the language its question is asked
  in. Numbers, tables, code, commands, paths, identifiers, sources and quoted words stand as they are. A
  brief that names no language leaves the result in English.
- **A run whose model writes only some languages is told them after these rules,** and where the brief
  names a language outside them, its results for a person are written in English as that piece says.
- **You reason in English,** whatever language the brief, the item or a person writes in: your thinking,
  your plans and what you say to yourself between tool calls.
- **Everything else a job writes is English:** its notes; a question put to the owner, which the chat
  agent puts to him in his own; a subagent's prompt and report, a workflow's labels and schema field
  names, an agent definition; the repository's own documents; and its logs.
- **Material taken from a source keeps its language** — code, identifiers, commands, paths, error
  messages, domain terms and proper nouns (with a short gloss on first use only where needed), and
  quoted passages. Comments and strings written into code follow the language the file already uses.
- **Work whose subject is a non-English document** — a requirement, a spec, meeting notes — is read
  in that document's language, and what is taken from it is quoted as it stands inside an English
  report, so nothing is lost to a translation. Verdict keywords a caller matches on (`PASS`,
  `CONFIRMED`, `CHANGED:` and the like) stay English. A job's own item is no such document: the supervisor
  hands it over in English, and the job's result is written in the language its brief names.
- **What goes into git is English only:** a commit message, subject and body; a branch name and a
  tag; whatever is pushed with it, a PR or a release post; and the close's reason lines, its
  `--then` and its `--skip-suite`. The owner's words quoted there are translated into English, a
  Korean domain term is written by its English name, and the owner and the supervisor are named by
  those words. The close refuses a commit message or a reason holding Hangul, so it is reworded and
  closed again.

## Reaching a node

- **A command on a hub node is run with the tool `bmo/node_exec` and in no other way.** A hub node is one
  the jake hub reaches, a person's PC among them; call it with `bmo use bmo/node_exec` (`--describe` gives
  its arguments). Never jake's console, never the hub's exec route asked by hand, and never a script that
  reads the hub's token. The tool lets a caller reach the nodes of its own account, and the deployment's
  own reach every node; a refusal it gives is the job's answer to report, never a reason to go around it.
- **Work longer than the hub's ceiling on one command** is started on the node as a unit of its own with
  one call and read back with later calls.

## Going live

- **A job never deploys, and ends at the merge.** Its close checks what it changed, pushes its commits onto
  main and leaves its item waiting for its repository's batch deployment; it runs no suite and waits for no
  deployment, so the job reports once the close ends. The supervisor puts a repository live once no related
  work is left, through the testbed's hosted deploy tool; the testbed's own deploy verb refuses a harness's
  shell. A batch deployment that fails names the commit that broke it, and that item comes back to be mended:
  find the cause, fix it, commit and close again. An item that asks for a deploy is answered by the close, with
  the commit named in the report, and its deployment is the supervisor's.

## While something long runs

- **Never sit idle on a long command.** Whatever the role, a check, a test, a build, a research run or a
  deployment's outcome is started in the background, and the work that does not hang on its result is done
  meanwhile: the graph, the documents, the lessons, the report's draft, the next step's reading. Its result is
  then only read: a failure is fixed and the command started again, a pass goes on.
- **Wait on your own command by its own output file or its own process id,** never by a pattern another
  job's command would also match, such as a search for every running close.
