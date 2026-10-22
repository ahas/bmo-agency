# What agency records of runs

Every agent's own log of what it did, and what is kept of a run that failed. What an agent decided
about a message is not agency's: the judgement records belong to the units that judge.

## Agent log

- Every agent's own internal log, one line per thing it did, written to agency's store as it happens
  (`packages/agent/runlog.ts`).
- Whoever reads a run's stream writes it — the bot for a chat session and a gate, the runner or the
  worker for itself — so a chat session's turn, a gate's question and answer, and a runner's or the
  worker's whole stream all land in one table under the agent's name.
- What the stream carries is what is kept: the run's text and thinking, every tool call with its
  arguments and what came back, MCP tools among them under their own names, and the branches a run
  fanned out, each line tied to the `Task` or `research_branch` call it came from so a branch reads
  on its own.
- A Claude Workflow is kept too, and it is kept differently, because the CLI does not run one as a
  chain of tool calls in the stream: it runs it as a background task of its own, and the stream says
  only that a task started, how its branches are getting on, and that it ended. Those events are
  what the branches' own rows are made of — the script the Claude Workflow was given, each branch's
  instruction, model and outcome — and what a branch actually did is read afterwards from the
  transcript the Claude Workflow keeps for it, so its tool calls, their arguments and what came back
  stand in the log like any other branch's.
- That last part arrives when the Claude Workflow ends rather than as it goes, since nothing of it
  is in the stream before then, and one branch is held to a few hundred lines so a fanned-out job
  cannot crowd out its own log.
- **A pane program's record is filed as its lines are.** A provider whose turns go through its own
  pane program (「The pane the bot drives」 in `docs/agent-control.md`) reports what the turn is
  doing in the record it writes: a `progress` line carries the phase, the tool being called and how
  full the session stands, and the `answer` line carries the turn's usage beside its words. Each is
  filed as the line it is, so a room reading the run as it goes sees the turn's progress and its
  spend without waiting for the end.
- A line is committed the moment it arrives, so a run killed from outside leaves everything up to
  the kill, and the ids run forward, so a finished run is scrolled back rather than only tailed.
- It is bounded three ways — a per-line cut, the last few thousand lines per agent, and an age —
  pruned by the writer as it goes, and over the whole table by agency's own clock: by its server as
  it starts and once a day after.
- The per-line cut falls only on what a machine leaves behind: a tool's arguments, what a tool
  handed back, and a prompt handed to a model are cut, while what a run or the bot wrote in its own
  words — a job's report, a failure reason, a gate's verdict — is kept whole, since those are the
  lines a person reads and they are a small share of what fills the table.
- The console reads the log through agency's run-log route, by agent and from an id onward
  (`docs/api.md`, 「log-lines」), so a finished run is scrolled back page by page.
- **A job's lines are read by the job,** across every run its turns were logged under: its last line,
  and its top-level tool calls and their results with how many model requests it made and how often
  its context was compacted, which is what the job unit's figures and its watch on a running job are
  made of.
- **A note in the bot's own words** is written into a run's log across the boundary by whoever carries
  the run, such as the job unit telling which road a turn went out on or why the run ended. It stands
  beside the run's own lines, under the run's agent, room and job, and is kept whole.
- **A room forgotten takes its lines with it,** but a job's: a job's turns are logged under its room
  and belong to the job, which outlives the room (`docs/api.md`, 「Sessions」, forgetting a room).

## The roads a turn walked

- **Each turn the session contract carries on a conversational agent's line walked one road:** the
  provider and model it went out on, how it ended, how long it held the run and what it spent. The road
  records are the gate unit's table, so agency writes none of them: it publishes each road as
  `road-walked` as the turn ends (`docs/api.md`, 「Events」), and the unit that keeps the road records
  writes the line.
- **What a road carries:** its stage, `chat`, the stage a conversational turn's run is written down at
  (`policy/companion-runner.toml`), the room, the run, the provider and the model, its outcome, `answered`
  or `error`, how long it held the run in milliseconds, its tokens summed over the models that answered,
  none where the road reported none, and how full the session's context window stood once it ended.
- **A turn a reset overtook walked no road,** and none is published for it.

## What a failed run leaves behind

- A run that died says why on its last lines, and everything above them is the program starting up —
  a welcome banner, an ASCII drawing, a rule across the terminal, a warning about the mode it was
  launched in. So what is written down of it is its **tail and never its head**
  (`packages/agent/failure-text.ts`): escape sequences go, then every line carrying no letter and no
  digit, so a drawing and a row of dots take no room from the cause; what is left is cut to two
  thousand characters from the end, on a line boundary, with a `…` where the head went, and output
  that was decoration and nothing else keeps its own tail rather than leaving the record empty.
- **One length, everywhere a failure is written down** — the reader of a run's output, a gate run,
  the warm line and a worker's keepalive, a compaction, an editor run, a research branch, the
  co-worker place ping, a worker session and the job runner — because a record cut to two hundred
  characters in one place and five hundred in another is read as two different failures.
- None of this decides anything: whether a run failed and which failure it is are read off what was
  printed whole, so a login gone and a session the CLI no longer holds are recognised before any
  shortening happens.
- A failed job's report is kept whole rather than clipped, since the job listing its caller prints is
  where anybody goes looking for a cause.

## Why a run died

- A run of an agent that died is diagnosed from what was read where it ended and nothing else: how it
  ended — an exit with its code, an exit by a signal, an error handed back in place of an answer, no
  answer where one was owed, a pane or host that could not be had, or nothing readable — what the
  program said back, cut to one line, and the last tool call its run log holds, with the command or
  file it was given. A cause nobody could read is said to be unknown, and a run that held together and
  said nothing is not counted as broken.
- The account of a dead run is two lines in the words of `policy/chat-health.toml`: what broke and
  why, then what it was last doing and where the rest is — the run's log by its name, or, for a failure
  no run was had for, the journal of the unit the caller names.
- **The failure standing now is kept in agency's settings** under `doctor_standing_failure`, so a
  server that went down over its own failure still knows of it after a restart. It holds the cause,
  what tells this cause from another, the room, how many times in a row the same cause came, and
  whether the owner has heard of it; once heard of, it stays heard of until a run answers again,
  whatever cause the latest failure had.
- **A run that answers again while a failure stands is a recovery,** published as `run-recovered`: the
  facts of the failure and of the answer, what saying it is for, the kind of notice it is and how long
  it stays worth saying (`docs/api.md`, 「Events」). Agency voices nothing; the consumer that owns the
  room says it in its own manner, and the standing failure is cleared.
