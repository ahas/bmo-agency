# The Claude launch lines

## What this document holds

- **How each agent's Claude process is started:** its model and effort, the tools it is shown, its
  permission mode and allow list, what of this machine's configuration it reads, how its system
  prompt is delivered, and its environment.
- **Where the lines are:** agency's server's `lines` module — the chat, a companion's turn, the job
  worker and the runners. A gate's judgement is the gate unit's to run, on the gate line
  described below; agency's TypeScript side runs none. The interactive program an agent keeps
  standing in its pane is built from those same lines by the Claude Code surface in
  `packages/agent/claude-code.ts` (`docs/agent-control.md`, 「The interactive surface a
  provider declares」).
- **Two shapes of one line.** Where an agent's line keeps a session — every conversational agent on
  the chat line and the job worker —
  the line is started once as the interactive program standing in the agent's pane: the same
  flags without print mode and without a prompt, the session named by `--session-id` for the first
  start and by `--resume` for every start after it, and `--settings` naming the agent place's
  settings file — the status line and the hooks — placed after the allowed tools so that list flag
  never swallows it. Every turn is words put into that program. A line that keeps no session — a
  runner's — runs each run apart: the same interactive program in a
  pane of its own, opened on a session of its own for the run and closed with it (`docs/agents.md`,
  「Runs apart」). A gate's line stands as a pool of the same interactive programs, each cleared between
  the judgements put into it (`gates.md`, 「A gate on Claude Code」); on dsh it stands in places kept
  for each program a gate's ask composes, which stay up between judgements
  (`docs/agent-control.md`, 「A gate's places」). No line has a print-mode
  shape: the code refuses a run that asks Claude Code for its answer on the program's output
  (`docs/agents.md`, 「No print mode」).
- **A line is this repository's code, changed by the work that needs it.** A job whose item is
  served by a line carrying something more or something else — a tool, a server, a setting — changes
  that line as part of the item, in the same commit as the code that uses it. What holds a line
  steady is the cache (「Why a line is held steady」 below): a change is made whole and once, never as
  a side effect of other work and never to a line the item does not need.
- **Which provider carries a piece of work is a different question** and is not settled here: every
  work key — the chat, the job worker, each gate — has its route in agency's store, the
  owner sets it in the console, the program that runs the work follows from that provider, and a
  provider whose program cannot carry that work is refused rather than silently standing in for it.
- **An agent's own thinking and effort stand over its tier.** The chat agent, a worker and a runner go
  out with the effort level and the thinking its agent's row names where the row names one
  (`docs/model-tiers.md`, 「An agent's own thinking and effort」). On Claude Code the level is the
  `--effort` word and thinking off is `MAX_THINKING_TOKENS=0` in the environment, so no flag moves that
  is not already the line's. Where the row names nothing, each line below is as written.

## Harnesses

- A launch line is one harness's answer to a description of a run. What a run is gets said once, in
  agency's own words and no program's (`packages/agent/harness.ts`): the model and the effort, the
  system prompt as a path to a file (both the CLI and any second harness take it that way, and a
  chat's prompt is tens of kilobytes no command line would hold), the tools shown, the tools allowed
  and the tools kept back, the MCP configuration, the prompt and whether it is written to the run's
  input or stands in the command, the session resumed and whether one is kept at all, whether the
  harness's own commands are reachable, whether the model thinks, the shape the answer comes back
  in, the directory and the environment.
- Not one of those fields is named after a flag, because the whole point is that another program
  answers the same questions in different words.
- A resumed harness reads that file again, so the runtime prompt files survive the atomic rebuild of
  the deployed agent-document tree; a restart must not leave a stored thread pointing at a path the
  rebuild erased before its first keepalive.
- A harness answers two: the command for a run, and the reply read out of what the finished process
  left behind — and both halves hold, so what a run printed is turned into a reply by an
  implementation and by nobody else.
- Whatever a caller wants of a finished run — the text a judgement answered with, the session a fold
  folded, the account's windows, what the run cost — it asks the harness that built the command for
  the reply and reads it off that; a run carried by one program and read in another's vocabulary
  would throw on its first answer.
- **Where the stream while it is still arriving says something in the program's own words, that too
  is read behind the interface**: a watcher learns which session a run is answering in, and what the
  run's events say about the co-workers it opened, by handing each event to the harness rather than
  reading those words itself, and a harness whose program streams neither answers neither.
- What is still outside the interface is the reading no second program has yet wanted: a run's own
  log, what it is doing this minute, the spans a job's time is broken into and what a run killed at
  its ceiling had written are all read off the bytes as they come, in Claude Code's own stream
  vocabulary, because there is no reply until the process has ended — so a second harness that
  streamed anything of its own would want that reading behind the interface as well.

### The harnesses this build carries

- **Claude Code is the one harness every provider's runs go out on** (`packages/agent/claude-code.ts`), and
  every flag the CLI is spoken to with is written in that one file — the five lines (a chat round, a gate's
  judgement, a job, a search, a browse) and the two folds are its answers, each in its fixed order.
- **dsh — the DeepSeek Harness — remains in the build** (`packages/agent/dsh.ts`) for a rollback to the road
  this deployment left, and no provider of this build names it. Its program takes no flags to speak of, so a
  run becomes a patch overlay written fresh for it, naming rows of dsh's own composition — the run's own prompt
  as the persona suffix after dsh's own, the model and the provider's API it runs on as a route, the tools it
  may have — and the command is the pinned `@deepseek-ai/dsh` at its headless profile with the task standing in
  the line. The task stands there as words headless joins back with one space, each short of the kernel's limit
  on one argument, and it reaches dsh out of a file of its own rather than through the string npx hands a shell,
  because a task can run to hundreds of kilobytes and that string is refused whole long before then.
- **dsh reads every `{{name}}` in a persona as a variable of its own** and refuses the whole prompt on a name
  it does not hold, so each `{{` of the text agency hands it is broken by a word joiner, which the model reads
  past; the headless opening line's own `{{model}}` is left for dsh to fill.
- **What a run may do to the files around it is not one of those rows alone**: dsh keeps how far a write may
  reach and whether the run stops to ask as two knobs, and names only the pairs they make together, so a run
  that set the reach and left the asking behind would be a composition dsh refuses outright — before a word
  goes anywhere. The mode therefore travels in the run's environment as the one name dsh resolves both ends
  of, as well as on the row that states it.
- **The Codex CLI remains in the build** (`packages/agent/codex.ts`) for a rollback, and no provider of this
  build names it: `codex exec` on the machine's own login, its JSON events read back into the same reply, a
  thread id kept per scope, model and effort.
- **A harness after those is one more file.** What it has to write is the whole of the interface: a name to be
  registered under, the lines it can really carry, the word its own program folds a session with, a command
  for each of those lines, a reader, and — where its program has them — the session one of its streamed events
  names, what those events say about the children a run opened, and the tail of the record it kept for a
  session it keeps.
- What it does not write is everything above it — it composes no documents, decides no policy about who may
  use which tool, records no usage, judges nothing about the account, and never reads the route that chose it.
- **What a harness cannot answer it says so about rather than pretending**: dsh's own prompt preamble stands in
  front of whatever prompt it is given, its tools are its own and can be had whole or not at all rather than by
  name, it keeps no session agency could resume, and it reports nothing about what a run cost — so a reply off
  it names no session and carries no figures, and nothing downstream is told a number that was never reported.

### Providers and what a route may promise

- **The owner chooses a provider, never a harness.** The providers are Claude Code, Codex, OpenRouter, the
  registry's and DeepSeek.
- Each is declared beside the program that carries it and answers for itself whether it can be used on this
  machine right now and why not, where its models come from, which effort words a mapping of it may hold and
  whether one of its runs may go out with an effort or without, whether a mapping of it carries a ceiling on
  the answer's tokens, whether agency chooses the login it goes out under, and how its own reply reports what
  one turn cost — so whoever wants one of those facts asks the provider, and no file outside those programs
  writes the providers' names out to answer it.
- `packages/agent/harnesses.ts` is the one file that names the providers this build carries: it composes them
  into the agent base's provider registry (`packages/agent/agent-providers.ts`), and every other file reaches
  a provider through that registry.
- Claude Code is carried by Claude Code, Codex, OpenRouter and DeepSeek by Claude Code pointed at agency's
  gateway under a member of their road (`packages/agent/member-provider.ts`), and the registry's by Claude Code
  pointed at the gateway under a registration, with a request that needs no tools asked of the gateway
  directly. The models of each member road are the members the deployment names, which the owner adds and
  changes with no commit (`docs/own-llm.md`). On every line their program in the agent's pane is Claude Code,
  and a headless run of any of them is the same program on the same gateway.
- Which provider runs a piece of work is that work key's row in the route table (`docs/own-llm.md`),
  so a program is picked in exactly one place.
- **What a route may promise is what the program really carries.** Each harness says what its
  program offers — a gate on every tool call, a tool set made of the MCP servers it is handed alone
  — and each line what it needs of its program: Claude Code offers both and so carries every line;
  Codex and dsh offer no gate on a tool call and carry a chat round, a job and a gate, each as a portable
  run the fallback runner hands over; dsh with every tool row of its own off also carries the search line,
  whose tools are the run's MCP servers alone, and the browse line stays Claude Code's.
- **A runner line's thrown-away session goes out on any road whose program carries the line.** A consumer
  that composes a runner's instructions itself starts a thrown-away session on the line with them
  (`docs/api.md`, 「sessions」) and names the provider and model: on Claude Code each turn is a run apart in a
  pane of its own; on a road whose program carries its own turns — a member road's, the registry's — each turn
  goes into a place of the session's own, composed as a worker's job on that road is (the member's or the
  registration's home, window, answer ceiling and wait), with the line's instructions as its system prompt,
  the line's MCP servers as its tools and nothing of the program's own, and the session token its place
  carries issued with the labels and the capabilities the session named. A provider whose program does not
  carry the line refuses the start.
- A work key names the one line it runs on, so a provider whose program does not carry that line, or
  that cannot be used here (no Codex executable, no gateway key, no OpenRouter key, no DeepSeek key file), is offered on
  the console as unavailable with the reason beside it and refused by the control API.
- The lines themselves are the owner's alone: a flag added, removed or reordered rewrites the prefix
  every warm session of that line stands on, so what an implementation may not do is move one.
## What is fixed

- **Every line on Claude Code is handed one whole system prompt** with `--system-prompt-file`, in the
  CLI's own place; none appends to the CLI's prompt. The chat session, the job worker, a companion's
  turn, and every turn on the search and browse runner lines each open theirs on the
  platform's harness opening, `prompts/harness.md` read out of the deploy tree, unless a base prompt
  stands in its place (`docs/base-prompts.md`); a gate's prompt is the gate's and the translator's is
  its role's and purpose's, each whole.
- **Chat session** (`chatArgs`): the model and effort the `bmo` work key's tier names, read out of
  agency's store at every launch. Neither is written in code and no constant stands behind either —
  a mapping the owner deleted, or a migration the owner has not settled, refuses the launch rather than
  being answered with a default (`docs/model-tiers.md`). The console is the one place they are
  chosen, and the line, the record of the session, the keepalive's cache policy and the check for
  a runtime downgrade all read that one answer. The system prompt is the harness, the role and the
  persona, each whole (`docs/agent-documents.md`, 「The chat agent」): the platform's harness opening,
  `prompts/harness.md`; then
  the chat role's documents with the tools paragraph composed for the program the session runs on, the
  three answer rulebooks, the rulebook against destroying the system
  (`prompts/rules/self-preservation.md`), the brain's discipline and the tool host's paragraph on finding
  and using tools (`docs/api.md`, 「Calls agency makes」); then the persona — the base layer of the
  agent's brain, or the one the turn hands in where the brain gives none (`docs/sessions.md`, 「Document
  reload」). On Claude Code the three are handed over as one file, the whole system prompt in the CLI's
  own place (`--system-prompt-file`); on dsh, whose own prompt is its harness, the role and the persona
  go as the persona suffix of its system prompt and the platform's opening is not sent. No
  settings sources, no skills, no Chrome, and the chat line's MCP servers, named in `chat.mcp.json`
  beside the strict flag that keeps every other server out: the tool host's namespaces the chat line
  names, `bmo`, `gate` and `eye`, each a streamable HTTP server behind the run's session
  token (the tool host's `mcp-host.md`), which is where the chat agent's own tools stand — sending, its
  brain's memory tools, and the gate's translator among them; the artifact and explorer services as
  first-class servers of the line, `artifact` and `explorer`, which publish a document and read a page;
  the job service's harness plugin, `job`, which leases an item
  from a queue, started on the chat's own caller token where the deployment names one (`BMO_JOB_CHAT_TOKEN`,
  or the file `BMO_JOB_CHAT_TOKEN_FILE` names), so the job service records what the chat does — an
  approval among it, which it keeps as the chat's recommendation rather than the owner's approval — under
  the chat's name, and on the deployment's shared token otherwise — either handed as the file it stands in and
  read by a shell in front of the plugin, which takes its token from its environment alone
  (`docs/agent-control.md`, 「A token is handed as the file it stands in」) — its own item creation left out of what it
  serves (`--without job_item_create`) and, on Claude Code, disallowed as well (`--disallowedTools`); and the agency's own, `agency`, started for the agent the configuration is
  composed for (`--agent`, one configuration a chat agent), which files an item through agency
  (`docs/api.md`, 「assignments and jobs」), assigns a worker and launches the item's job. A
  hosted tool those namespaces do not show is reached with `bmo use`. The chat line no longer names the
  tool host's `web`: its own `WebSearch` and `WebFetch` are denied, and the explorer server is the
  search and page reader of the line. The search and browse lines keep their `web` tools, since a search or
  a browse is the open web's work and they have no explorer server of their own.
  The tools shown are exactly Bash, Read, Write, Edit and the CLI's two MCP resource tools, the list
  and the read, in every run of every chat (none when the owner has turned tools off in that chat),
  with the line's servers' tools beside them; the permission mode is don't-ask, and the allow list is
  those same six and the whole of the line's servers by their prefixes in every run, whoever wrote the
  turn's messages. A line that names fewer servers than these leaves the chat agent with no tool of
  its own: a configuration that drops the tool host drops sending and memory with it. What is the owner's alone —
  approving, launching or finishing work, deploying, reading another room — is held by the
  session's own judgement under its rules, not by the allow list. The chat stands as the chat agent's
  resident program, whose line keeps its slash commands — `/compact` is one, typed into the program
  to fold the session in place, on the same prefix its turns stand on. There is no separate fold
  line for the chat. It works in a folder whose name a release does not change — the deploy tree's link
  on dsh, the agent's own home `companion-homes/<agent>` on Claude Code, which names its records after
  the folder's real path — so the program's own record of its session is taken up again after a restart
  or a deploy (docs/sessions.md, 「A restart hands the session over」). A turn that finds no pane to stand the program in, herdr down,
  fails as a transport failure and is run in no other shape.
- **Gate runs**, which the gate unit carries, each at medium effort with the system prompt the gate hands
  with its turn as the whole system prompt, handed over as a file named by its content; agency holds no
  gate's instructions, rulebooks or order of its own, and the rulebook against destroying the system is
  not among a gate's, since a gate holds no tool to act with. No
  tools at all, no settings but the pool slot's own status line and hooks, no MCP, no Chrome, and no
  session named: the program opens its own, and `/clear` opens the next after every judgement
  (`gates.md`, 「A gate on Claude Code」). No gate picks what a turn reads: the chat agent reads the
  brain's index in its own draft step and fetches what its answer needs with its tools (the gate's
  `reference-selection.md`).
- **Job worker**: the model, effort and tier the job's turn names in its mapping, which its runner
  reads off `job-worker`'s tier (`docs/model-tiers.md`) unless the job was launched with another
  model, and the tier's own where a turn names none, in
  bypass-permissions mode with WebSearch and WebFetch disallowed, and with them every tool that
  publishes a page outside the system's own archive — the CLI's Artifact tools and the claude.ai
  Docs connector — since a document a job shows the owner goes into that archive (`archive.md`, 「A
  document a job shows the owner」). The tools that bring a session back after its turn has ended are
  disallowed too — `Monitor`, `ScheduleWakeup` and the session cron tools `CronCreate`, `CronDelete`
  and `CronList` — because a job has no later: its turn's end is its answer, and a run those tools
  keep re-invoking never reaches its own ending, so nothing it wrote is recorded and the unit burns a
  model run on every re-invocation until somebody stops it. The claude.ai account's connectors — Gmail, Google Calendar,
  Google Drive, Claude Docs and whatever else the login has connected — are not loaded at all:
  the worker settings set the CLI's `disableClaudeAiConnectors` to `true`. To turn them back on,
  set that key to `false` in `prompts/agents/worker/settings.json` and let the commit go live; it
  moves the worker prefix like any change to the line (「Why a line is held steady」). The Claude
  Docs server stays on the disallowed list either way, so turning the connectors on brings back
  the others and never the one that publishes. The program runs with permissions bypassed, so the worker
  settings also set `skipDangerousModePermissionPrompt` to `true`: a run on the line in a pane of its own,
  which reads those settings as they stand rather than a place's, would otherwise stop at the CLI's
  warning dialog until a person answered it. The job worker runs in the directory its own worker
  row names — this
  repository for the built-in worker, since editing this code is what that one is for. No persona: its
  whole system prompt, given with `--system-prompt-file` in the CLI's own place, is the platform's
  harness opening, `prompts/harness.md`, then the worker's rules, as one file named by what it holds.
  The rules are one file the line composes for the worker whose
  run it is — the owner's standing rules for every job, `prompts/agents/worker/standing-rules.md`,
  then the rulebook against destroying the system, then that worker's own rules where it has a
  document of its own, `prompts/agents/<worker>/own-rules.md` (a support
  worker takes its base's, and a co-work branch in a standing checkout, which is no worker's, takes
  none), then the report trailer every job is held to, `prompts/agents/worker/trailer.md` filled in as
  the deployment shows it (a co-work branch takes none), then the documents its row names as its prefix
  documents (`docs/worker-bench.md`, 「Prefix documents」). The trailer stands in the whole prompt, the
  session's cached prefix, so a session pays for it once; the job's prompt carries no trailer, and of
  its item's notes only as many as fit the policy's bytes (`docs/api.md`, 「assignments and jobs」). A
  worker that chooses a base prompt (`docs/base-prompts.md`) is handed it in the harness opening's
  place: the chosen prompt with the
  same rules after it, as one file named by what it holds, given as the whole system prompt; on
  Codex as its instructions file, with the checkout's `AGENTS.md` still read, and on OpenRouter and
  DeepSeek as dsh's persona prefix. A run that goes out on a Claude account naming a base prompt of its own
  (`docs/cache-fallback.md`, 「The Claude Code road is a list of accounts」) takes the account's base in
  that place, over the worker's own choice and in place of the harness opening alike, with the same
  rules after it, as one file named by what it holds and given with `--system-prompt-file`; so a run
  on an endpoint account opens on the base its endpoint's harness runs on rather than on the
  platform's opening. The base is composed with the account the run goes out on, so the
  line's prefix is the account's as well as the worker's. Every co-worker
  definition's prompt, which stands as the co-worker's whole system prompt, opens on the platform's
  harness opening, then the role's own prompt, and closes on the rules every worker shares and the
  worker's own. A job carried by another provider whose program is a harness of its own is handed
  the rules file alone, in the place that program keeps beside its own default prompt, never in place of
  it, and the platform's opening is not sent: on Codex as the run's developer instructions (`developer_instructions`), with Codex's own
  coding-agent prompt and the checkout's `AGENTS.md` left as they are; on OpenRouter and DeepSeek as the
  persona suffix of dsh's system prompt, after dsh's own prompt whole, with its instruction-file row left
  as it is. A worker run handed no such file is
  refused on every provider rather than started without it. A worker the deployment gives rules of its own
  names them in that document (`docs/own-llm.md`, 「The own model and the engine behind it」). What
  the run is shown beyond the prompt comes from this repository and not from the machine: the
  machine's own CLAUDE.md, which every checkout under the home directory would walk up to, is named
  in the worker settings' `claudeMdExcludes`, the user scope this machine
  registers its own skills, Claude Code subagent definitions and MCP servers in is shut off on the
  line, and in its place the line hands over the skills as a plugin directory — the skills
  repository's `worker` bundle, `prompts/agents/worker/plugin/` of the release, which every deployment
  places afresh with that repository's own placer (`bmo-skills place --bundle worker`) from the skills
  checkout that stands beside agency's in its workspace, or the one `BMO_SKILLS_DIR` names, before the
  deploy tree flips, so a skill the skills repository gains reaches the next job after the next
  deployment; the copy a checkout of this repository carries stands for a tree no deployment placed — the five co-worker
  definitions as JSON with the model and the effort each type runs at, as its role's mapping gives them
  on the provider the worker's program stands on where it lends co-workers and on Claude Code otherwise,
  and the plugin skills a
  type is started with, the settings the worker is
  held to — which is also where the Bash tool's wait for a job's command is set, its default and its
  most both the largest the program takes, so a command is waited on until it ends, and where the hook
  stands that reads every shell command a job runs
  before it runs and refuses one that would open a Claude Code session or write into the CLI's
  session records until the job has said what for with `bmo-agency live-claude --purpose`,
  which asks nobody and keeps a count of every run it lets through for the item's report
  (`docs/live-claude-guard.md`), and where the hooks
  stand that hold a native Claude Code subagent call until the common co-worker pool gives it a place and
  give the place back when the child ends (`docs/coworker-pool.md`); a job about to open several native
  co-workers of one tier holds that many places first with
  `bmo-agency coworker pool reserve --role <role> --count <n>`, so its calls take them without waiting,
  and gives them back with `bmo-agency coworker pool release` when done — they go back by themselves
  once its CLI ends —
  and two namespaces of the tool host in one configuration: `web`, which every agent that may use
  the web is given, and `worker`, which serves the graph's reading verbs and the queue's two as
  tools (`docs/worker-mcp.md`), shown to it by the worker role's bundles (`docs/agent-roles.md`). No
  other line is handed that file and no other role gives the worker's own bundles, so what is added to
  them reaches job workers and nothing else. So exactly one copy of each reaches a job, a checkout on another
  machine runs the same job, and changing any of it is a commit here rather than an edit to the
  machine. The deploy unit's `deploy.md` says what the whole of it adds up to and what to do when it moves.
  Every one of those files — the settings and their hooks, the plugin, the rules and the co-worker
  definitions — is read out of the release the job was launched on rather than through the deploy
  tree's link: the job carries that release in `BMO_DEPLOY_DIR` (`docs/agent-control.md`, 「The
  job's release」), so a deployment during the job changes none of them under it.
  The skills the CLI bundles itself stand beside the plugin's, and the worker settings'
  `skillOverrides` turn off those a job has no use for, so they are not listed to it: the three
  Artifact skills, since the Artifact tools are disallowed; `keybindings-help`, `update-config`
  and `fewer-permission-prompts`, which edit the CLI's own configuration that a job leaves alone
  and a line in bypass mode never asks about; `loop` and `schedule`, since a job has no later; and
  `init`, since the checkout's CLAUDE.md is the owner's. To bring one back, remove its entry and let
  the commit go live.
  The job's run is a turn of the worker's resident program, the interactive CLI standing in the
  one pane of that worker's own workspace (`docs/agent-control.md`): the program is started with
  no prompt on the session named by `--session-id`, started again on `--resume` of that session
  whenever it has to be once that session has a record, told each job's work over herdr's socket
  once it is idle, and read back
  out of the record the CLI keeps for that session — so a word added to the item reaches it while
  it works, and the owner can watch it in herdr or type into it. The pane is neither opened nor
  closed for a job. A worker with no job stands the same program with nothing put in, so its first
  request can be typed in herdr, and a session a person opens there becomes the one its next job on
  that model resumes. One session is kept per model and resumed, by the job and by the keepalive
  alike, and the keepalive is a turn of the same program; the resume adds nothing to the cached
  prefix, so the line itself is the same line. A keepalive that brings a session up goes out on
  the login a job's walk offers now, named on its line, so the job resuming that session finds the
  program the keepalive left standing. A model or a login changing is a command changing,
  and the program is started again on the new one in the same pane, between turns: a program whose own
  record says a turn of it is working is never ended for a changed command, and the next stand after
  that turn's end replaces it (`docs/sessions.md`, 「A restart hands the session over」). A session whose
  context has filled is folded by `/compact` typed into that same program, so the fold stands on
  the prefix the jobs stand on. Each job's turn goes into the program under the run name its runner
  gives it, carried as `BMO_RUN_NAME` beside `BMO_TURN_AGENT`, the worker, and `BMO_TASK_LOCKED=1`
  where the turn has a job, so the verbs the job runs find their job; the variables the turn names
  stand beside them, and the turn's title is what the pane shows over it (`docs/api.md`, 「Sessions」,
  a work turn's parts). A co-work branch is a
  turn of the resident program of the standing checkout it works in (`docs/coworker-pool.md`,
  「Standing branch checkouts and their list」), on a thrown-away session of its own that it keeps
  nowhere; a branch outside every standing checkout, and a thrown-away session handed a runner line's
  instructions — which goes out on that runner line rather than this one (`docs/api.md`,
  「sessions」) — go into a pane of their own opened for the turn. There
  may be more than one worker: the owner adds them, names them and points each at a checkout of
  its own, and a worker is that row and nothing more — the line above is the same for all of
  them, and what differs is the directory the run works in, the session it resumes (one per
  worker per model) and the lock it takes (one per directory, so two jobs never share a checkout
  and two checkouts never wait on each other). No
  clock ends any of these sessions: a warm agent stays warm, and what a session is lost to is
  the owner's word, a disaster, or a Claude API error — never the passing of time.
- **A job worker on a registered model**: Claude Code pointed at agency's gateway under the
  registration's name (`docs/own-llm.md`, 「How requests go」), on the job worker line above whole —
  the platform's harness opening with the worker's rules after it, or its base prompt with them after
  it, the plugin, the co-workers, the worker's MCP
  server, the project and local setting sources and every tool the line shows — and the settings the
  place writes over the worker's own. What differs is the registration's gateway name, with its node
  where it has several, at `medium` effort with thinking on and `low` with it off — thinking as the
  worker's own setting names it, else as the tier mapping does, else as the registration's default —
  and its endpoint:
  the gateway's address, the registration's context window, the answer ceiling and the request wait,
  each read from the registry as the program is started. Its records and first-run answers stand
  under a home of the registration's own, and the gateway's key is read from its file by the
  settings' key helper. Each job is a turn of that program in the worker's pane on a session of its
  own (`docs/own-llm.md`, 「A worker on a registered model is composed as on Claude」).
- **A job worker on a DeepSeek, OpenRouter or Codex model**: Claude Code on the gateway under the member the
  pin or the worker key's tier names, exactly as a worker on a registration stands (`docs/own-llm.md`): the
  worker line whole — the platform's harness opening or its base prompt with its rules after it, its
  plugin, its co-workers, the worker MCP server, its settings and hooks — with the member's home, window,
  answer ceiling and request wait, and the gateway's key read from its file by the settings' key helper. The
  member's effort word goes out in the effort header; no login of the Claude road goes with it.
- **A companion's turn** (agency's server's companion turn): a turn of the companion's resident program,
  which stands on the chat line above as the chat agent's does (`docs/agents.md`, 「Conversational agents」) —
  the same tools shown and allowed, the same configuration but for agency's own plugin, which a
  companion is not handed, so it files no item and launches no job and tells the chat agent
  in the consult room what needs hands, the same mode and the same shell guard —
  with the system prompt its own settings compose (`docs/companions.md`, 「What a turn is given」) in
  place of the chat's — one whole prompt given with `--system-prompt-file`, opening on the base prompt it
  chooses or else on the platform's harness opening — its own folder, the session of the room the turn is for, resumed or opened under
  the id the turn names, and its program's own commands reachable. The room, the companion, the run,
  the messages the turn answers and whether each is the owner's stand in the place's run as its own
  variables. On a registered model it goes out as every run on one does, the thinking its agent row
  names or else the registration's, with its generation settings as the run's sampling.
- **Search runner**: the line a search goes out on, carried by agency and composed by whoever starts a
  thrown-away session on it (`docs/api.md`, 「sessions」) — the research unit for its searches, their rounds
  and their branches. Its instructions are handed in whole by that caller; agency composes none and asks
  nothing of a run after its answer. On Claude Code: the model the session names and the effort and tier the
  turn's mapping names, don't-ask mode, no built-in tool shown, the web MCP server's tools and the brain's
  `bmo` namespace, and nothing else allowed; any other hosted tool it holds is reached with `bmo use`; no
  settings sources, no other MCP, no Chrome, run in its deployed directory unless the session names a
  folder. It is handed the standing `search.mcp.json`, and the run's session record carries the labels and
  keeps the capabilities the session's caller named, so the web tools read what that caller told them; the
  record also names the chat agent — the agent whose row answers every room that names nobody — as the turn
  its brain tools run for, so a fact the run establishes is written into the chat agent's brain while the run
  itself stays the runner's. The searcher role's bundles show it bmo-explorer's `research` tool, which opens
  a branch through the research unit, and the research unit's `rulebook` reader where the record holds
  `web.rulebooks`; the line's `eye` namespace shows it `eye/see`, which looks at a picture by its address and
  answers a question about it. No native Claude Code subagent is opened by any run on it.
  - **Its system prompt is the platform's harness opening and then the instructions handed in**, as one
    file replacing the CLI's own, written under a name its text gives, so two sessions handed the same
    instructions read one prefix. What differs per run is the prompt. So nothing of Claude Code's coding
    conventions reaches a search.
  - **What a run establishes is written into the shared brain.** The search line holds
    `brain_learn` and `brain_similar` on the chat agent's brain: a fact or a piece of knowledge
    a run establishes is written there with the source it was read from, after the brain's similar rows
    are read, and the brain's write judgement confirms a duplicate, strengthens a version of a row and
    closes what a row contradicts.
- **Translate runner**: one translation asked through agency's `POST /translation-runs` (`docs/api.md`,
  「translations」), on the road the road resolution answers for the work key `translate`, whose profile
  starts on the registry's provider, so the registered translation model answers first and the walk
  after. Its system prompt is the translator role's prompt as the store holds it (`docs/agent-roles.md`)
  and then the
  instructions of the purpose asked — `prompts/agents/translator/instructions.md` for a document's part,
  `worker.md` for a text a job worker reads, `record.md` for a worker's record the owner reads, that
  one after the plain-writing rulebook, and the caller's note on the part after them — and its input is
  one line naming the target language and then the text. Where the caller names terms (`docs/api.md`,
  「translations」), the input is instead the translation model's own terminology form, which a
  translation model follows more closely than an instruction in the system prompt: a line saying that
  reference translations follow, one line for each term, its source and then its target, a blank line,
  the instruction to translate the text into the target language and answer with the translation alone,
  a blank line, and the text. On the registry's provider it is one request to the gateway's chat completions path under
  the registration the mapping names, with thinking off and the profile's sampling (`docs/own-llm.md`,
  「A request with no tools」), with no wait of agency's own and naming none to the gateway, so a translation
  on a registration naming a card service is waited for however long the translator takes, its parts
  queued at the engine included, and one on a registration of addresses waits that entry's own wait
  (`docs/own-llm.md`, 「The gateway」); where the registration names a card service, the request names
  the entry of the card the translations out on it at once share, waited on however long its grant takes
  and given back once the last of them has ended — answered, failed, or let go by its caller — whether its
  demand was granted yet or not (`docs/own-llm.md`, 「An engine the arbiter stands」). Such a registration is the
  registry's road whether or not its engine stands when the translation is asked: the demand starts it,
  the request waits until the gateway serves the granted card's entry, and a translation never moves to
  Claude Code because no instance stood. A demand the master refuses, or a request the engine fails, fails the translation, since a key set to the registry's
  provider does not walk on a failure (`docs/own-llm.md`, 「When a registered model does not answer」). On
  Claude Code it is a run apart (`docs/agents.md`, 「Runs apart」): that system prompt as
  the whole of it, no tool shown or allowed, no MCP server, a fresh session, thinking off. No other
  provider carries the line. The answer is the translation whole.
