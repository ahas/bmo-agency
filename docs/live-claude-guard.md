# A job opening a real Claude session

## What it costs and who may open one

- **The cost:** opening a new Claude session, copying a session record to continue it as a new
  session, or putting something like `/compact` into a CLI in a pane makes that session write its
  uncached prefix and conversation from the start. For a session with a long conversation, one such
  write exceeds a hundred thousand cache-write tokens.
- **The bot's own sessions** — room answers, job turns, gates, runners, keepalive,
  session folding — are opened knowing that cost.
- **A job's sessions** for a measurement or an experiment are opened only once the job has said
  what it is opening them for. The owner is not asked: the job opens the guard itself, and what it ran,
  and how many times, is kept and carried into the item's report.
- **Who holds the line:** code, not a sentence. It stops a job from spending without knowing it,
  not a job that knows.

## Which processes are a job's

- **Two marks, inherited together.** A process a job's CLI starts as a tool inherits both:
  - the session mark (`CLAUDECODE`), which Claude Code puts on what its tools start
  - the runner mark (`BMO_TASK_LOCKED`), which the job runner sets in its own environment and hands
    down to the job's pane along with agency's variables
- **Who carries what:**
  - A shell command, and a `bun` script that shell calls, carry both.
  - A tool a room's session starts carries only the session mark.
  - The bot's and the console's services, and the runner through which the bot starts a job's turn,
    carry no session mark.
- **The second layer below stands only for a process carrying both marks.** A room's tools have
  nothing to do with this guard.
- **Tool runs the bot starts as a unit or a pane** (search, browse, speaking and the like) are not
  a job's processes.
  - They receive only the environment the bot picked by name, and inherit neither mark.
  - This holds even when a job called the tool that started them: a tool run is opened by the bot's
    tool, knowing its cost, and so are its own children, such as a search's branches.
  - So they are not what this guard is about.

## First layer: a hook that reads shell commands before they run

- **Where:** a PreToolUse hook in the worker settings (`prompts/agents/worker/settings.json`) takes a job's
  Bash command before it runs and judges it. The same hook also takes the job's file edits, for the
  document below.
- **Who judges:** the job guard of the service that holds the jobs, which the deployment names in
  agency's environment as `BMO_JOB_GUARD` and agency hands to every worker program it starts
  (`docs/api.md`, 「Hooks」). The hook runs it through `sh` with the hook's input on standard input. A
  refusal comes back as a deny decision carrying the reason and the piece of the command the guard read
  it in — the program word, the redirect's target, the text handed to `eval` or a shell, and where a word
  was read as a command, why it was read that way. So a job reads what was blocked and why instead of
  guessing at the rule and writing around it. What the guard refuses, below, is that service's; agency
  holds only the hook that hands it the call.

### The four things it refuses

1. **A command that writes into the CLI's record directory.**
   - Every login keeps its own record directory: the `projects` directory under that login's
     configuration directory. The one this machine's own login uses, `~/.claude/projects`, and the
     word `CLAUDE_CONFIG_DIR` are always read as the records; so is the `projects` directory under
     every account's configuration directory, spelled as an absolute path, from `~`, from `$HOME`
     or from `${HOME}`.
   - Which configuration directories there are is agency's store's: the accounts the owner registers
     through agency's account routes (`docs/api.md`, 「Accounts」). The hook reads them off the store's database
     before it judges a command, and hands the list to the guard; an endpoint account keeps its
     records in its own configuration home, which is listed like a directory account's; a token
     account keeps its records in the machine's directory, and the machine's own directory in the hook's environment
     is added beside them. A store with no account, or one the hook cannot read, leaves the
     guard with the machine's spellings alone, which it refuses exactly as before.
   - Among commands that mention the records anywhere: those that copy, move, link, delete or
     create (`cp`, `mv`, `ln`, `rm`, `rsync`, `tee`, `touch` and the like), edit in place
     (`sed -i`), or redirect to anything other than the null device or a literal path outside
     every configuration directory.
   - A target written as a variable the command did not itself assign could be anywhere, so inside
     a command that mentions the record directory it counts as a write.
   - Reads (`grep`, `jq`, `tail`, `ls`) are not refused.
