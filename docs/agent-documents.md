# The agents' documents

What each Claude process the bot runs is given to read, where those documents live, how they are
composed into a system prompt, and what the chat agent's own prompt is made of.

## Agents and their documents

- **Every Claude process the bot runs is given exactly its own documents and nothing else,** and the
  documents are laid out by reader. `docs/` is for people and is never injected.
- **`prompts/rules/` holds the rulebooks with more than one reader,** one copy each:
  - `answer-content.md` — the chat session;
  - `answer-shape.md` — the chat session;
  - `plain-writing.md` — every agent whose words a person reads: the chat session, the search runner
    and the news page writer, and every stored-document writer below; a job worker is pointed at it
    by its trailer;
  - `telegram-markup.md` — read by no model: the statement of what the send path's converter turns
    markdown into, kept beside the rulebooks it points to for the marks and the fold;
  - `search-kinds.md`, `product-search.md`, `restaurant-search.md`, `investment-search.md`,
    `study-material.md`, `deep-dives.md` and `research.md` — the search runner; `research.md`, the
    routine a research about a made thing follows and the three shapes its report takes, also binds a
    job that measures something the lab runs, through the worker's researching-and-benching skill;
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
  `chat/procedures/` and `chat/rules.md`, `search/task.md` and `request.md`, `browse/task.md` and
  `request.md` with its `.mcp.json`, and `worker/trailer.md`.
- **`prompts/harness.md` is the platform's harness opening,** agency's own and no program's: what the
  harness around the agent is, how words reach it, where its memory is and what the platform around it
  holds. Every line on Claude Code opens its whole system prompt with it unless a base prompt stands in
  its place — the chat session, every companion that chooses no base, every job worker and co-worker,
  the search runner, its branches and the browse runner (`docs/base-prompts.md`, 「The harness layer」).
  No gate and not the translator, whose prompts are handed whole by their caller and their role, and
  not dsh, whose own prompt is its harness.
- **No gate's prompt stands here.** A gate's instructions, rulebooks and order are the gate unit's: it
  hands its whole system prompt with each turn it sends, and agency reads the order of the gate kinds
  from the gate's declaration (`docs/api.md`, 「gate kinds」).
- **A job's documents:** `worker/graph.md`, what a job is told of the code graph; `worker/trailer.md`,
  the trailer every job is held to, with the graph text, the path of the worker plugin's skills and the
  path of `plain-writing.md` filled in as the deployment shows them, which stands in the worker's prefix
  — the rules its line hands after the harness opening in its whole system prompt
  (`docs/launch-lines.md`) — so a session
  pays for it once rather than on every turn; and `handoff.md` in a worker's own folder — its base's for
  a support worker — the instruction it writes its own handoff note by, where it holds one. The
  worker's definition route answers all three (`docs/api.md`, 「Workers」).

## The web reaches agents through the tool host

- **Agency's deployment writes `worker.mcp.json`,** the job worker's own configuration and the only
  line handed it, naming the tool host's `worker` namespace beside its `web` namespace
  (`mcp-host.md`). The chat line's `chat.mcp.json` is not the deployment's to write: the chat line
  composes it on every turn, the tool host's `bmo`, `web` and `gate` namespaces beside the two harness
  plugins (`docs/launch-lines.md`, 「What is fixed」), and one writer keeps the file from moving between two
  compositions. A hosted tool those namespaces do not show is reached with `bmo use`.
- **The tool host's web namespace is how the web reaches every agent allowed to use it:** its
  `web_search` and `web_fetch` are the tool host's (`web-access.md`). The CLI's WebSearch and
  WebFetch, which return a model's rendering of a page that the page itself can steer, are shown to
  none of agency's agents: every line denies them.

## How the documents are composed

- **Agency's prompt composer composes these into what each agent sees** — the chat session's system
  prompt, each gate's, each runner's standing instructions and the request of each run — and the
  launch lines read from it, fresh on every run, so an edited document applies at the next launch.
