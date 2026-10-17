# Formatting judgement (서식 판단)

- The rulebook that decides whether a message people read is folded.
- Answers are written in markdown; code turns them into the room's dialect, and then, for a dialect
  that folds (the only kind with a gate), the formatting gate judges.
- A gate that decides the fold with a model reads this document and `answer-shape.md` on every call.
- How the gate runs (what passes, the order of the steps, what goes out on a failure) is in
  `output-gate.md`, a document for people; this one keeps only what the judgement needs.

## What is judged

- The writer already chose bold, code and links in markdown, and code carries them over as they are.
- The gate neither picks which characters get formatting nor takes formatting away.
- Code measures the one-bold rule from where the bolds sit, and checks that the tags are sound and
  the text escaped.
- One judgement is left to the gate: **is this message folded?**

## Fold judgement

- Whether to fold is the model's call; where to fold is the code's, a rule rather than a judgement.

### Only informational answers are folded

- **Folding is only for informational answers:** search results, answers to a question asking to
  be told something, writing that carries knowledge or an explanation, and reports of what was
  looked up.
- **What sorts them is content and shape, not voice.**
  - A message in the chat agent's voice, calling someone by name, is still informational when it holds
    looked-up facts, figures and explanation.
  - A message where an item-title line and an explanation line follow each other several times is
    informational by that shape alone.
  - A job's work report sent when it ends (one summary paragraph, a body listing what was changed
    and checked as items, a one-line conclusion in counts) has both, so it is informational even in
    the chat agent's voice calling the owner by name.
- **Words exchanged with people are conversation:** a greeting, a reaction, small talk, an
  opinion, a feeling, agreement, a joke. They carry no facts to pass on and no items, and are never
  folded however long they are.
- Bold or code does not make a message informational.
- A folded answer shows only its title and conclusion, the body hidden: right for someone reading
  for information, while in conversation it looks like words half swallowed.
- So the gate looks at the kind first, and measures length only for an informational answer.
  Only words with no facts to pass on and no items count as conversation.

### Whether to fold: the reader's screen

- In an informational answer, **whether** to fold is measured by **the reader's screen**, not the
  character count: fold when the answer pushes the conversation off the screen, not when it fits.
- The boundary is roughly five lines of body on a phone screen.
- A line is a wrapped line on screen. A phone fits about twenty Korean characters to a line, so one
  sixty-character sentence takes three; count lines after wrapping, not in the source.
- A code block does not wrap: one line is one line.

### Where to fold: the code's rule

- **Where** to fold is set by code, taking blocks separated by blank lines as the unit and looking
  inside a block only for the conclusion.
- A code block or a quote is one whole block even with blank lines inside, never split in two.
- The conclusion is never folded: what is left after folding has to be the answer
  (`answer-shape.md`).
- The rules:
  1. A last block of short writing, a few lines that tie things up, is the conclusion and stays
     open. The measure is generous: a body block left open costs a few lines of screen, a folded
     conclusion makes the answer disappear.
  2. When the last block is longer, look at its last line: one finished, short sentence is the
     conclusion, and only that line stays open while what comes before is folded.
     - Writing that runs its conclusion on into the body without a blank line is like that; the
       guarantee cannot hang on how the writing was spaced.
     - An unfinished line is not a conclusion but the last item of a list, and stays where it is.
     - Split this way, the line break before the conclusion still goes out as written: folding
       decides where to cut, not how the writing is spaced.
  3. A code block or a quote is never the conclusion.
  4. In the body without the conclusion, the first block stays open and the rest goes into an
     expandable quote. A first block long by itself is not cut but goes into the folded part; the
     short last block still stays open.
  5. A body of a single block has nowhere to split, so the whole block goes into the expandable
     quote; so does a message of one block with no conclusion.
  6. When the part to be folded is only a few characters, folding gets in the way: code does not
     fold, and such a message is not put to the gate at all.
  7. A quote inside the folded part stands as its characters without a quote tag, since an
     expandable quote cannot hold a quote.
- Code uses the same boundary rules when it folds a message that does not go through the gate.
