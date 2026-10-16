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
  - `document-writing.md` — every agent that writes a stored document: the digest and person-book
    editors and the standing-sheet summariser, ahead of their own instructions, `plain-writing.md`
    right after it;
  - `agent-words.md` — every designated agent: the chat session and every companion, for the turns
    another agent's words woke it in;
  - `recollection.md` — every recollector: the digest, the person-book writer and the session
    summariser;
  - `self-preservation.md` — every agent that acts: the chat session, every job worker and
    co-worker, the digest and person-book editors, the search runner, its branches, the news page
    writer and the browse runner; never a gate, which holds no tool.
- **`prompts/agents/<worker>/own-rules.md`** is what one worker alone is appended after every
  worker's rules, where it has one; no worker carried in this repository has one.
- **`prompts/agents/<name>/` holds what one agent alone is given:** `chat/manual.md`,
  `chat/procedures/` and `chat/rules.md`,
  `output-gate/<dialect>/format.md` — one per folding dialect, which is the dialect's own and names
  it — `system-documents/instructions.md`, `room-rules/instructions.md`,
  `library-knowledge/instructions.md`, `voice-polish/instructions.md`,
  `voice-reading/instructions.md`, `search/task.md` and `request.md`, `browse/task.md` and
  `request.md` with its `.mcp.json`, `digest/task.md`, `people/task.md` and `worker/trailer.md`.

## The web reaches agents through the tool host

- **Agency's deployment writes `worker.mcp.json`,** the job worker's own configuration and the only
  line handed it, naming the tool host's `worker` namespace beside its `web` namespace
  (`mcp-host.md`).
- **The tool host's web namespace is how the web reaches every agent allowed to use it:** its
  `web_search` and `web_fetch` are the tool host's (`web-access.md`). The CLI's WebSearch and
  WebFetch, which return a model's rendering of a page that the page itself can steer, are shown to
  none of agency's agents: every line denies them.

## How the documents are composed

- **Agency's prompt composer composes these into what each agent sees** — the chat session's system
  prompt, each gate's, each runner's standing instructions and the request of each run, each editor's
  task — and the launch lines read from it, fresh on every run, so an edited document applies at the
  next launch.
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
    who has become a drain is muted by the chat agent's own judgement instead.
- **`prompts/agents/chat/manual.md` is the operating manual** — long work, messages and small room
  actions, watches, the TODO document, images, search, browsing, the response protocol with its
  sentinels, the answer rules, notes, the archive scanner and work orders. It is how the chat agent
  works at all, and it says when to act.
- **The procedure documents:** how to run a command that is needed only at the moment it is run —
  deploying, folding and reloading, the send scripts and their media, watches, running a web search,
  filing and managing TODO items, changing a work order, opening and stopping a word-chain game, a
  fair draw — is in `prompts/agents/chat/procedures/`, one document each, which the session reads
  right before running the command.
  - It reads one as a resource of its own server, `bmo://procedure/<name>`, or through
    `bmo procedure show`; either reads it as the room it is read in may use it, leaving out what
    needs a verb that room's chat provider does not perform, so one copy of each document serves
    every road.
  - Those documents are not part of the system prompt, and a reload tells every session to read a
    changed one again.
  - How to read the queue, choose, launch a job, write its prompt, and mend a failure before moving
    on is `prompts/agents/chat/procedures/todo.md`; how to put the backlog live is `deploying.md`.
- **The room context:** a group room's turn carries `prompts/agents/chat/rules.md`, its standing
  grants and requests. The owner's chat gets `prompts/agents/chat/owner.md` there instead: the names
  the owner calls the agents by (the chat agent and its per-room presences, the gates, the runners,
  the editors, the worker), which no room needs.
- **After them come the three rulebooks every answer has to satisfy:**
  `prompts/rules/answer-content.md`, `prompts/rules/answer-shape.md` (the very text the output gate
  reads, so the session and its gate hold one document) and `prompts/rules/plain-writing.md`; then
  the rule against destroying the system, `prompts/rules/self-preservation.md`; and then the persona.
- **That whole composition is the system prompt,** replacing the CLI's own: nothing of Claude Code's
  coding conventions, memory rules or instruction files reaches a chat session, and neither do this
  machine's settings, MCP servers or skills.
- **The session stands as the chat agent's resident program in its pane,** on the login that session was opened
  under, and that program keeps its slash commands, so compaction is `/compact` typed into it.
- **The login is part of what identifies a session** rather than a detail of the run: a fold sent out
  under any other one is told there is no such conversation, which is not a session too full to fold
  but a session the CLI cannot see at all.
