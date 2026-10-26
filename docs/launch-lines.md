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
  runner's, a research branch's — runs each run apart: the same interactive program in a
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

- **Claude Code is one implementation of it** (`packages/agent/claude-code.ts`), and every flag the
  CLI is spoken to with is written in that one file — the five lines (a chat round, a gate's
  judgement, a job, a search, a browse) and the two folds are its answers, each
  in its fixed order.
- **dsh — the DeepSeek Harness — is the second** (`packages/agent/dsh.ts`): it takes no flags to
  speak of, so a run becomes a patch overlay written fresh for it, naming rows of dsh's own
  composition — the system prompt as the persona prefix, the model and the provider's API it runs on as
  a route, the tools it may have — and the command is the pinned `@deepseek-ai/dsh` at its headless profile with
  the task standing in the line. The task stands there as words headless joins back with one space,
  each short of the kernel's limit on one argument, and it reaches dsh out of a file of its own
  rather than through the string npx hands a shell, because a task can run to hundreds of kilobytes
  and that string is refused whole long before then.
- **dsh reads every `{{name}}` in a persona as a variable of its own** and refuses the whole prompt on a
  name it does not hold, so each `{{` of the text agency hands it is broken by a word joiner, which the
  model reads past; the headless opening line's own `{{model}}` is left for dsh to fill.
- **What a run may do to the files around it is not one of those rows alone**: dsh keeps how far a
  write may reach and whether the run stops to ask as two knobs, and names only the pairs they make
  together, so a run that set the reach and left the asking behind would be a composition dsh
  refuses outright — before a word goes anywhere.
- The mode therefore travels in the run's environment as the one name dsh resolves both ends of, as
  well as on the row that states it.
- **Codex is the third** (`packages/agent/codex.ts`): `codex exec` on the machine's own login, its
  JSON events read back into the same reply, a thread id kept per scope, model and effort.
- **A harness after those three is one more file.** What it has to write is the whole of the
  interface: a name to be registered under, the lines it can really carry, the word its own program
  folds a session with, a command for each of those lines, a reader, and — where its program has
  them — the session one of its streamed events names, what those events say about the children a
  run opened, and the tail of the record it kept for a session it keeps.
- What it does not write is everything above it — it composes no documents, decides no policy about
  who may use which tool, records no usage, judges nothing about the account, and never reads the
  route that chose it.
- **What a harness cannot answer it says so about rather than pretending**: dsh's own prompt
  preamble stands in front of whatever prompt it is given, its tools are its own and can be had
  whole or not at all rather than by name, it keeps no session agency could resume, and it reports
  nothing about what a run cost — so a reply off it names no session and carries no figures, and
  nothing downstream is told a number that was never reported.

### Providers and what a route may promise

- **The owner chooses a provider, never a harness.** The providers are Claude Code, Codex,
  OpenRouter, the registry's and DeepSeek.
- Each is declared beside the program that carries it and answers for itself whether it can be used
  on this machine right now and why not, where its models come from, which effort words a mapping of
  it may hold and whether one of its runs may go out with an effort or without, whether a mapping of
  it carries a ceiling on the answer's tokens, whether agency chooses the login it goes out under, and
  how its own reply reports what one turn cost — so whoever wants one of those facts asks the
  provider, and no file outside those programs writes the providers' names out to answer it.
- `packages/agent/harnesses.ts` is the one file that names the providers this build carries: it composes
  them into the agent base's provider registry (`packages/agent/agent-providers.ts`), and every
  other file reaches a provider through that registry.
- Claude Code is carried by Claude Code, Codex by Codex, OpenRouter and DeepSeek by dsh, and the
  registry's by Claude Code pointed at agency's gateway, with a request that needs no tools asked of the
  gateway directly; the models it offers are the registrations, which the owner adds and changes with no
  commit (`docs/own-llm.md`). OpenRouter and DeepSeek differ only in dsh's one route: OpenRouter's
  gateway with its key in the process's environment, or DeepSeek's own OpenAI-compatible API with the
  road's key file handed to the program as the variable the route reads. On every line their program in
  the agent's pane is bmo-tui on a dsh session, which each provider's declaration names as its resident
  program, and a headless run of either is dsh's headless profile on the same route.
