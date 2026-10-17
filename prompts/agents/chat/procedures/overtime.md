# A run past its clock

Read this when the bot hands you a run that reached its time limit — an order headed
`[A run reached its clock — keep it or cut it]`. The run is a word said to a worker, a digest's or a
person book's editor run, a browse, or a voice line being spoken. `docs/agent-control.md` 「A clock asks before it
cuts」 is the canonical statement.

## What you are handed

- Which agent and run it is and what it is doing, how long it has run, when it last did anything and
  what that was, and which clock it reached with its length.
- The ask's number, which every command below takes.
- Nothing has been cut. The run goes on while you decide, and nothing is cut until you
  answer.

## Deciding

- **The decision is yours alone.** Never ask the owner whether to keep or cut it; he left it to you.
- Look before you decide, as far as the facts leave it open: the run log the order names, a
  worker's screen and its pane in herdr, what the last activity was and how long ago.
- **Others wait on a kept run.** A digest or person-book editor holds the brain's editing lock, and
  the batches behind it wait for as long as you keep it.
- **Keep it** when it is still working: its last activity is recent, or what it is waiting on (a
  build, a test run, a long command) is expected to take this long.
- **Cut it** when it is stuck: nothing written for a long while with no command that explains the
  wait, the same thing repeated, a question nobody is going to answer, or work that has gone off what
  it was asked.
- A fault you see in the run or in the clock itself — a run that keeps running long for a reason in
  the system, a bound that is wrong for the work — is filed as a TODO item and fixed by a job, as
  todo.md says.

## Answering

- Two services ask, each with its own numbers. A word said to a worker is asked by the job service and
  answered with `bmo overtime` below. A digest's or a person book's editor run is asked by agency, and
  its order names `bmo-agency overtime`: answer it with that command and the same flags, `--keep`,
  `--cut`, `--for`, `--why` and `--list`.
- Keep: `~/.local/share/bmo/deploy/bin/bmo overtime --keep <number> [--for <length>] --why '<one line>'`
  - `--for` takes `20m`, `1h` or a plain number of minutes; without it the run gets the clock's own
    length again, and two hours is the most one answer gives.
  - When that runs out with the run still going, you are asked again.
- Cut: `~/.local/share/bmo/deploy/bin/bmo overtime --cut <number> --why '<one line>'`
  - The run is cut within seconds, and the record says it was your call and why.
- List the asks still open: `~/.local/share/bmo/deploy/bin/bmo overtime --list`
- A refusal saying the run already ended means it finished on its own, or was cut for want of an
  answer: there is nothing left to decide.
- With no answer in thirty minutes the bot cuts the run itself and records it as its own call.

## Telling the owner

- Once you have answered, nothing needs saying: answer `<<SKIP>>`.
- Where you cut it or filed an item about it, tell him in one short line what you did and why.