- **Browse runner**: the line a browse goes out on, carried by agency and composed, as the search line is, by
  whoever starts a thrown-away session on it: the model the session names and the effort its turn's mapping
  names, don't-ask mode, no built-in tool shown, the chrome-devtools tools, the tool host's web tools and the
  brain's `bmo` namespace from the line's MCP configuration — chrome-devtools started headless, apart from a
  person's session, its D-Bus and its keyring, the web namespace as every line names it, the run's own values
  in its session record — and nothing else, no settings, no other MCP, no Chrome integration, run in its
  deployed directory. Its system prompt is the platform's harness opening, `prompts/harness.md`, then the
  instructions its caller handed in, as one file replacing the CLI's own; the task and the time the run has
  are the prompt. chrome-devtools is started without its performance and memory tools — tracing a page's
  load and taking a heap snapshot — since a run that reads what a page says has no use for them. Claude Code
  alone carries the line.
- **The web**: no agent of the system's is shown the CLI's WebSearch or WebFetch. Those hand back a
  model's rendering of a page, and a page can steer that model. The web reaches every agent
  through the tool host's `web` namespace (`mcp-host.md`):
  `web_search` asking the search engines directly (`web-access.md`), `web_fetch` returning
  the page's own text, a GitHub file page as the raw file. The runners get it from a configuration
  of their own — the search runner's standing `search.mcp.json` and the browse runner's `.mcp.json`
  beside its chrome-devtools; the job worker has the two built-ins disallowed on its line and is handed
  `worker.mcp.json`, which carries that same server beside the worker's own and which no other
  line is given, since the scope this machine registers the server in is shut off for it.