- Which provider runs a piece of work is that work key's row in the route table (`docs/own-llm.md`),
  so a program is picked in exactly one place.
- **What a route may promise is what the program really carries.** Each harness says what its
  program offers — a gate on every tool call, a tool set made of the MCP servers it is handed alone
  — and each line what it needs of its program: Claude Code offers both and so carries every line;
  Codex and dsh offer neither and carry a chat round, a job and a gate, each as a
  portable run the fallback runner hands over, and not the search and browse runners, which need
  both.
- A work key names the one line it runs on, so a provider whose program does not carry that line, or
  that cannot be used here (no Codex executable, no gateway key, no OpenRouter key, no DeepSeek key file), is offered on
  the console as unavailable with the reason beside it and refused by the control API.
- The search and browse runners have no work key and run on Claude Code alone.
- The lines themselves are the owner's alone: a flag added, removed or reordered rewrites the prefix
  every warm session of that line stands on, so what an implementation may not do is move one.
## What is fixed

- **Chat session** (`chatArgs`): the model and effort the `bmo` work key's tier names, read out of
  agency's store at every launch. Neither is written in code and no constant stands behind either —
  a mapping the owner deleted, or a migration the owner has not settled, refuses the launch rather than
  being answered with a default (`docs/model-tiers.md`). The console is the one place they are
  chosen, and the line, the record of the session, the keepalive's cache policy and the check for
  a runtime downgrade all read that one answer. The system prompt is the chat agent's
  documents with the tools paragraph composed for the program the session runs on
  (`docs/agent-documents.md`, 「The chat agent」), the three answer rulebooks, the rulebook against destroying the system
  (`prompts/rules/self-preservation.md`), the brain's discipline, the tool host's paragraph on finding
  and using tools (`docs/api.md`, 「Calls agency makes」) and the persona — the base layer of the
  agent's brain, or the one the turn hands in where the brain gives none (`docs/sessions.md`, 「Document reload」) — handed
  over as a file and replacing the CLI's own. No settings sources, no skills, no Chrome, and five MCP servers, named in `chat.mcp.json`
  beside the strict flag that keeps every other server out: the tool host's namespaces the chat line
  names, `bmo`, `web` and `gate`, each a streamable HTTP server behind the run's session
  token (the tool host's `mcp-host.md`), which is where the chat agent's own tools stand — sending, filing
  a TODO item, its brain's memory tools, the web's search and page fetch, and the gate's translator among
  them; the job service's harness plugin, `job`, which leases an item
  from a queue, started on the chat's own caller token where the deployment names one (`BMO_JOB_CHAT_TOKEN`,
  or the file `BMO_JOB_CHAT_TOKEN_FILE` names), so the job service records what the chat does — an
  approval among it, which it keeps as the chat's recommendation rather than the owner's approval — under
  the chat's name, and on the deployment's shared token otherwise; and the agency's own, `agency`, which
  assigns a worker, launches the item's job and sends a research out. A
  hosted tool those namespaces do not show is reached with `bmo use`.
  The tools shown are exactly Bash, Read, Write, Edit and the CLI's two MCP resource tools, the list
  and the read, in every run of every chat (none when the owner has turned tools off in that chat),
  with the four servers' tools beside them; the permission mode is don't-ask, and the allow list is
  those same six and the whole of the four servers by their prefixes in every run, whoever wrote the
  turn's messages. A line that names fewer servers than these leaves the chat agent with no tool of
  its own: a configuration that drops the tool host drops sending and memory with it. What is the owner's alone —
  approving, launching or finishing work, deploying, reading another room — is held by the
  session's own judgement under its rules, not by the allow list. The chat stands as the chat agent's
  resident program, whose line keeps its slash commands — `/compact` is one, typed into the program
  to fold the session in place, on the same prefix its turns stand on. There is no separate fold
  line for the chat. It works in the deploy tree's link, whose path stays the same across releases,
  so the program's own record of its session is taken up again after a restart (docs/sessions.md, 「A
  restart hands the session over」). A turn that finds no pane to stand the program in, herdr down,
  fails as a transport failure and is run in no other shape.
