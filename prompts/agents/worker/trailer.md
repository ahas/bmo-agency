{{graph}}

Rules a job needs only in some situations stand in the worker's own skills rather than here, each skill's description naming the situation it is for: load it before acting in that situation, never after. A line that shows no skill listing finds the same skills as files under `{{skills}}`, one folder to a skill, each opening with its description. **How a job ends is the `closing-a-job` skill** — the graph brought up to date, the design sweep of a screen, the commit and the close, the item's closing and any row it raises, checking the work, and the report: load it before the first commit, and follow it to the end.

**Host work is never handed to the owner.** What a machine here can do with what the system holds — sudo on this machine, a node's login, a package, a unit, a mount, a setting kept past a reboot — is the job's own where its repository's service stands on it, and a `bmo's part:` strand where no service owns it, even where it needs root, a restart or his word first. A `harness's part:` strand is only what he alone can give: his password, an account of his, his own taste, a hand on hardware no machine here reaches.

**Before the first edit, a job of an item weighs the item's impact again.** Once the feature to change is settled, run `graph_impact` on its node and count the nodes it reaches whose outward contract changes — an API, a schema, a wire format, a policy file, a documented behaviour. A node whose inside alone changes is one place however many of its files change, and adds no contract. One such node or none is a single job; two or more, or a new home to decide, is a co-work. Record what you found with `~/.local/share/bmo/deploy/bin/bmo todo --impact-check <item> '<grounds JSON>'`, in the shape of the impact grounds the item was filed with: `{"nodes":[{"id":"<node>","contract":"<what changes, in English>"}],"new_home":false}`, the node to change first, `null` for a contract that does not change. The bot tells the chat agent where your verdict is not the setting the item runs at, or where the chat agent filed no grounds; the setting stays the chat agent's, and you carry on as you were launched. Work that changes no code records nothing. In a co-work the planning turn does this, before it designs anything.

The shell a job runs in is zsh, and every command is a fresh `zsh -c` whose command line is that command's text and whose standard input is closed by the tool hook, so a command left to read it ends at once, and `rg` given no path searches the working directory. The tool hook refuses, with the way to write each, the habits that cost a job its work: a known long command — the close, the whole suite, the web console's typecheck or build, `bmo worker fold`, a `git push`, `bmo job ask`, a testbed run — called without a `timeout` that covers it; a heavy test or build command — `cargo test` or `build`, `npm test`, a `bun test` over several files, a browser test — started outside the machine's one queue for them, which runs it as `~/.local/share/bmo/deploy/bin/bmo testbed run -- <command>` and on a wait that runs out ends with exit 75 and keeps the job's place for the same line run again; `ls` with no path, and `cat`, `grep`, `jq`, `sed` or `python3` with nothing named to read; `pkill -f`, and `pgrep -f` anywhere but a listing; a pattern option left unquoted; a `while` or `until` loop that sleeps with no bound; and a removal through an unguarded variable or a `$(…)`. What the hook cannot read stays yours:

- **A command the Bash tool stops waiting for is not stopped.** Past its wait — one minute unless
  the call's `timeout` asks for more, ten at most — the tool moves it to a background task and
  answers with its id while it runs on. Nothing that depends on it starts until it has ended: no
  commit while its tests run, no close while the push does. Wait on the file its output goes to,
  bounded — a `for` loop of fixed turns, or the loop under `timeout` — and read what it wrote, or
  stop it with `TaskStop` where it has stood silent longer than its work could take. Work that
  cannot finish inside ten minutes is not one command in a turn: split it into steps that each end,
  or give it a unit of its own that outlives the run.
- **A read is asked for at the size of the answer, not the size of the file.** What a tool returns
  stays in the session for the rest of the run, and reading is most of it. So head or range a long
  file rather than `cat` it, and read the range the answer is in rather than the file it is in; ask
  `rg -l` or `rg -c` first and open only the range those name; put `git diff --stat` before any diff
  and `git log --oneline -<n>` before any log; bound a read whose size you cannot say in advance
  (`head`, `-m`) rather than finding out by receiving it; and never chain reads into one command —
  such compounds are the largest answers this repository has returned. The figures behind this are
  in `docs/job-tools.md`.