- **`bmo use`**: a hosted tool a line holds and its namespaces do not list is reached with the `bmo use`
  command, under the run's own tool session token, as the tool host's paragraph in the session's prompt says
  (`toolbook.md`). No line's MCP configuration names the tool host's toolbook. The site tools are the
  searcher's alone; the job worker and a companion's hands keep web search and page fetch.
- **Environment**: every one of these processes is started with the repository's git status
  left out of its prompt; every Claude line also carries the configuration directory of the
  account its run was put on, and that account's token where the account is a token account,
  or its endpoint's base URL, key and model ids where it is an endpoint account
  (`docs/cache-fallback.md`, 「The Claude Code road is a list of accounts」), so what a run goes out
  under is a login agency chose rather than whatever the machine happens to hold; a reference gate's run also carries `MAX_THINKING_TOKENS=0`; no line sets the share of
  its model's window a session compacts at, so every program, the job worker's included, compacts
  at its own default (`docs/sessions.md`, 「Session compaction」), and only a run on a registered
  model's road carries `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE`, at the share its registration leaves for
  the answer (`docs/own-llm.md`, 「A turn at the input limit」); nothing else is added beyond the
  switches below, and the store's address is never carried: it names agency's own store, not the
  archive a program reads from the data root's setting (`docs/store.md`, 「Connecting」).
  An account's variables are handed to a line by name rather than inherited, because the job
  line rebuilds its environment from a few names and would otherwise drop them. They add
  nothing to a cached prefix — the flags, their order, the system prompt and the files it
  points at are untouched — but a run moved to another account stands on that account's own
  cache and warms it from where that account left it.