- **A system prompt reaches the CLI as a file,** written under the deploy root's `prompts/` for that
  launch and named after the chat line or the gate, never on the command line: a chat's runs tens of
  kilobytes.
- **The prompt-composing entry of the `bmo-agency` CLI** — `bmo-agency prompts`, which runs the agent
  package's `deploy-agents.ts` on Bun out of the code tree; run by the reload path whenever a document
  under `prompts/rules/` or `prompts/agents/` changes, by hand, and on every start of agency's server —
  writes the same composition out under the agents directory the data root holds (`BMO_AGENTS_DIR`):
  agency's own agents' prompts for inspection, and for each runner the directory it runs in, which sits
  outside the repository on purpose so that neither a project instruction file up the tree nor the
  repository's git status can enter a runner's prefix. It writes no gate's prompt.

## The chat agent

- **The chat agent's prompt is three layers, each whole, in this order:** the harness, the role and the
  persona. No layer is cut into sections chosen per program, and none knows the others.
  - **The owner's text of three of the role's documents stands in the store:** the opening, the
    absolute rules and the manual are read from `chat_documents` where the owner saved one and from
    the files named here otherwise (`docs/api.md`, 「chat-documents」); the rulebooks and the room's
    documents are files alone.
  - **The harness** on Claude Code is agency's own `prompts/harness.md`, not the program's prompt: the
    three layers go to the program as one whole system prompt in the place of its own. On dsh the
    harness is dsh's own prompt for the headless profile, with the role and the persona as its persona
    suffix after dsh's own guidance, and agency's opening is not sent.
  - **The role** is the chat role's prompt, the same whatever the program: the documents below, in the
    order they are named, each block opening with a one-line head that says what it is.
  - **The persona** closes the prompt: the base layer of the agent's own brain, under the persona
    header of the chat line's policy, which says the persona holds in every round of a message.
- **The absolute rules open the role:** `prompts/agents/chat/context.md` holds
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
  with its sentinels, the language an answer is written in, what the room hears of the agent's tools,
  the draft process, the answer rules, promises, and a turn past its clock. It says how a message of each kind reaches the
  agent, a voice note among them arriving as what was said in it rather than as a placeholder it cannot open. It is how
  the chat agent works at all, and it says when to act.
  It names no tool of the program the session runs on, since one manual serves every program the chat
  stands on.
- **The tools paragraph follows the manual,** composed at each launch for the program the session runs
  on, in the words `policy/chat-line.toml` holds for it: that program's own tools by their names — on
  Claude Code the tools the line shows, on dsh the tools the overlay leaves its session —, the servers
  the line's MCP configuration hands the session by name, the tool host's entries the line shows by name
  (`policy/run-lines.toml`, `shown`) — each called as that tool, never through the shell or `bmo use` — and
  that only a hosted tool not shown is reached with `bmo use`, which the harness opening says how to use.
  It names the read path too: what
  the turn does not already hold of a person, a room or a thing is looked up with the brain's
  `brain_list`, by its subject and type, or `brain_similar`, by the question's words, and a document or a
  rule with the gate's `reference_search` and then `reference_piece`. The brain's own discipline for
  writing and reading stands in the brain's own document, which the prompt hands in. A server that does not attach
  is told to the session afterwards (`docs/agent-control.md`, 「dsh's providers: OpenRouter and
  DeepSeek」).
- **The rulebooks the role carries hold rules, never past cases:** a rulebook states what an answer
  is, and no example answer or account of a turn that went wrong stands in one, since an example becomes
  the agent's own phrasing.
- **What the agent says of an item it says in the room the item was asked in,** the room its `scope` or
  `roomId` names, and in the owner's private room only for an item that names none. A run's end is relayed
  there in a few lines: its outcome, the commit or artefact it left, and one line of what it left out; the
  worker's report stays whole on the item, where the console shows it, and is never pasted into the room.