2. **Calling the `claude` executable.**
   - The same whether it stands as one stage of a pipeline; after an assignment or `env`,
     `command`, `exec`, `timeout`, `nohup` or `xargs`; written as a path; or inside the string of
     `bash -c` or `zsh -c`.
   - Also code that writes `"claude"` as one slot of a command line (such as
     `Bun.spawn(["claude", …])`). A name in backquotes counts only inside such a list's brackets;
     standing in prose it is a markdown code span naming the program, not an argument of one.
   - `.claude/` inside a path, or the word standing as an argument, is not a call.
   - A call with `-p` or `--print` among its words is print mode, which no agent of this system runs
     (`docs/agents.md`, 「No print mode」): it is refused whatever the job has opened.
3. **Putting words into a session in a pane.** Code that writes herdr's `agent.prompt` call, and
   driving agent or pane placement with the `herdr` command.
4. **The flag that forks a session:** `--fork-session`.

### How it reads a command

- **As the shell splits it.** A command line is read by brush-parser's tokenizer and word parser,
  which keep each word's quoting as written and give a heredoc's body apart from the commands, as
  the text it carries. A line they cannot read — a quote left open, a construct of zsh's alone —
  reads as no command at all: the shell refuses the first, and the second is out of the hook's sight
  as a script file is.
- **One layer deeper, the way the shell reads it.** A variable the command itself assigned is read as
  its value, so `A=claude; $A`, and a copy with the record directory split across two variables,
  are judged the same.
- **Text run as a command.** Where a command has such a place — a string passed to `eval`, `source`
  or `.`, a shell reading standard input or a process substitution with no script file, a variable
  or command substitution standing in the command position — every word inside is read as a
  command too.
- **A backquote is a command substitution only where the shell opens one.** Inside single or double
  quotes it is a character of the word, so nothing in a quoted report, commit message or judgement
  is read as a program: ``--report 'added the node (`bot/claude`)'`` names a graph node and starts
  nothing, while ``(`echo claude`)`` outside quotes still does. A `$(` is read the same way.
- **Never caught as a live Claude session:** `bun run test`, `bun test`, `bmo-agency worker fold`,
  the deploy unit's commit, and git commands. Some of them still have to ask for a wait that covers
  them (「The same hook refuses the shell habits that stall a job」).

### When the hook cannot run

- If the hook cannot run the guard — the worker's line names no source checkout, the deployment names
  no guard, the guard is missing, or it fails or reads no input — the command is not blocked. The
  guard's judgement is waited for until the guard ends: the hook's limit is the longest the program
  takes, a day, and nothing cuts the guard short.
- One hook failing to run must not stop every shell command of every worker, and the second layer
  stands behind it.

### The hook moves the worker's prefix