- **The interactive program's background calls are switched off on every line.** The resident
  program — the chat's and the job worker's alike — would otherwise spend calls of its own that no
  agent of the system's reads, each one a fork of the session reading its whole context out of the cache and
  never written into the session's record, so the record and the status line's running cost part:
  - `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=0` — the next-prompt suggestion drawn in the input box. It
    runs after every turn whose last call wrote under ten thousand tokens, while the account is
    under its usage warning; on a chat carrying hundreds of thousands of tokens it is a
    whole-context read after most turns, and it is the whole of the gap between the chat's
    status-line cost and its session record (measured).
  - `CLAUDE_CODE_ENABLE_AWAY_SUMMARY=0` — the recap drawn when a person comes back to the pane after
    five minutes away: the same whole-context read, spent on a pane the owner only watches.
  - Both are part of the variables every line sets, beside the git-status one. Those variables are Claude Code's own
    declaration in agency's policy (`policy/providers/`), which every side that starts one of
    its lines reads. To turn one back on, set its value to `1` there and let the commit go live.
- **The CLI's own memory is switched off on every line.** `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` is
  among those variables. Left on, the CLI's prompt tells every run that keeps it — the job worker
  among them — about a memory directory of its own under the login's records and how to write
  it: some seven hundred tokens in a worker's prefix, and more where the file tools make it offer
  writing as well. No agent of the system's keeps anything there: what a job learns goes into its
  checkout's documents (the `bmo-docs-conventions` skill), and an agent's long-term memory is its brain.
  To turn it back on, remove the variable and let the commit go live;
  it moves the worker's prefix like any change to the line. A variable adds nothing to a cached prefix,
    but it is part of the command a pane's program was started with, so the change starts each
    resident program again on `--resume` of its own session at its next turn.

