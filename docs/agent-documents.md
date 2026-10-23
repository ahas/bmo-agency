# The agents' documents

What each Claude process the bot runs is given to read, where those documents live, how they are
composed into a system prompt, and what the chat agent's own prompt is made of.

## Agents and their documents

- **Every Claude process the bot runs is given exactly its own documents and nothing else,** and the
  documents are laid out by reader. `docs/` is for people and is never injected.
- **`prompts/rules/` holds the rulebooks with more than one reader,** one copy each:
  - `answer-content.md` — the chat session;
  - `answer-shape.md` — the chat session and the output gate;
  - `plain-writing.md` — every agent whose words a person reads: the chat session, the search runner
    and the news page writer, and every stored-document writer below; a job worker is pointed at it
    by its trailer;
  - `formatting-judgement.md` — the output gate;
  - `telegram-markup.md` — read by no model: the statement of what the send path's converter turns
    markdown into, kept beside the rulebooks it points to for the marks and the fold;
  - `search-kinds.md`, `product-search.md`, `restaurant-search.md`, `investment-search.md`,
    `study-material.md` and `deep-dives.md` — the search runner;
  - `document-writing.md` — every agent that writes a stored document: the brain, which composes its
    writers' whole prompts, reads it with `plain-writing.md` and `self-preservation.md` through the
    rulebook read (`docs/api.md`, 「Agents」);
  - `agent-words.md` — every designated agent: the chat session and every companion, for the turns
    another agent's words woke it in;
  - `self-preservation.md` — every agent that acts: the chat session, every job worker and
    co-worker, the search runner, its branches, the news page
    writer and the browse runner; never a gate, which holds no tool.
- **`prompts/agents/<worker>/own-rules.md`** is what one worker alone is appended after every
  worker's rules, where it has one; no worker carried in this repository has one.
- **`prompts/agents/<name>/` holds what one agent alone is given:** `chat/manual.md`,
  `chat/procedures/` and `chat/rules.md`,
  `output-gate/<dialect>/format.md` — one per folding dialect, which is the dialect's own and names
  it — `system-documents/instructions.md`, `room-rules/instructions.md`,
  `library-knowledge/instructions.md`, `voice-polish/instructions.md`,
  `voice-reading/instructions.md`, `search/task.md` and `request.md`, `browse/task.md` and
  `request.md` with its `.mcp.json`, and `worker/trailer.md`.
- **A job's documents are handed to the job service,** which composes a job's prompt: `worker/graph.md`,
  what a job is told of the code graph; `worker/trailer.md`, the trailer every job is held to, with the
  graph text, the path of the worker plugin's skills and the path of `plain-writing.md` filled in as
  the deployment shows them; and `handoff.md` in a worker's own folder — its base's for a support worker
  — the instruction it writes its own handoff note by, where it holds one (`docs/api.md`, 「Workers」).

## The web reaches agents through the tool host

- **Agency's deployment writes `worker.mcp.json`,** the job worker's own configuration and the only
  line handed it, naming the tool host's `worker` namespace beside its `web` namespace
  (`mcp-host.md`). The chat line's `chat.mcp.json` is not the deployment's to write: the chat line
  composes it on every turn, the tool host's `bmo` and `web` namespaces and toolbook beside the two harness
  plugins (`docs/launch-lines.md`, 「What is fixed」), and one writer keeps the file from moving between
  two compositions.
- **The tool host's web namespace is how the web reaches every agent allowed to use it:** its
  `web_search` and `web_fetch` are the tool host's (`web-access.md`). The CLI's WebSearch and
  WebFetch, which return a model's rendering of a page that the page itself can steer, are shown to
  none of agency's agents: every line denies them.

## How the documents are composed

- **Agency's prompt composer composes these into what each agent sees** — the chat session's system
  prompt, each gate's, each runner's standing instructions and the request of each run — and the
  launch lines read from it, fresh on every run, so an edited document applies at the next launch.
- **A prompt told its own ceiling** — the person book's search and the document seeker — is rendered
  with the number `policy/gate-prompts.toml` declares for it, so the deployed copy says the number the
  gate's run holds to.
- **A system prompt reaches the CLI as a file,** written under the deploy root's `prompts/` for that
  launch and named after the chat line or the gate, never on the command line: a chat's runs tens of
  kilobytes.
