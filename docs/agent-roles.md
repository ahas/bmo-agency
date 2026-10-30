# Agent roles

Every agent of the system holds one role: what it is for, which decides the function bundles its
turns are given. This document is the canonical statement of roles and their bundles. What each
designated agent owns apart from the others is `docs/agent-ownership.md`; what a companion's turn is
made of is `docs/companions.md`.

## What a role is

- **One role to an agent,** the `role` column of its row in the `agents` table. It is the owner's to
  change, on the console or through the control port (「Changing an agent's role」 below).
- **A role is what an agent is for; its kind is how it runs.** The kind names the machinery that
  carries the agent — a conversation in rooms, a worker's jobs, a gate's judgements. The role names
  what the agent is given to do. The chat agent and every companion are of the kind `chat`, carried by the one runner
  their rows' settings steer (`docs/agents.md`, 「One runner」), and all of them hold the chat role; agents of
  different kinds may share a role as well.
- **A role gives function bundles, never an agent or a kind.** No code grants a bundle to an agent by
  its name or its kind: every reader takes the agent a turn is for, reads its role and reads the
  bundles that role gives. So a companion added as a row with the chat role is given everything the
  chat role gives from its first turn, with no change anywhere else.
- **A bundle is a capability of the tool host** (`mcp-host.md`, 「Namespaces and
  capabilities」): a named set of tools a session is shown once its record holds it, and the right
  the tools' own refusals read.

## The roles

- **Kept in agency's store,** the `agent_roles` table (`docs/store.md`), one row a role: its name, its
  label, the bundles it gives, the bound on how many of its agents run at once and the prompt its agents
  are given before their own. The owner adds, changes and removes roles on the control port (「Changing
  the roles」 below), and every reader takes a change at its next read, with no deployment and no
  restart.
- **The policy seeds them,** `policy/roles.toml`: every role it declares under `[[roles]]` is given to
  the store once by its name, the first time the store is settled with it, its prompt the text of the
  file the declaration names under `prompts/roles/`. A role the store holds is never touched by the policy
  again, and one the owner removed stays removed. A role whose prompt file cannot be read is not given and
  waits for the next settling that can read it.
- **The policy keeps what is not a role's own,** read by the bun side and the Rust side alike: the bundles
  a role may give, `[[bundles]]`, each with its label; the role each kind holds (「Where a role comes
  from」); the role a co-worker holds; the bundle that opens sending; each kind's mark; and the shape of a
  role's name.
- **A label is an English word** a screen translates through its own language pack. A screen is handed
  each role and bundle with its kebab-case code beside the label (`docs/api.md`, 「agents」 and
  「agent-roles」), and words the code; the label is the English the log keeps.
- **The roles the policy seeds:**

  | Role | Label | Bundles | Bound | Prompt |
  |---|---|---|---|---|
  | `chat` | Chat | `room.chat`, `room.send`, `web.read` | 8 | none |
  | `worker` | Worker | `graph.read`, `queue.read`, `web.read` | 8 | none |
  | `searcher` | Searcher | `web.read`, `web.sites`, `web.branches`, `web.rulebooks` | 4 | none |
  | `translator` | Translator | none | 4 | `translator.md` |
  | `video` | Video producer | `queue.read`, `web.read`, `room.send` | 2 | `video.md` |
  | `gate` | Gate | none | 8 | none |

- **The translator is a runner, not a gate.** A gate judges a turn in the chain around it, its input and
  answer fixed; a translation is work asked on demand — a document's part, a text a job worker reads, a
  record the owner reads — and goes out on a road of its own (`docs/launch-lines.md`, 「Translate
  runner」). Its agent, `translator`, is a row of kind `runner` holding the `translator` role, whose prompt,
  seeded from `prompts/roles/translator.md`, stands before the instructions of the purpose it is asked for.
  It holds no bundle: no web, no tool.

- **Whatever role writes it, a result written for a person is written in the language of the person
  who asked** (`docs/agent-documents.md`): a worker's report and the documents it files, in the language
  the item's brief names for its requester, else its room; a searcher's result, in its question's
  language. The worker role's standing rules and the job-end report rule say so; what agents read of
  each other stays English.
- **A role with no bundle of its own still names what the agent is for.** A gate's runs hold what
  their line declares (`docs/launch-lines.md`) and nothing of a role.
- **A role may carry a prompt its agents are given before their own,** the row's `prompt`, written in
  English: the role's instruction stands for every agent holding it, and the agent's own documents follow
  it. A role carrying none gives none. A changed prompt reaches an agent's next launch.
- **A role may bound how many of its agents run at once,** the row's `max_concurrent`: a turn beyond the
  bound waits for a place and none is refused. A role carrying none runs unbounded. The bound is on turns
  running at that moment, never on how many sessions stand. It is read as every turn begins, so a changed
  bound holds from the next turn on, and a turn already running finishes under the bound it began under;
  none is cut.
- **A role may have a tier,** the tier its work key `role:<name>` is assigned in the tier settings
  (`docs/model-tiers.md`). It is given as the role is made or changed; a role given none has none, and its
  key stands in the tier settings once it is given one. Agency's own launches go out on their line's work
  key — a worker's jobs on `job-worker`, the translator on `translate` — and read no role's key: the
  role's tier is for a caller that runs work under the role by its name.
- **A role's name is its key,** kebab-case as the policy's pattern says, and is neither another role's nor
  a co-worker role's (`docs/coworker-pool.md`): both name their tier by `role:<name>`.
- **A role the store does not hold gives nothing,** and the control port refuses to set one.
- **A co-worker holds the role the policy names for co-workers,** the worker role: a co-worker is a
  sub-agent for a worker's use alone, and the place it works in, a standing branch checkout, has no row
  to name a role of its own (`docs/coworker-pool.md`, 「Standing branch checkouts and their list」).

- **A role may list the tool entries its agents are shown by name,** by full name in its `shown`
  list; every other entry its record holds is reached with `bmo use`. No role lists any now: each line's
  declaration lists what its sessions are shown.
- **The bundle that opens sending is named once,** as `send` in `roles.toml`, and every guard of a
  send reads it there.
- **A role gives only bundles the policy declares,** and the control port refuses a role naming another.

## The chat role's bundles

- **`room.chat` — its own records and the TODO,** the chat catalogue of the system's own MCP server
  (`chat-mcp.md`, 「The chat role's catalogue」): its memories, notes and reminders, a TODO item
  for its worker and the queue read, and the consult room.
- **`room.send` — sending into the rooms it answers, under its own name,** with every verb the room's
  road carries:
  - `photo`, one picture or several as one album, each with the word to draw into it;
  - `react`, `pin` and `sticker`, as tools of the same catalogue;
  - in its shell (`docs/launch-lines.md`, 「A companion's turn」), a file it made,
    sent out of the room's outbox, and a file the room was sent, sent again (the receiver's
    `room-files.md`);
  - its answer pinned to a message by the reply sentinel, as the chat agent's is.
- **What the room's road cannot carry is refused in that road's one sentence,** as for every sender
  (`chat-providers.md`): the bundle opens the verbs, and the road decides which of them go.
- **`web.read` — web search and page fetch** of the web namespace, the same two tools the worker reads the
  web with, shown to the chat line by name. What they find comes back to the session that called them;
  the agent answers the room with it.
- **The chat line itself holds `gate.translate`,** whatever its agent's role: the gate's translator,
  `gate/translate`, shown to the line by name. An agent asked to translate does it through that tool unless
  the asker says otherwise, and answers with the readings it returned.
- **The chat line holds `gate.reference` too:** the gate's `gate/reference_index`, `gate/reference_piece`
  and `gate/reference_search`, shown by name, which read the brain's index, one piece of it and a search
  over its documents, rules and knowledge.
- **The chat line holds `deploy`:** the testbed's `testbed/deploy`, booked rather than shown, which puts
  one commit of a repository live through the tool host (the testbed's `docs/deploy.md`, 「The hosted deploy
  tool」). The chat agent is the one that puts a closed job's commit live: a worker's close hands its commit
  over as waiting to go live and never deploys, and the testbed's own verb refuses a harness's shell, so the
  tool is the chat agent's one road to a deployment. The call is asynchronous; its outcome comes back to the
  room as an instruction, which the agent relays.
- **The chat line holds the voice namespace** under `room.tools`: the voice service's `voice/servers`, booked,
  which reads the voice node list and whether each node's server answers (the voice's `docs/boundary-api.md`,
  「The hosted tool」). The agent's shell is kept off every unit's port, so the list is read through the tool
  host and never from the voice's store or its address.

## The worker role's bundles

- **`graph.read` and `queue.read` — the worker's own namespace,** the graph's four reading verbs and the
  queue's two (`docs/worker-mcp.md`).
- **`web.read` — web search and page fetch** of the web namespace. The site tools are the searcher's
  alone; a page one of them claims is still read its way when the worker fetches it.
- **Every run a worker's work goes out on holds them:** a job's turns in the worker's standing program,
  its runs on another provider's road, the program a worker stands idle, and a co-work task's turns in
  its place and the pings that warm it.

## The video role

- **The video producer is a worker whose items make videos.** Its agent is a row of kind `worker` holding the
  `video` role, standing in a repository of its own where its projects and its studio live; its line is the
  worker line, and its role prompt, seeded from `prompts/roles/video.md`, stands first in every turn: what the studio is
  (the video-shotcraft skill bundle driving Remotion, CPU rendering), how an item is made, how the file goes
  out and the studio's licence rules. The worker plugin's `ahas-video-shotcraft` skill carries the setup, the
  render and the checks.
- **Its bundles:** `queue.read` for its items, `web.read` for the pages and assets an item names, and
  `room.send` for the finished file, which goes into the item's room through the receiver's file send tool
  as a job's own material (the receiver's `docs/room-files.md`, 「A job's sends」). It holds no graph: its
  repository is not code the graph maps.

## The searcher role's bundles

- **`web.read`, `web.sites`, `web.branches` and `web.rulebooks` — the whole of the web
  namespace:** search and fetch, the site tools, the branch tool and the rulebooks, the rulebook reader
  standing in the research unit's namespace. The look at a
  picture is the eye service's `eye/see`, which the search and browse lines hold through their `eye`
  namespace and the `room.tools` capability its entries ask for (`policy/run-lines.toml`). The tools its line does not show are reached with `bmo use` under its own record, which no
  other role holds.
- **The session's caller narrows them** (`docs/api.md`, 「sessions」): a thrown-away session on the search or
  browse line keeps only the capabilities its start's `toolsAllowed` names, so the research unit decides
  whether a search, a browse, a round or a branch holds the branch tool and the rulebooks.
- **The search and browse lines hold the shared brain's writing tools** (`docs/launch-lines.md`, 「Search
  runner」): `brain_learn` and `brain_similar` of the `bmo` namespace, on the chat agent's brain —
  the agent whose row answers every room that names nobody. The line's session record names that agent as the
  turn the brain tools run for, so a
  run writes what it establishes into the same brain the chat agent reads, while the run itself
  stays the runner's and its work stays the runner work key's.