## The owner's gate

A chat agent holds Bash and reads every word said in its rooms, so what a room's words may make it do is not
left to its prompt. In front of each action that touches the machine or spends money, the code that carries
the action out asks who sent the words the turn answers.

- **The actions gated:** asking a worker through `bmo/worker_ask`, which starts the worker's session on the
  words; handing a task off through `bmo/delegate`, which takes a co-worker seat and puts the words into the
  helper's session; and `run_job`, the chat agent's own `agency` server launching an item's job on its worker. Each
  asks the room relation `launch`.
- **The turn asked about** is the one the agent's place carries now: the room it answers (`BMO_TURN_CHAT`), the
  messages it answers (`BMO_TURN_MESSAGES`) and whether the receiver marked every new message in it the
  owner's (`BMO_TURN_PRIVILEGED`), as the run the door put in names them.
- **The owner's own words are never asked about.** A turn the receiver marked the owner's goes on at once; so
  does a turn no room message opened — words the owner typed into the program, a job, a schedule — since no
  sender stands behind it but the deployment's own.
- **Anybody else's are checked, every message.** Each message the turn answers is handed to the account
  service's `POST /checks/message` with the room and the relation, and the action goes on only where every one
  is answered allowed: the platform administrator, the owner, passes every check, and anybody else passes only
  on a grant of that relation the owner made in that room.
