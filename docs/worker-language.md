# The language between agency and its workers

A job worker is an agent, and what only an agent reads is English. A result it writes for a person is
the exception: its report, the account it closes its item with, a document it files for the archive, a
measurement report or a research write-up is written in the language of the person who asked
(`docs/agent-documents.md`). Its notes, a question it puts to the owner, which the supervisor puts to
him in his own language, and what it writes into git are English. So the worker reads English, writes
English for agents and the asker's language for the asker, and every other translation in between is
done on the way in or on the way out, never by the worker. Agency's part is what reaches a
worker through it and what a worker hands back through it; the rest — the item's record, the
screens, the rooms — is read in the owner's language by the units that show it.

## The rule

- **What reaches a worker is English.** Every text a worker is handed or reads through agency — a
  job's prompt, words said to the worker, the owner's words in the worker's preamble, a co-work
  step's texts, an item as the queue tools show it — reaches the worker in English.
- **Whoever writes for a worker writes in English at the source.** A caller that hands a job its
  prompt, or a change to it, writes it in English, so nothing waits on a translator and nothing is
  lost to one. Only what a person wrote for a worker directly — words said to the worker, the
  preamble — is translated by agency on its way.
- **The owner's own words are quoted only where the wording is itself the requirement.** A name, a
  label or a sentence to be used as said stands in a ```` ```text ```` fence exactly as said, and an
  English rendering in ordinary prose follows right after the fence, saying all it says and nothing
  more. The fence is code to every language reading and every translator here, so the original
  reaches the worker untouched.
- **A result written for a person is in the asker's language.** A job's report, the account it closes
  its item with, a document it files for the archive, a measurement report and a research write-up are
  written in the language of the person who asked: the item's requester, else its room, as the item's
  brief names it, the same rule a search's result keeps with its question's language. Numbers, tables,
  code, commands, paths, identifiers, sources and quoted words stand as they are. A brief that names no
  language leaves the result in English.
- **What else a worker writes back is English:** its notes, the reasons it holds or stops an item for,
  a question it puts to the owner and a subagent's prompt and report. The worker writes in no persona
  and never addresses the owner.
- **The worker never translates.** A worker that meets another language anyway — a quoted domain
  term, an error message, a file's own contents — reads it as it stands and never renders it for
  anyone.
- **What is never translated:** code identifiers, paths, commands, product and service names, error
  messages, names of address, and domain terms a reader needs to match against the code or the
  screens.
- **What a worker writes into git is English only.** The worker's standing rules say so
  (`prompts/agents/worker/standing-rules.md`, 「Language」), and every landing refuses a commit that
  breaks it (`docs/worker-slots.md`, 「Landing on main」). Every repository a worker lands in is held
  to it, since every worker reads the same standing rules.

## What agency translates on the way in

- **The job run, before the first turn,** translates the owner's part of the worker's preamble
  (`docs/worker-bench.md`, 「Preamble」), and asks that every text of the item the queue tools would
  show the worker — its title, its own sections while it has no brief, the titles of the rows it
  names — has its translation kept, so they are there before the job first asks for its item. A text
  that could not be rendered is shown under the mark (「When there is no translation」), and the job
  starts all the same.
- **The job run, as it pushes words into a running job,** translates the words said to the worker
  before they go in, and the same for a run asked again over words that came after its answer. A
  change to the brief is English already and goes in as written.
- **A co-work step's prompt** is composed by the job unit, which translates what it carries before it
  enqueues the step; an earlier stage's result is a co-worker's own English and passes untouched.
- **No translation for a job's prompt.** Whoever hands a job its prompt, agency hands it to the
  worker as written, through the sanitiser alone. A part in another language that a person handed
  over anyway reaches the worker as written, and the worker reads it as it stands.
- **The hand that translates is the gate service's translator** (`translator`), which runs its own
  judgements outside agency (`docs/own-llm.md`, 「Gates are not agency's runs」), under instructions for a worker:
  quoted speech is translated too, since the person who said it is not the reader. A part already in
  English, a fenced quote in it included, passes untouched and asks nothing of the gate.
- **Nothing a worker runs calls the translator.** The queue tools read translations that are already
  kept and never ask the gate, so a worker's reading never waits on a model.

## Where the translation is made

- **Code decides the language, part by part.** A text is cut into parts the way a long document is
  cut, and each part is read by one count: a part with few letters of the owner's script against its
  Latin ones, outside code, is English and stays exactly as written; only the other parts are
  translated. An English text quoting a paragraph of another language outside a fence has that
  paragraph translated.
- **A part with no letters outside its fences and inline code takes the language of its text.** A
  long fenced quote can be cut into a part of its own, and a part with nothing to count is read as the
  whole text it was cut from reads: a fenced quote inside an English text is never translated.
- **A text is translated in parts,** up to three at a time, and joined back; the text's translation is
  kept only when every part came back.

## What is kept

- **One row per text, keyed by the text itself,** in the translation table the document unit keeps:
  the key is the SHA-256 of the original exactly as written, and the row holds the whole English. A
  changed original is a new key and is translated afresh, while an unchanged one is read straight out
  of the table and never translated twice, whichever item, note or job it stands in.
- **The original is never overwritten.** The translation is a reading of it, kept beside it.
- **Only a translation that passed the structure check is kept.** Every part the gate answers is held
  to the check that it is a translation — not a line introducing one, a word about the gate's role, a
  question back or a summary — asked once more where it fails, and has no translation where it fails
  twice.

## When there is no translation

- **The job is never held for a translation.** A translation is asked once per need and waited for
  only as long as the translator's own time limit.
- **A text with no translation goes in its original, marked:** a line in English before it says it
  could not be translated and stands in its original language. A part that came back is used and a
  part that did not stays as written under the same mark, since half a text in English is better than
  none.
- **A text standing inside a line** — a title, a state, a reason for a hold — wears its mark after it,
  `[not translated]`, rather than a line over it.
- **A failure is not kept.** The next reading that needs the text asks again.
- **An item's instruction keeps English heads even then.** The heads are the queue's own words, so
  the worker still reads each block's label and the fixed words of its source line in English; the
  words inside each block stay as written under the mark.

## What agency writes for a worker

- **Text no person wrote for the moment is English at its source.** The preamble's header, the job's
  trailer, the header over a job's turns, a co-worker place's instructions, and every answer and
  refusal of the worker's tools (`docs/worker-mcp.md`) are written in English where they are kept, so
  nothing waits on the translator and nothing is kept twice.
- **The header over a job's turns names the item as the queue tools do:** its number and its kept
  English title, or, with none kept, the title as written with `[not translated]` after it.
- **What it quotes stays as written:** a title as a person filed it, a phrase a verb was handed, a row
  a query read.

## What a worker hands back

- **A job's run ends with the worker's report,** in the asker's language, returned whole in the run's
  result and in its run record (`docs/run-records.md`). Agency renders nothing of it into the owner's language and
  sends it to nobody: the caller that started the job reads it, and telling the owner is the chat
  agent's.
- **What a worker writes onto an item** goes through the job unit's verbs, which keep the English and
  render it for people to read (the job unit's `todo.md`). Agency's tools hand those verbs the
  worker's text as written.
- **A worker's answer on its own console screen** when the owner speaks to it directly with no job
  behind it is the one place a worker answers in the owner's language: the owner reads it there, with
  nothing retelling it. The owner's words still reach the worker in English.
