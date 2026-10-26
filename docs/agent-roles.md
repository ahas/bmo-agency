# Agent roles

Every agent of the system holds one role: what it is for, which decides the function bundles its
turns are given. This document is the canonical statement of roles and their bundles. What each
designated agent owns apart from the others is `docs/agent-ownership.md`; what a companion's turn is
made of is `docs/companions.md`.

## What a role is

- **One role to an agent,** the `role` column of its row in the `agents` table. It is the owner's to
  change, on the console or through the control port (「Changing a role」 below).
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

- **Declared in agency's policy,** `policy/roles.toml`, one table to a role, read by the bun side and
  the Rust side alike: its label, an English word a screen translates through its own language pack,
  and the bundles it gives. A screen is handed each role and bundle with its kebab-case code beside the
  label (`docs/api.md`, 「agents」), and words the code; the label is the English the log keeps.

  | Role | Label | Bundles |
  |---|---|---|
  | `chat` | Chat | `room.chat`, `room.send`, `web.read` |
  | `worker` | Worker | `graph.read`, `queue.read`, `web.read` |
  | `searcher` | Searcher | `web.read`, `web.sites`, `web.images`, `web.branches`, `web.rulebooks` |
  | `translator` | Translator | none |
  | `gate` | Gate | none |

- **The translator is a runner, not a gate.** A gate judges a turn in the chain around it, its input and
  answer fixed; a translation is work asked on demand — a document's part, a text a job worker reads, a
  record the owner reads — and goes out on a road of its own (`docs/launch-lines.md`, 「Translate
  runner」). Its agent, `translator`, is a row of kind `runner` holding the `translator` role, whose prompt,
  `prompts/roles/translator.md`, stands before the instructions of the purpose it is asked for. It holds
  no bundle: no web, no tool.

- **A role with no bundle of its own still names what the agent is for.** A gate's runs hold what
  their line declares (`docs/launch-lines.md`) and nothing of a role.
- **A role may name a prompt its agents are given before their own,** `prompt`, a file under
  `prompts/roles/`: the role's instruction stands for every agent holding it, and the agent's own
  documents follow it. A role naming none gives none.
- **A role may bound how many of its agents run at once,** `max_concurrent`: a turn beyond the bound
  waits for a place and none is refused. A role naming none runs unbounded. The bound is on turns
  running at that moment, never on how many sessions stand.
- **A role the policy does not declare gives nothing,** and the control port refuses to set one.
- **A co-worker holds the role the policy names for co-workers,** the worker role: a co-worker is a
  sub-agent for a worker's use alone, and the place it works in, a standing branch checkout, has no row
  to name a role of its own (`docs/coworker-pool.md`, 「Standing branch checkouts and their list」).

- **A role may list the tool entries its agents are shown by name,** by full name in its `shown`
  list; every other entry its record holds is reached with `bmo use`. No role lists any now: each line's
  declaration lists what its sessions are shown.
- **The bundle that opens sending is named once,** as `send` in `roles.toml`, and every guard of a
  send reads it there.

## The chat role's bundles

- **`room.chat` — its own records and the TODO,** the chat catalogue of the system's own MCP server
  (`chat-mcp.md`, 「The chat role's catalogue」): its memories, notes and reminders, a TODO item
  for its worker and the queue read, and the consult room.
- **`room.send` — sending into the rooms it answers, under its own name,** with every verb the room's
  road carries:
  - `photo`, one picture or several as one album, each with the word to draw into it;
  - `react`, `pin` and `sticker`, as tools of the same catalogue;
  - in its shell (`docs/companions.md`, 「Its tools」), a file it made,
    sent out of the room's outbox (`upload.md`), and a file the room was sent, sent again
    (`room-files.md`);
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

## The worker role's bundles

- **`graph.read` and `queue.read` — the worker's own namespace,** the graph's four reading verbs and the
  queue's two (`docs/worker-mcp.md`).
- **`web.read` — web search and page fetch** of the web namespace. The site tools are the searcher's
  alone; a page one of them claims is still read its way when the worker fetches it.
- **Every run a worker's work goes out on holds them:** a job's turns in the worker's standing program,
  its runs on another provider's road, the program a worker stands idle, and a co-work task's turns in
  its place and the pings that warm it.

## The searcher role's bundles

- **`web.read`, `web.sites`, `web.images`, `web.branches` and `web.rulebooks` — the whole of the web
  namespace:** search and fetch, the site tools, the picture viewer, the research branch and the
  rulebooks. The tools its line does not show are reached with `bmo use` under its own record, which no
  other role holds.
- **A run's research settings narrow them** (`mcp-host.md`, 「The session record」): a research run
  holds the picture viewer, the branch tool and the rulebooks only where its settings open them, so a
  search, a browse, a branch and a news brief are each shown what their own settings ask for, and a run
  carrying no research setting at all is shown the reading tools alone.
- **The search and browse lines hold the shared brain's writing tools** (`docs/launch-lines.md`, 「A
  research run's rounds」): `brain_learn` and `brain_similar` of the `bmo` namespace, on the chat agent's brain —
  the agent whose row answers every room that names nobody. The line's session record names that agent as the
  turn the brain tools run for, so a
  research run writes what it establishes into the same brain the chat agent reads, while the run itself
  stays the runner's and its work stays the runner work key's.
- **The search runner and the browse runner hold it,** and so does every run on their behalf: a research
  branch, a news brief, and the keepalive pings that warm their lines and their branch places.

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
  capabilities with the bundles that agent's role gives, read as the record is issued
  (`mcp-host.md`, 「The session record」).
  - The agent is the one the turn names, for a turn that names one — the chat agent's and each companion's;
    otherwise the agent the launcher runs it for — the worker whose standing program it is, the agent
    a run is recorded under.
  - An agent the archive holds no row of holds no role, but for a co-worker's place, which holds the
    co-workers' role.
- **The catalogue a call is served.** The system's own MCP server serves a record the whole catalogue where it
  holds `room.tools`, and otherwise the chat catalogue's tools whose bundles it holds.
- **The send guard,** for every sending tool and script.
- **The console's agent list,** which shows each agent's role and lets the owner change it.

## Changing a role

- **The console's general tab** shows every agent's role and saves a new one as it is picked, through
  the role route below.
- **`POST /agent-role` on the control port** takes the agent and the role, and refuses an agent the
  archive does not hold or a role the policy does not declare, each with a status code and one English
  sentence (`docs/api.md`, 「Agents」).
- **A change reaches an agent's next record:** a companion's next turn, a worker's next run on
  another road, a runner's next run, and a standing program's next placement. Nothing running is
  stopped for it.

## Where a role comes from

- **The role each kind is for** is declared in `policy/roles.toml` under `kinds`: the chat role for the
  `chat` kind, the worker role for `worker`, the searcher role for `runner` and the gate role for
  `gate`. Agency's own processes register it as they load, so a row
  the store makes for a kind with no role named holds its kind's.
- **Every row added names its role:** a row written for a kind holds its kind's role unless the writer
  names another; a worker added by the owner holds the worker role; a companion's seed names its own,
  and one that names none holds the chat role.
- **A row that stood before roles** holds the role its kind is for, written once as the column is
  added: the chat agent and every companion the chat role, a worker the worker role, a runner the
  searcher role and a gate the gate role.