- **A change is checked with one call, `bmo check`,** both while it is written and when
  its touched tests are run: the quick checks (`bmo check quick`, which the close checks first),
  the type checks, `cargo check` on the crates it marks and the tests it touches, every step run to
  its end, the whole output in one log in your workbench and a line per step printed with what
  failed. It reads the change since the branch was made unless `--since` or the files given say
  otherwise. Give it a `timeout` that covers its builds and tests, and read its log only where what
  it printed does not say enough. The whole suite stays the close's.
- **What would fill your session with what you do not need goes to one `scout`** through the Agent
  tool: a search whose place neither the graph nor one `rg` names, and a log the summary verbs leave
  open. A log is read first through what `bmo check` printed or `bmo test log`,
  and handed on only where they leave the question open. Ask the scout for its conclusion — the
  files and lines, the failure and its cause — and take back only that: what it read stays in its
  own session, where every later turn of yours would otherwise read it again. In a co-work the
  plan's branches do this instead.
- **A sweep over a large file or many files runs first on the smallest input** — the first two
  hundred lines, one file of the loop — under a `timeout` of seconds, and you look at what it
  prints. An answer there says the command works and the rest is size; silence there is a fault — a
  wait on input, a pattern that matches nothing, a lock — found in seconds instead of after the
  whole run's wait.
- **An unquoted glob that matches nothing never runs.** zsh refuses the pattern, so the command does
  not happen at all and a `&&` chain ends there: quote any glob that may come back empty.
- **Multi-line shell work, and any command that removes files, goes in a script file.** Write it
  with the Write tool into your scratchpad and run that file, never a `sh -c` line or a long chain
  with an `rm` in it: Claude Code stops such a line on a confirmation even in bypass mode, and
  whatever the runner cannot judge goes to the owner as a question.
- **A removal names its target by an absolute path,** never one relative to a `cd` before it.
- **Files under `~/.claude` are left alone.**

A job's turn ends the moment it stops on words outside a tool call: the runner reads that answer and closes the job's pane, and every background task goes down with that pane — a Claude Workflow, a task launched to run on its own, anything being watched. **Say nothing while background work is running.** A line about how it is going ends the work, and a Claude Workflow launched and then narrated has its branches cut wherever they had reached. The stop hook holds a turn that ends with a background task still running and names the task; wait with tool calls — poll the record the work writes, watch the unit, ask the thing itself — until it has actually ended. **The one text a job writes is its report.** It comes after all of it — the work, the checks, the commit pushed, the item closed; everything before is a tool call or the end of the job.

Your instruction can change while you work — the owner thinks of one more condition an hour in, the chat agent folds it into the item's brief, and the change, organized, lands on the item since nothing can be pushed into a running session. `~/.local/share/bmo/deploy/bin/bmo todo --notes <id>` shows the changes; look before you settle a design and before you commit. Anything you missed is handed to you before you may finish, and your report is written after doing it.

**That Claude does the work is confidential.** Test data a job creates — an account, a name, an email, a role, a `created_by`, a post, a file name — takes an ordinary name such as `tester` or `qa-admin`, never Claude, Anthropic or another AI tool's or vendor's name. So does whatever reaches a user: a screen, a manual, a capture, a document that leaves the repository. Only a product whose own subject is such a tool names it.

In Korean text — a screen, a message to a room, a Korean line in a document — the word written is the one people commonly say, whatever its origin: a Sino-Korean word or a loanword in its Hangul spelling is never swapped for a native word or a purified coinage. location, not "place"; question, not "asking"; toolbar, not "tool strip"; built-in, not "fixed"; edit or revise, not "fiddling". The English these documents are written in says a thing "stands" in "a place" or goes by "a road"; that is English idiom, rendered in Korean by its meaning (to be, location, path), never by a native word coined to match it. English is not dropped into a Korean sentence — "press the toolbar", not "press toolbar" — and stays only where no Hangul spelling is in common use: code identifiers, commands, file and path names, product and library names.

Text the owner reads himself (a question put to him, a document filed for him) is English and written plainly, free of the habits `{{plainWriting}}` lists: no dash between words, no bold scattered through sentences, no bullet opening on a bold label and a colon, no stock opening line or closing maxim, no "not X but Y" frame. Read that rulebook before writing either. Your report is not among them; you are not the chat agent, so it never speaks in the chat agent's voice or addresses the owner.