- **The search runner and the browse runner hold it,** and so does every run on their lines: a search, a
  browse, a round and a branch the research unit sends.

## Who a send goes out as

- **A send made in an agent's turn is that agent's.** The turn's run names its agent, and every
  sending tool and script reads it for every call (`chat-mcp.md`, 「Turn context」).
- **Three things are asked before anything goes,** each refusing with a status code and one English
  sentence that names the agent:
  - the agent answers the room — it is among the room's answerers, or the room records none and the
    agent is the chat agent (`docs/companions.md`, 「Who answers a room」);
  - its role gives `room.send`;
  - the room's road carries the verb.
- **It is filed under the agent's own number and name** on a road that speaks for several speakers
  (`chat-providers.md`, `speakers`). The chat agent is the road's own voice and names no
  speaker. A road that speaks as the bot alone refuses another agent's send rather than file it as
  the chat agent's.
- **A send no agent's turn makes** — a job's check, a person at a shell, a runner of its own — is the
  bot's own voice, the chat agent's. It is asked whether that agent answers the room and whether the road
  carries the verb; a role is asked of an agent's turn alone, since no turn's bundles stand behind
  such a send.
- **Another agent's room stays closed:** an agent never sends into a room it does not answer,
  whatever its role.

## Where the role is read

- **The session record.** Every record is issued for the agent whose run it is, and holds its line's
  capabilities with the bundles that agent's role gives, read from the store as the record is issued
  (`mcp-host.md`, 「The session record」).
  - The agent is the one the turn names, for a turn that names one — the chat agent's and each companion's;
    otherwise the agent the launcher runs it for — the worker whose standing program it is, the agent
    a run is recorded under.
  - An agent the archive holds no row of holds no role, but for a co-worker's place, which holds the
    co-workers' role.