- **Gate runs**, which the gate unit carries: the output gate, whose judgements keep a fixed shape in
  and out; the translator, for the gate's own translations route, until that route hands each part to
  agency's translate runner (「Translate runner」 below); and the sanitiser, which is shelved and asked by
  no turn. Each at medium effort, with its own
  instructions and rulebooks as the whole system prompt, handed over as a file named by its content; the
  rulebook against destroying the system is not among them, since a gate holds no tool to act with. No
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
  the others and never the one that publishes. The job worker runs on the CLI's default prompt,
  in the directory its own worker row names — this
  repository for the built-in worker, since editing this code is what that one is for. No persona and no system prompt
  of its own: what is appended to the default prompt is one file the line composes for the worker whose
  run it is — the owner's standing rules for every job, `prompts/agents/worker/standing-rules.md`,
  then the rulebook against destroying the system, then that worker's own rules where it has a
  document of its own, `prompts/agents/<worker>/own-rules.md` (a support
  worker takes its base's, and a co-work branch in a standing checkout, which is no worker's, takes
  none), then the report trailer every job is held to, `prompts/agents/worker/trailer.md` filled in as
  the deployment shows it (a co-work branch takes none), then the documents its row names as its prefix
  documents (`docs/worker-bench.md`, 「Prefix documents」). The trailer stands in this file, the
  session's cached prefix, so a session pays for it once; the job's prompt carries no trailer, and of
  its item's notes only as many as fit the policy's bytes (`docs/api.md`, 「assignments and jobs」). A
  worker that chooses a base prompt (`docs/base-prompts.md`) is handed the default prompt's place instead: the chosen prompt with the
  same rules after it, as one file named by what it holds, given as the whole system prompt; on
  Codex as its instructions file, with the checkout's `AGENTS.md` still read, and on OpenRouter and
  DeepSeek as dsh's persona prefix. A run that goes out on a Claude account naming a base prompt of its own
  (`docs/cache-fallback.md`, 「The Claude Code road is a list of accounts」) takes the account's base in
  that place, over the worker's own choice and in place of the default prompt alike, with the same
  rules after it, as one file named by what it holds and given with `--system-prompt-file`; so a run
  on an endpoint account opens on the base its endpoint's harness runs on rather than on a prompt
  written for Anthropic's models. The base is composed with the account the run goes out on, so the
  line's prefix is the account's as well as the worker's. Every co-worker
  definition's prompt closes on the same file's text. A job carried by another provider is handed
  the same file in the place that program keeps beside its own default prompt, never in place of
  it: on Codex as the run's developer instructions (`developer_instructions`), with Codex's own
  coding-agent prompt and the checkout's `AGENTS.md` left as they are; on OpenRouter and DeepSeek as the
  persona prefix of dsh's system prompt, after the headless profile's own opening line, with dsh's other
  sections and its instruction-file row left as they are. A worker run handed no such file is
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
  definitions as JSON with the model and the effort each type runs at and the plugin skills a
  type is started with, the settings the worker is
  held to — which is also where the Bash tool's wait for a job's command is set, a minute by
  default and ten at most, and where the hook stands that reads every shell command a job runs
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
  nowhere; a branch outside every standing checkout, and a research run, goes into a pane of its own
  opened for the turn. There
  may be more than one worker: the owner adds them, names them and points each at a checkout of
  its own, and a worker is that row and nothing more — the line above is the same for all of
  them, and what differs is the directory the run works in, the session it resumes (one per
  worker per model) and the lock it takes (one per directory, so two jobs never share a checkout
  and two checkouts never wait on each other). No
  clock ends any of these sessions: a warm agent stays warm, and what a session is lost to is
  the owner's word, a disaster, or a Claude API error — never the passing of time.