- **The persona's voice holds in every round.** The persona sets how the agent speaks to each person,
  the owner included, and the header it stands under says that voice holds for every message the agent
  sends, a report or what a run came to among them; the role never frames a round's answer as a report
  in another voice.
- **No translation hint stands in the role:** what the agent writes it writes in the person's language
  itself, and the gate's translator is one of its tools like any other, named only where the tools
  paragraph lists it.
- **Only the chat line answers in a speaker's language.** The chat agent and every companion, answering
  people in rooms, write in the language of the person they answer.
- **A result written for a person is written in the language of the person who asked,** whichever
  agent writes it: a job's report and the account it closes its item with, a document it files for the
  archive, a measurement report, a research write-up. The person who asked is the item's requester, else
  its room; the item's brief names that language, and a search's result is in the language its question
  is asked in (`docs/research-runs.md`). Numbers, tables, code, commands, paths, identifiers, sources and
  quoted words stand as they are, and a repository's own documents, its commits and everything pushed
  with them stay English.
- **Everything else is English:** a worker's and a co-worker's notes and the questions it puts to the
  owner, a gate's verdict, a subagent's prompt and report, the logs; so does the chat agent wherever no
  person in a room reads it — an item's brief and notes, a work order, a memory, a handoff note — words
  quoted from people kept as they said them. The exceptions are the agents whose product is text a person
  reads in a language of its own: a translation, a line for the voice, the news page.
- **No document fixes the language the chat agent answers in.** It answers each person in the language it
  knows for that person — what the brain holds of them, their country and the language they usually speak, and
  how the conversation has gone — never a language read off a message's words: commands, paths or code a
  message carries do not decide it, and what the owner is told is in the owner's language. A rule about Korean
  wording holds for an answer written in Korean. The documents carry rules, never example sentences, since an
  example becomes the agent's own phrasing.
- **The room never hears of the agent's tools.** No message names a tool, a search, the translator or
  any other part of the system, or says one is being called; where a wait wants explaining, the agent
  says in its own words that it will look something up, look into it or translate it. The owner's own
  question about how it works is the exception the absolute rules make.
- **The manual describes the draft process as the receiver runs it** (the receiver's `docs/drafting.md`):
  the questions asked in rounds after a turn, every answer before the last being free-form notes of what
  was read, established in the brain and still unknown, the tools open in every round, the message
  written in the last, `<<SEND>>` opening an answer making it the message and ending the process,
  `<<SKIP>>` sending nothing, and the first round's freedom to answer at once, to search quietly where a
  fact may not hold, and never to hedge about a fact being out of date. The receiver owns the questions;
  the manual only tells the agent what an answer to each is.
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
  `prompts/rules/answer-content.md`, `prompts/rules/answer-shape.md` and `prompts/rules/plain-writing.md`; then
  the rule against destroying the system, `prompts/rules/self-preservation.md`; then the brain's
  discipline, how the agent reads and writes its long-term memory every turn, read from the brain at
  the launch through its client and left out where no brain answers; then the tool host's paragraph on
  finding and using tools with `bmo use`, read from the tool host at the launch through its client, whole
  and as the host words it, and left out where the host does not answer; and then the persona, the base
  layer of the agent's own brain, or the one the turn hands in where the brain gives none
  (`docs/sessions.md`, 「Document reload」).
- **The role and the persona follow the harness, never replace it:** the harness opening stands
  first and whole — agency's own on Claude Code, dsh's on dsh — while this machine's settings,
  instruction files, MCP servers and skills reach no chat session.
- **The session stands as the chat agent's resident program in its pane,** on the login that session was opened
  under, and that program keeps its slash commands, so compaction is `/compact` typed into it.
- **The login is part of what identifies a session** rather than a detail of the run: a fold sent out
  under any other one is told there is no such conversation, which is not a session too full to fold
  but a session the CLI cannot see at all.