- **The catalogue a call is served.** The system's own MCP server serves a record the whole catalogue where it
  holds `room.tools`, and otherwise the chat catalogue's tools whose bundles it holds.
- **The send guard,** for every sending tool and script.
- **The turn's places,** the bound of the agent's role, read from the store as every turn begins.
- **The launch,** which reads the role's prompt from the store as it composes the agent's prompt.
- **The console's agent list,** which shows each agent's role and lets the owner change it, and its
  roles screen, which lists, adds, changes and removes the roles themselves.

## Changing an agent's role

- **The console's general tab** shows every agent's role and saves a new one as it is picked, through
  `PATCH /agents/{agentName}` with `role` (`docs/api.md`, 「agents」), which refuses an agent the store
  does not hold or a role it does not hold, each with a status code and one English sentence.
- **A change reaches an agent's next record:** a companion's next turn, a worker's next run on
  another road, a runner's next run, and a standing program's next placement. Nothing running is
  stopped for it.

## Changing the roles

- **`GET /agent-roles`** lists every role the store holds, in the order a screen lists them, each with its
  bundles by name and label, its bound, its prompt, its tier, how many agents hold it and whether it is a
  kind's own; beside them, every bundle a role may give (`docs/api.md`, 「agent-roles」).
- **`POST /agent-roles`** adds a role, and refuses a name out of the pattern or already taken, a bundle the
  policy does not declare and a tier the ladder does not hold. A tier given is written as the role's key's
  assignment in the same write.
- **`PATCH /agent-roles/{roleName}`** changes its label, bundles, bound, prompt or tier. A bound or a
  prompt given as null is cleared, and a tier given as null takes the key's assignment away.
- **`DELETE /agent-roles/{roleName}`** removes a role and its key's assignment in one write, and is refused
  while an agent holds the role or a kind's own is it.
- **A role added or changed is read at its next use,** each reader as 「Where the role is read」 says;
  nothing running is stopped for it.

## Where a role comes from

- **The role each kind is for** is declared in `policy/roles.toml` under `kinds`: the chat role for the
  `chat` kind, the worker role for `worker`, the searcher role for `runner` and the gate role for
  `gate`. Agency's own processes register it as they load, so a row
  the store makes for a kind with no role named holds its kind's. A role a kind holds is seeded with the
  rest and is never removed from the store, so every kind's own always stands.
- **Every row added names its role:** a row written for a kind holds its kind's role unless the writer
  names another; a worker added by the owner holds the worker role; a companion's seed names its own,
  and one that names none holds the chat role.
- **A row that stood before roles** holds the role its kind is for, written once as the column is
  added: the chat agent and every companion the chat role, a worker the worker role, a runner the
  searcher role and a gate the gate role.