- **A job worker on a registered model**: Claude Code pointed at agency's gateway under the
  registration's name (`docs/own-llm.md`, 「How requests go」), on the job worker line above whole —
  the CLI's own prompt with the worker's rules appended, or its base prompt with them after it, the
  plugin, the co-workers, the worker's MCP
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
- **A job worker on a DeepSeek model**: bmo-tui on a dsh session in the worker's pane, as a worker on
  OpenRouter stands (`docs/agent-control.md`, 「dsh's providers: OpenRouter and DeepSeek」): the worker's
  rules after the headless profile's opening line as dsh's persona prefix, every tool row but the web,
  the deployment's MCP servers, and DeepSeek's own API as dsh's one route on the model the pin or the
  worker key's tier names, thinking off where the run does not think. The road's key file is read as the
  program is stood and handed to it in the variable the route names; no login of the Claude road goes
  with it.
- **A companion's turn** (agency's server's companion turn): a turn of the companion's resident program,
  which stands on the chat line above as the chat agent's does (`docs/agents.md`, 「Conversational agents」) —
  the same tools shown and allowed, the same configuration, the same mode and the same shell guard —
  with the system prompt its own settings compose (`docs/companions.md`, 「What a turn is given」) in
  place of the chat's, its own folder, the session of the room the turn is for, resumed or opened under
  the id the turn names, and its program's own commands reachable. The room, the companion, the run,
  the messages the turn answers and whether each is the owner's stand in the place's run as its own
  variables. On a registered model it goes out as every run on one does, the thinking its agent row
  names or else the registration's, with its generation settings as the run's sampling.
- **Search runner**: the model and the effort its depth's tier assignment names on the road the road
  resolution answers for the depth's key, the first provider of its walk whose program carries the search
  line and that can answer now (`research-runs.md`, 「The road」) — Frontier at every depth to start with —
  refused rather than launched where that model does not list the effort
  (`research-runs.md`) — don't-ask mode, no built-in tool shown, the web MCP server's tools and the brain's
  `bmo` namespace, and nothing else allowed; any other hosted tool it holds is reached with `bmo use`; no
  settings sources, no other MCP, no Chrome, run in its deployed
  directory. It is handed the standing `search.mcp.json`, and the run's session record holds when
  research closes, so the web tools know it; the record also names the deployment's agent as the turn its
  brain tools run for, so a fact the run establishes is written into the chat agent's brain while the run
  itself stays the runner's. The searcher role's bundles show it the `research_branch`
  tool, which opens a branch only after taking a place in the common co-worker pool, and the
  `view_image` tool, which shows the run a picture by its address.
  A branch is a run apart on the same line, which the web server opens through
  its own branch verb, with the model and effort of the profile's pooled role, the web tools and
  the picture viewer only, and the parent's research close; no native Claude Code subagent is opened
  by any of them.
  - **Its system prompt is the search's standing instructions**, handed over as a file and
    replacing the CLI's own: the search task, the site tools' catalogue, the search rulebooks and
    the rulebook against destroying the system. The catalogue is the web namespace's listing, which
    a process that does not serve the web tools itself reads from the tool host (`mcp-host.md`,
    「What a namespace says of itself」), and a line whose listing cannot be had is not composed at all.
    What differs per run — the question, who asked and when, the options, the depth and the branch
    allowance — is the prompt. So nothing of Claude Code's coding conventions reaches a search,
    and the prefix a ping on this line reads is the prefix a search reads.
  - **A branch's system prompt is the branch instructions** with the same catalogue and the rulebook
    against destroying the system; its angle and
    when its research closes are its prompt, so every branch shares one prefix.
  - **A research run's rounds.** A run on a research line keeps no receiver, so agency asks the
    line's questions itself after the run's first answer (`docs/api.md`, 「research drafts」): each
    question goes to a session of the rounds' own, opened fresh by the first round on the same program
    and road, as one more turn of it, carrying the task it was given and the answer written last, and the
    last answer is the run's result. The search session is let go after its first answer, so no round
    reads again the pages and branches it fetched; the rounds' session holds the rounds' own words. Its
    record withholds the branches, the image viewer and the rulebook reader, so a round that checks a
    source holds the reading tools alone. The policy holds the questions and each research
    key's list in `policy/research.toml` — what the task must establish, the sources the answer needs,
    whether every fact was checked against a second source, whether the answer covers the question,
    whether the facts it rests on were established, and finally the answer itself. The list is the key's,
    since the depth decides how hard an answer is checked: `search:basic` asks only the final question
    after its first answer, `search:thorough`, `search:exhaustive` and `browse` ask every question in
    turn. The console changes a key's list or a question's text through the agency's routes.
  - **What a run establishes is written into the shared brain.** The search line holds
    `brain_learn` and `brain_similar` on the deployment's agent's brain: a fact or a piece of knowledge
    a run establishes is written there with the source it was read from, after the brain's similar rows
    are read, and the brain's write judgement confirms a duplicate, strengthens a version of a row and
    closes what a row contradicts. The question before the final one asks whether every fact the answer
    rests on was written.
  - **The morning news page** runs on this line with its own instructions as the system prompt and
    the day it is written for as the prompt (`news-brief.md`).
- **Translate runner**: one translation asked through agency's `POST /translation-runs` (`docs/api.md`,
  「translations」), on the road the road resolution answers for the work key `translate`, whose profile
  starts on the registry's provider, so the registered translation model answers first and the walk
  after. Its system prompt is the translator role's prompt (`prompts/roles/translator.md`) and then the
  instructions of the purpose asked — `prompts/agents/translator/instructions.md` for a document's part,
  `worker.md` for a text a job worker reads, `record.md` for a worker's record the owner reads, that
  one after the plain-writing rulebook — and its input is one line naming the target language and then
  the text. On the registry's provider it is one request to the gateway's chat completions path under
  the registration the mapping names, with thinking off and the profile's sampling (`docs/own-llm.md`,
  「A request with no tools」). On Claude Code it is a run apart (`docs/agents.md`, 「Runs apart」): that
  system prompt as the whole of it, no tool shown or allowed, no MCP server, a fresh session, thinking
  off. No other provider carries the line. The answer is the translation whole.