- **A check that cannot be made refuses.** No account service named, one not reached, an answer that is not a
  success, a turn that names no message: the action is refused, never let through.
- **The refusal is a sentence.** The tool answers with a failure saying what was refused and why — whose
  message, which relation, or why the check could not be made — and nothing is started.
- **Where the account service is.** `WEB_ADDRESS` names its base address and `WEB_TOKEN`, or the file
  `WEB_TOKEN_FILE` names, its token; a process whose environment names neither reads them from the web unit's
  `env/web.env` under the data root, as the receiver's room tools do.
- **What stands outside agency.** A deploy, `testbed/deploy`, is the testbed's hosted tool and a capability
  `bmo use` reaches is the tool host's; each takes the same check with its own relation (`deploy` for a
  deploy) in the unit that serves it, and agency hands it nothing it could gate there. An item the chat agent
  files is not gated: one filed on a turn not the owner's stands a draft that only the owner's approval opens
  (`docs/api.md`, 「assignments and jobs」), so nothing runs on it before he says so.
- **What the gate does not close.** The agent's shell reaches agency's control port with the token file it is
  handed, so a model turned against its rooms can still call a route itself; the gate stands in front of the
  tools the chat agent is given to act with, not in front of the port.

## Why a line is held steady

- The prompt cache matches a byte-identical prefix, and the tools shown, the system prompt and the
  machine configuration are that prefix.
- A change to any of them rewrites every affected session's context and, in a chat, costs that chat
  its warm cache.
- So a line changes when the work it carries needs it to, all of the change in one commit, and a
  line that keeps a session is changed once rather than over several commits, each of which would
  cost every session on it a cold start of its own.