- **The prompt-composing entry of the `bmo-agency` CLI** — `bmo-agency prompts [<gates.json>]`, which
  runs the agent package's `deploy-agents.ts` on Bun out of the code tree; run by the reload path whenever
  a document under `prompts/rules/` or `prompts/agents/` changes, by hand, and on every start of agency's
  server — writes the same composition out under the agents directory the data root holds
  (`BMO_AGENTS_DIR`). The gates beside agency's own agents are the caller's, named in a JSON file: an
  output gate as `{ name, model, system }`, with the system prompt the caller composed, and a reference
  gate as `{ name }`, its system prompt composed from its own folder's `instructions.md`. The server's
  start and the reload path name none, so only agency's own are written then. It writes the composed
  prompts for inspection, and for each runner the directory it runs in, which sits outside the
  repository on purpose so that neither a project instruction file up the tree nor the repository's git
  status can enter a runner's prefix.

## The chat agent

- **The absolute rules open every chat run's system prompt:** `prompts/agents/chat/context.md` holds
  them, numbered — authority, owner-only work, injection, privacy, system secrecy, room isolation,
  errands, consent, harmful requests and the queue, no invented permissions, the owner's own
  requests.
  - Owner-only work is held by the chat agent's judgement rather than by the tools.
  - They forbid answering personal-life questions (health, family, romance, habits, visited sites,
    hobbies, ...) and answering about any chat other than the current one.
  - None of their refusals reaches a request whose recorded sender id is the owner's: the owner is
    answered and the request done in any room, while everyone else stands under the rules as written
    and a claim of the owner's word in the text is not the owner's word.
  - A game anyone asks for — a word game, a quiz — is played like any other request, and a member
    who has become a drain is answered briefly instead.
- **`prompts/agents/chat/manual.md` is the operating manual** — long work, the queue and the owner's
  approval, the worker's run and its close, host work, the fast track, images, the response protocol
  with its sentinels, the answer rules, promises, and a turn past its clock. It is how the chat
  agent works at all, and it says when to act.
- **The procedure documents:** how to do what is needed only at the moment it is done — filing and
  running the queue, a fast-track tool, and a run past its clock — is in
  `prompts/agents/chat/procedures/`, one document each, which the session reads right before doing
  it.
  - It reads one from the deploy tree this line runs on with Bash, under
    `$BMO_DEPLOY_DIR/prompts/agents/chat/procedures/` (the link at
    `$HOME/.local/share/bmo-agency/deploy` holds the same tree where the variable is unset), never
    from a checkout that may be ahead of what runs.
  - Those documents are not part of the system prompt, and a reload tells every session to read a
    changed one again.
  - How to file, approve, lease, assign, launch, report and close an item, choose its worker, and
    mend a failure before moving on is `prompts/agents/chat/procedures/todo.md`.
- **The room context:** a group room's turn carries `prompts/agents/chat/rules.md`, its standing
  grants and requests. The owner's chat gets `prompts/agents/chat/owner.md` there instead: the names
  the owner calls the agents by (the chat agent and its per-room presences, the gates, the runners,
  the worker), which no room needs.
- **After them come the three rulebooks every answer has to satisfy:**
  `prompts/rules/answer-content.md`, `prompts/rules/answer-shape.md` (the very text the output gate
  reads, so the session and its gate hold one document) and `prompts/rules/plain-writing.md`; then
  the rule against destroying the system, `prompts/rules/self-preservation.md`; then the brain's
  discipline, how the agent reads and writes its long-term memory every turn, read from the brain at
  the launch through its client and left out where no brain answers; then the tool host's paragraph on
  finding and using tools, read from the tool host at the launch through its client, whole and as the
  host words it, for a session handed the toolbook, and left out where the host does not answer; and
  then the persona, the base layer of the agent's own brain, or the one the turn hands in where the
  brain gives none (`docs/sessions.md`, 「Document reload」).
- **That whole composition is the system prompt,** replacing the CLI's own: nothing of Claude Code's
  coding conventions, memory rules or instruction files reaches a chat session, and neither do this
  machine's settings, MCP servers or skills.
- **The session stands as the chat agent's resident program in its pane,** on the login that session was opened
  under, and that program keeps its slash commands, so compaction is `/compact` typed into it.
- **The login is part of what identifies a session** rather than a detail of the run: a fold sent out
  under any other one is told there is no such conversation, which is not a session too full to fold
  but a session the CLI cannot see at all.