- **Browse runner**: the model and the effort its tier assignment names (Frontier to start with),
  don't-ask mode, no built-in tool shown, the chrome-devtools tools, the tool host's web tools and the
  brain's `bmo` namespace from an MCP configuration written for the run — chrome-devtools started as the profile's strategy
  asks, the web namespace as every line names it, the run's own values in its session record — and
  nothing else, no settings, no other MCP, no
  Chrome integration, run in its deployed directory. The profile's browser strategy decides
  whether chrome-devtools starts standard Chrome on a virtual display the runner owns — never the
  desktop session, its D-Bus or its keyring — or headless. Its system prompt is the browse
  instructions and the rulebook against destroying the system, replacing the CLI's own; the task and the time the run has are the prompt.
  chrome-devtools is started without its performance and memory tools — tracing a page's load and
  taking a heap snapshot — since a run that reads what a page says has no use for them.
- **The web**: no agent of the system's is shown the CLI's WebSearch or WebFetch. Those hand back a
  model's rendering of a page, and a page can steer that model. The web reaches every agent
  through the tool host's `web` namespace (`mcp-host.md`):
  `web_search` asking the search engines directly (`web-access.md`), `web_fetch` returning
  the page's own text, a GitHub file page as the raw file. The runners get it from a configuration
  of their own — the search runner's standing `search.mcp.json` and the browse runner's `.mcp.json`,
  written for each run for its chrome-devtools; the job worker has the two built-ins disallowed on its line and is handed
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
  switches below.
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

## Why a line is held steady

- The prompt cache matches a byte-identical prefix, and the tools shown, the system prompt and the
  machine configuration are that prefix.
- A change to any of them rewrites every affected session's context and, in a chat, costs that chat
  its warm cache.
- So a line changes when the work it carries needs it to, all of the change in one commit, and a
  line that keeps a session is changed once rather than over several commits, each of which would
  cost every session on it a cold start of its own.