- The hook is in the worker settings file, so it moves the mark of the worker's prefix.
- After a deployment that changed these settings, run `bmo-agency worker fold`
  (the deploy unit's `deploy.md`).

## The same hook holds the owner's statement of the system

- **What it guards:** the file agency's policy names as the owner's statement of the system, which a
  job changes only on the owner's own request.
- **Where:** the same hook takes every file edit a job makes (Edit, Write, MultiEdit, NotebookEdit)
  as well as its shell commands.
- **What it refuses:**
  - a file tool whose target is a file of that name
  - a shell command naming it that writes to it: a redirect into it, a writer program (`cp`, `mv`,
    `rm`, `tee`, `touch` and the like) or an in-place editor given it, `git rm`, `mv`, `checkout`,
    `restore` or `apply` on it, and an interpreter's inline script whose write call names it — or
    that names it and writes somewhere the reading cannot resolve, which is refused rather than
    guessed at
  - the same inside `bash -c`, `zsh -c` and `eval`, with the variables the command assigns read as
    their values
- **What it lets through:** reading it — `cat`, `grep`, `git diff`, `git add`, and a script that
  only opens it to read.
- **Who may change it:** a job whose TODO item was requested by the owner (the requester id is the
  owner's) and whose title or instruction names the file. The job is found by its run name,
  as for the live-Claude opening. No question is asked and nothing else opens it.
- **Where the code differs from it:** the job reports the difference; the refusal says so.

## The same hook refuses a removal the CLI would stop on

- **Why:** Claude Code asks a person before some removals even in bypass mode, and a job's pane has
  nobody to answer (`docs/agent-control.md`, 「A confirmation in the middle of a turn」). A removal
  through an empty variable is also a real danger: `rm -f $S/*.md` becomes `rm -f /*.md` and
  `rm -rf $S/` becomes `rm -rf /`.
- **What it refuses:** an `rm` or `rmdir` — behind assignments and runners, inside `bash -c`,
  `zsh -c` and `eval`, and on any line of the command, heredoc lines included — one of whose
  targets
  - holds a command substitution, `$(…)` or a backquote, or
  - holds a parameter expansion that does not stop on an empty value: anything other than
    `${NAME:?…}` or `${NAME?…}`. Text in single quotes is literal and not read.
- **What it lets through:** a literal target, a guarded one (`"${S:?}"/*.md`), and a removal whose
  targets come from elsewhere (`xargs rm`, `find … -delete`).
- **Nothing opens it.** The refusal says how to write the removal instead: guard the
  variable, quote the path, or name the path outright, and leave files under `~/.claude` alone.

## The same hook closes every command's standard input

- **Why:** the Bash tool opens a command's standard input on `/dev/null` only while the command's
  text holds no input redirection. One `<`, `<<` or `<<<` anywhere in it, and the tool hands the
  whole shell a socket instead, which stays open for the whole run and is never written to. A
  `</dev/null` given to one part of a compound then leaves every other part on that socket, and a
  reader given nothing to read — `rg` with no path, a path held in a variable that came back empty —
  waits on it with no end.
- **What it does:** every Bash call it lets through, the job's own and a co-worker's alike, goes to
  the tool rewritten with `exec </dev/null` on a line of its own ahead of the command. The shell's
  own standard input is then `/dev/null` for everything the command runs, a part sent to the
  background included, and a reader given nothing ends at once: `cat` or `grep` reads an empty
  input, and `rg` given no path searches the working directory, as it does from a terminal. A pipe,
  a redirect, a heredoc and a here-string still feed the command they stand on.
- **The tool's wait does not end such a command.** The worker's settings set the tool's wait, the
  default and the most a call may ask, to the largest the program takes, so a call that names no
  `timeout` waits for its command to end. A call that names a shorter one has the tool move the command
  to a background task past it, where it runs on. So closing the input is what ends a reader's wait.
- **When the hook cannot run,** the command goes to the tool as written.

## The same hook refuses the shell habits that stall a job

- **Why:** a job's shell is a fresh `zsh -c` for every call, its command line holds the command's
  own text, its standard input is closed (above), and zsh refuses a glob that matches nothing. Each
  habit below is a command that then runs for ever, runs past the call that started it, answers
  nothing, or never runs at all, and the refusal arrives at the moment the command is written, with
  the way to write it.
- **What it refuses**, read the way the shell splits the command — behind assignments and runners,
  inside `bash -c`, `zsh -c`, `eval` and command substitutions, and never inside a heredoc body,
  which is text:
  - **A known long command with no wait asked for.** The close and the check of a change, by their
    script or their command, the whole suite (or `bun test` with no file), the web console's typecheck
    or build, `bmo-agency worker fold`, asking the owner, the landing verb, a testbed run and
    `git push`, in a Bash call whose `timeout` is under five minutes and which does not
    run in the background. Past its wait the tool moves the command to the background and answers
    at once, so what depends on it would start before it ends.
  - **A reader named nothing to read.** `ls` or `eza` given no path, and `cat`, `head`, `tail`,
    `wc`, `sort`, `grep` (not recursive), `sed`, `jq` (not `-n`), `python3`, `tr` and `xargs` given
    nothing to read, where neither a pipe, a redirect, a heredoc, a here-string nor a `while` loop's
    own input feeds them. On the closed standard input it would answer nothing, and an empty answer
    reads as one that found nothing.
  - **A `-f` match that finds its own shell.** `pkill -f` in any form, and `pgrep -f` other than a
    listing (`-a`, `-l`): the pattern stands in the command line of the shell running the call, so
    the kill takes that shell and a test or a wait built on the match never falls.
  - **A pattern the command expands for itself, left unquoted.** Any `--option=value` word whose
    value holds `*`, `?` or `[` outside quotes, and the value after `-name`, `-iname`, `-path`,
    `-ipath`, `-wholename`, `-lname`, `--glob`, `--iglob`, `--include`, `--exclude`, `--exclude-dir`,
    or `rg -g`.
  - **A sleeping wait with no bound.** A `while` or `until` loop in a command that also sleeps,
    unless the loop runs under `timeout`.
- **What it lets through:** the same commands with a path, a file, a redirect, a quoted pattern, a
  covering `timeout`, `run_in_background`, `pgrep -af`, a `for` loop of fixed turns, and a loop
  under `timeout <secs> zsh -c '…'`.
- **Nothing opens it.** Each refusal names what the command would have done and how to write it.

## The same hook sends a heavy command to the testbed

- **Why:** one heavy test or build command holds the machine at a time (the testbed unit's `testbed.md`), and a
  job's shell is where another repository's `cargo test` or `npm test` starts. The hook stands on
  every worker's line, whatever checkout the job works in, so it is the one place every such command
  passes.
- **What it refuses:** a command some language of the testbed claims as heavy, read the way the shell
  splits it as the habits are, that is not itself a run of the testbed.
- **What it answers with:** the whole command written after the testbed's run verb and
  `--`, with the call's `timeout` to give it.
- **What it lets through:** a light command no language claims, the testbed's own run, and a verb of the checkout's own
  that takes the turn itself — the close, the suite's script, a cargo wrapper.
- **Nothing opens it.**

## The same hook keeps a job in its own checkout

- **What it guards:** every registered worker's checkout — each base and each support worker — but the
  job's own worker's and the ones the job has borrowed (`docs/worker-slots.md`, 「A job writes only in
  its own checkout」).
- **What counts as a write there:** a file tool whose target lies in it, a redirect, a writer
  program, an in-place editor, a git command that changes files, an interpreter's inline script, and
  relative paths after a `cd` into it. A path is matched at a boundary, so a support
  worktree whose path begins with its base's is never read as the base, and `~` and `$HOME` spellings
  count.
- **What it refuses with:** whose checkout it is, and `bmo-agency borrow <worker>` by the deploy tree's path,
  which lends the job a free checkout of that repository.
- **When it cannot tell:** a job whose row or worker it cannot read, or a store that holds no loans yet,
  refuses nothing. The job's own checkout is also what the shell's verb refusals
  (`job-tools.md`) read as 「the checkout」.

## Second layer: the guard in agency's code

- **Why it exists:** the shell hook cannot see what a script file calls. If a job writes a script
  that uses agency's code to open a session or put words into a pane, the command line shows only
  `bun <file>`. So agency's code blocks it once more.
- **One guard for every launch.** Every place that starts a Claude Code process — directly through a
  pipe or inside a herdr pane — and every place that puts words into a CLI in a pane goes through
  one guard. In a job's process the guard throws with the reason, and the process does not start.
- **The herdr client asks too.**
  - Calls that put words or keys into a pane, and calls that open a pane carrying the CLI as its
    command, are blocked before they reach the socket.
  - The function that opens a pane asks before it creates the workspace.
  - So a script that calls the pane-opening function or the herdr client directly, bypassing the
    function that runs a turn, meets the same guard.
- **What it blocks:** the lines that hold a session or write a prefix the next run will read — job,
  room, a companion's turn, search, browse, and a session inside a pane.
- **A pane holding another provider's program** — Codex's, standing in an agent's pane — is still a
  pane a job does not put words into, so the job refusal holds there too. No Claude session is
  written by it, so Claude being off does not refuse it and nothing is recorded as a Claude run.
- **Agency's server's resident door asks the same marks.** In a test it reaches only a stand-in herdr under
  the test's home or the temporary directory; in a process carrying both of a job's marks it refuses,
  since the opening a job makes is read by the bun guard alone, except a run its procedure requires
  that the verb declared first (「A run the procedure requires declares itself」); and Claude Code switched off
  closes its turns on a Claude Code program as the bun guard does.
- **What it lets through:** a gate run judges once on a small fixed instruction and leaves nothing
  behind, and the tools a job uses (asking the owner, sending a message) may call one, so it is not
  blocked.
- **A test holds the list of files that start the CLI.** A file that starts the CLI without going
  through the guard fails that test.
- **Tests:** a process carrying the test mark follows the test rules instead of this one
  (the operations repository's `test-suite.md`).

### A run the procedure requires declares itself

- The session fold after a deployment that changed the worker's prefix
  (`bmo-agency worker fold`) is such a run.
- That verb declares the purpose of its run to the guard in agency's server code before it opens anything,
  and the guard opens for the process that declared it.
- A test holds the list of files allowed to declare. Any other file that declares fails the test.

## A job opens it by saying what for

- **The way in:** when the guard blocks, the refusal names it. The job runs
  `bmo-agency live-claude --purpose '<what it measures and why>'` and goes on; nobody is asked
  and nothing waits.
- **What it lets through:** the interactive program, a fork, a write into the records and words put
  into a pane. Print mode stays refused, and its refusal names no way in.
- **Scope:** the opening stands for the one job whose run name it was made under, found by that
  name as for everything else here. The stages of a work order are separate jobs, so each stage
  opens it separately. A process outside a job has nothing to open, and the script refuses it.
- **Both layers read the same opening.** An opening is kept in agency's store under the run name it
  was made under; the guard looks for an opening of its run's name there. Which job and item a run is,
  agency does not read: a job's rows are the job unit's.
- **A copy is still the same job.** A script a job points at a copy of the store — `BMO_DB_PATH`
  or `BMO_DATA_DIR` naming a workbench copy — reads the store it was named first, and where that
  holds no job of the run's name, agency's own store, which holds the job and its opening. The
  opening is made there too, and every run it lets through is recorded where it was found.
- **Agency's own store is found where agency reads it**, whatever data root the script is
  pointed at: the PostgreSQL database the own data root's store setting names, and that root's
  `bmo.db` file only where the setting names none.
- **Every run it lets through is recorded** against that opening: the line or the command, and
  when. The hook records each shell command it passes because of the opening; agency's code
  records each launch or pane prompt it passes, a launch once however many places on its way ask
  the guard about the same command line.
- **The count reaches the report.** `bmo-agency live-claude --list` prints what this job has
  opened and run, and agency's run routes serve the same to the job unit, whose close adds one line
  to the report
  for each opening made by any of the item's jobs — its purpose and how many runs it let through —
  so the record says what was spent even when the job's own words leave it out.

## What the guard is not

- It is a wall against spending without knowing, not a sandbox.
- It cannot stop code written to get around it, and getting around it is spending without
  the record the opening keeps.
