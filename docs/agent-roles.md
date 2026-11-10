# Roles

A role is the executing axis of agency (`docs/agents.md`, 「Two axes」): what a run is for, which decides
the prompt it opens on, the tools it is given, the providers and models it runs on, how many of its runs go
at once and whether its runs keep their sessions. This document is the canonical statement of roles and
their bundles. How a role's runs are let in is `docs/role-slots.md`; how a run gets its provider is
`docs/provider-order.md`.

## What a role is

- **Every run holds one role,** the one the work names (「Which role a run holds」 below). A role is a preset —
  a prompt, tools, skills, a line and how its sessions are kept — that every run of it reads as it starts; the
  seats its runs take, each on one road, are its agents (`docs/role-agents.md`).
- **A role gives function bundles, never a run by its name.** Every reader takes the role a run holds and
  reads its row, so a role added as a row is given everything its row says from its first run, with no
  change anywhere else.
- **A bundle is a capability of the tool host** (`mcp-host.md`, 「Namespaces and capabilities」): a named set
  of tools a session is shown once its record holds it, and the right the tools' own refusals read.
- **The workers, the coder, the doppelganger, the helper, the producer, the voyager, the seeker, the translator,
  the transliterator and every co-worker are roles alike.** Nothing tells a co-worker role from any other
  role but what its row says.

## What a role row holds

- **Kept in agency's store,** the `roles` table (`docs/store.md`), one row a role:
  - **its name,** its key, kebab-case as the policy's pattern says, which a screen shows the role by, and
    **its label**, the English words a log keeps of it (「A role's label」 below);
  - **its prompt,** written in English, which every run of the role opens with after the platform's harness
    opening; a role carrying none gives none;
  - **its description,** one paragraph saying what the role is for and when to call it, which is what a
    run that may call co-workers is shown of the role (「Co-workers」 below);
  - **its bundles,** the function bundles its runs are given, each one the policy declares;
  - **its skills,** the names of the skills its runs carry on the worker line, or none set, which leaves its
    runs the whole plugin the release places for the role (`docs/role-agents.md`, 「What a run reads off its
    agent」);
  - **its line,** the launch line its runs go out on — the worker line, the search line, the browse line or
    the translate line (`docs/launch-lines.md`);
- **A role holds no road, no cap, no sessions setting, no compaction threshold and no word on being called.**
  Whether a run keeps its session, the share at which that session is folded and whether a run of another
  role may call it as a co-worker are each its agent's (`docs/role-agents.md`, 「What an agent row holds」).
- **A role holds no road and no cap.** The roads its runs go out on are its agents', one each, and how many
  of its runs go at once is the number of its agents that are switched on (`docs/role-agents.md`). A role
  with no agent runs nothing.
- **A role whose agents name no road follows the supervisor that asks it.** A run of such a role a
  supervisor asks goes out on that supervisor's own provider, model and effort as they stand when it is
  asked; one no supervisor asks is refused with why (`docs/role-slots.md`, 「A delegated task」). The role's
  row answers `followsSupervisor` true while none of its agents names a road (`docs/api.md`, 「roles」).
- **An agent may lay its own prompt, bundles and skills over its role's,** each standing in place of the
  role's for that agent's runs alone (`docs/role-agents.md`, 「What an agent row holds」).
- **The owner adds, changes and removes roles on the control port** (「Changing the roles」 below), and every
  reader takes a change at its next read, with no deployment and no restart.

## The bundles

- **The policy keeps what is not a role's own,** `policy/roles.toml`, read by the bun side and the Rust side
  alike: the bundles a role may give, `[[bundles]]`, each with its label; the supervisor bundles every
  supervisor holds (`docs/supervisors.md`, 「The bundles a supervisor holds」); the role a job runs as where
  its item names none, `worker`; the role delegated work goes to where its ask names none, `doppelganger`; the
  role each of agency's own runs that stands for no one's work holds, `upkeep`; the bundle that opens
  sending; and the shape of a role's name.
- **A role gives only bundles the policy declares,** and the control port refuses a role naming another.
- **A role may list the tool entries its runs are shown by name,** by full name in its `shown` list; every
  other entry its record holds is reached with `bmo use`. No role lists any now: each line's declaration
  lists what its sessions are shown.
- **A screen is handed each role and bundle with its kebab-case code beside the label** (`docs/api.md`,
  「roles」): it shows a role by its name (「A role's label」 below) and words a bundle by its code; the label
  is the English a log keeps.

## A role's label

- **A role's label is its row's,** never empty: every seeded role is given the label the policy declares
  for it, and an add or a change refuses an empty one. It is the English word a log keeps of the role, and
  agency's API answers it with the row.
- **A screen shows a role by its name alone,** the kebab-case key agency holds it under — `worker`,
  `coder`, `doppelganger`, `voyager`, `seeker`, `producer` and every other — on every screen that names
  one: never the row's label and never a word translated from the name. A role the owner adds is shown by
  its name from its first listing.

## The roles the policy seeds

- **The policy seeds them,** `policy/roles.toml`: every role it declares under `[[roles]]` is given to the
  store once by its name, the first time the store is settled with it, with the prompt and the description
  its declaration gives. A role naming a prompt file under `prompts/roles/` takes that file's text as its
  prompt and the one sentence its declaration states as its description; a role naming a co-worker
  definition under `prompts/agents/worker/coworkers/` takes the definition's body as its prompt and its
  front matter's description as its description, and states none of its own; a role naming neither takes
  no prompt and the description its declaration states. Every seeded role carries a description. A role
  the store holds is never touched by the policy again, and one the owner removed stays removed, so a
  prompt or a description the policy gives a role the store already holds reaches the row only through
  `PATCH /roles/{roleName}`. A role whose prompt file or definition cannot be read is not given and waits
  for the next settling that can read it.
- **The seeded roles:**

  | Role | Label | Line | Bundles | Cap | Prompt | Description | Sessions | Co-worker |
  |---|---|---|---|---|---|---|---|---|
  | `worker` | Worker | worker | `graph.read`, `queue.read`, `web.read` | 3 | none | policy | keep | no |
  | `coder` | Coder | worker | `graph.read`, `queue.read`, `web.read` | 3 | `coder.md` | policy | keep | no |
  | `doppelganger` | Doppelganger | worker | `graph.read`, `queue.read`, `web.read` | 2 | `doppelganger.md` | policy | keep | no |
  | `helper` | Helper | worker | `graph.read`, `queue.read`, `web.read` | 2 | `helper.md` | policy | keep | no |
  | `producer` | Producer | worker | `queue.read`, `web.read`, `room.send` | 2 | `producer.md` | policy | keep | no |
  | `builder` | Builder | worker | `graph.read`, `queue.read`, `web.read` | 8 | definition | definition | keep | no |
  | `verifier` | Verifier | worker | `graph.read`, `queue.read`, `web.read` | 6 | definition | definition | fresh | yes |
  | `scout` | Scout | worker | `graph.read`, `queue.read`, `web.read` | 12 | definition | definition | fresh | yes |
  | `screener` | Screener | worker | `graph.read`, `queue.read`, `web.read` | 12 | definition | definition | fresh | yes |
  | `voyager` | Voyager | search | `web.read`, `web.sites`, `web.branches`, `web.rulebooks` | 4 | none | policy | fresh | no |
  | `seeker` | Seeker | browse | `web.read`, `web.sites`, `web.branches`, `web.rulebooks` | 4 | none | policy | fresh | no |
  | `translator` | Translator | translate | none | 4 | `translator.md` | policy | fresh | no |
  | `transliterator` | Transliterator | search | none | 4 | none | policy | keep | no |
  | `upkeep` | Upkeep | worker | none | 1 | none | policy | fresh | no |

  A prompt named by file is the file under `prompts/roles/`; `definition` is the co-worker definition of the
  role's name under `prompts/agents/worker/coworkers/`. A `policy` description is the sentence the role's
  declaration states.

- **Each seeded role's agents are the policy's,** given with the row: as many agents
  as its declaration's count, each on the road it declares and with the sessions setting, the compaction
  threshold and the callability the declaration states (`docs/role-agents.md`, 「The roles the policy
  seeds」), changed by the owner afterwards like any agent. The `doppelganger` and the `coder` declare no road,
  so their agents follow the supervisor that asks them until the owner sets an agent a road.
- **Whatever role writes it, a result written for a person is written in the language of the person who
  asked** (`docs/agent-documents.md`): a job's report and the documents it files, in the language the item's
  brief names for its requester, else its room; a voyager's result, in its question's language. What
  roles read of each other stays English.

## The worker role

- **The role a job runs as where its item names none.** Its run works in a repository's checkout, on what
  the repository hands it — its prefix documents, the prompt that stands before the job's and its rules
  (`docs/repositories.md`) — under the worker line's standing rules.
- **It keeps the job whole:** the brief and its notes, the work order and its tasks, the checkout and the
  branch, the checks the repository declares, the close and the report. The coding passes are not its own
  (「The coder role」 below).
- **It carries no prompt:** a job's run opens on the harness opening and the worker line's rules, with
  what its repository hands it after them.
- **Its bundles:** `graph.read` and `queue.read`, the worker's own namespace, the graph's four reading verbs
  and the queue's two (`docs/worker-mcp.md`); and `web.read`, web search and page fetch. The site tools are
  the voyager's and the seeker's alone.
- **Any job may name another role,** whichever the owner chose for the item (`docs/repositories.md`, 「A
  job」), and its run holds that role's prompt and bundles, on one of that role's agents, in the same
  repository's checkout.

## The coder role

- **The role a coding job runs as.** Its run works in the same repository's checkout, on what the
  repository hands it, and holds the coding work the item asks for: the code, its design, its tests, the
  documents that cover it, and the commit. The item, the close and the report stay the job's, as on the
  worker line.
- **Its line is the worker line,** and its prompt, seeded from `prompts/roles/coder.md`, stands first in
  every run after the harness opening, before the line's standing rules and the repository's own.
- **Its coding skills are its own bundle's,** the `coder` bundle placed at `prompts/agents/coder/plugin/`:
  `ahas-code-design-patterns` first where a structure is under question, `ahas-code-refactoring` second,
  `ahas-code-clean` third and last, `ahas-code-test-writing` before a test is written or changed,
  `ahas-docs-git` and `bmo-docs-conventions` for the documents and the commit, and `ahas-ux-design-rules`
  for a screen. Its prompt names them; the worker's run carries the `worker` bundle, which holds none of
  them.
- **Its plugin holds the worker bundle's skills as well,** the ones every job needs whatever its work —
  closing a job, asking the owner, reaching beyond the checkout — placed beside its own, so a coder's job
  ends the way any job does. The producer's plugin holds them the same way.
- **Its bundles are the worker's:** `graph.read`, `queue.read` and `web.read`.
- **Its agents are seeded on no road,** so a run of it a supervisor asks goes out on that supervisor's own
  provider, model and effort until the owner sets an agent a road of its own.
- **It is seeded with three agents, as the worker is,** so a repository's coding jobs run as many at once as
  its jobs did.
- **Which jobs name it:** an item whose work changes a repository's code names `coder`; one whose work is
  not code names `worker`, and an item naming no role stays `worker` (`docs/repositories.md`, 「A job」).
  An item already raised keeps the role it named, so every existing job runs as it did.

## The helper role

- **The role a supervisor names when it hands off a problem it could not settle in its own turn** — a design
  question, a stubborn bug, a plan that has to hold, an analysis that needs care (`docs/tools.md`, 「The
  delegate」; the chat agent's manual, `prompts/agents/chat/manual.md`). Its prompt, seeded from
  `prompts/roles/helper.md`, stands first in every run.
- **Its runs go out on the helper role's own order, the strongest road the deployment names,** and hold a slot
  of the helper role, like any role named in a delegation; a run a supervisor asks holds that supervisor's tools
  beside its bundles (`docs/tools.md`, 「The delegate」). This is what sets it apart from the double, which goes
  out on the asking supervisor's own road: the helper is asked where the problem wants a better model than the
  supervisor's, the double where it wants the supervisor's own hands elsewhere.
- **Its runs work in the folder the task names,** the asking supervisor's scratch workspace under the data
  root where none is named, so a delegation that follows another at the same folder on the same provider and
  model goes on in the same session while it is valid (`docs/sessions.md`, 「A role's session」).
- **Its bundles** are the worker's: `graph.read`, `queue.read` and `web.read`.

## The doppelganger role

- **The role a supervisor's delegated task runs as where the ask names none** (`docs/tools.md`, 「The
  delegate」): the supervisor's double. Its prompt, seeded from `prompts/roles/doppelganger.md`, stands first
  in every run.
- **A turn of it is a task or a round handed over whole.** A task names the folder to work in and what to
  find or do, and its answer is a report the supervisor carries back. A round a supervisor's own draft turn
  handed over (the receiver's `docs/drafting.md`, 「A round handed to the double」) carries the room's block
  and the round's turn as the supervisor read them: the double then answers as the supervisor itself, by that
  block's rules, and its answer is the message the room hears. The role's prompt tells the two apart.
- **Its agents are seeded on no road, so a run of it goes out on the asking supervisor's own provider, model
  and effort** as they stand when it is asked, and waits while that provider has no quota
  (`docs/role-slots.md`, 「A delegated task」). An agent the owner sets a road goes out on that road instead,
  like any role's agent.
- **Its agents bound every supervisor's delegations together,** two as seeded, a number the owner changes by
  adding, removing or switching its agents.
- **Its runs work in the folder the task names,** the asking supervisor's scratch workspace under the data
  root where none is named, so a delegation that follows another at the same folder on the same provider and
  model goes on in the same session while it is valid (`docs/sessions.md`, 「A role's session」).
- **Its bundles** are the worker's: `graph.read`, `queue.read` and `web.read`; a run a supervisor asks holds
  that supervisor's tools beside them (`docs/tools.md`, 「The delegate」).

## The producer role

- **The producer is a role whose items make videos.** Its runs work in the video repository's checkout,
  where its projects and its studio live; its line is the worker line, and its role prompt, seeded from
  `prompts/roles/producer.md`, stands first in every run: what the studio is (the video-shotcraft
  skill bundle driving Remotion, rendering on the card under the card arbiter's ticket), how an item is made, how the file goes out and the studio's
  licence rules. The worker plugin's `ahas-video-shotcraft` skill carries the setup, the render and the
  checks.
- **Its bundles:** `queue.read` for its items, `web.read` for the pages and assets an item names, and
  `room.send` for the finished file, which goes into the item's room through the receiver's file send tool
  as a job's own material (the receiver's `docs/room-files.md`, 「A job's sends」). It holds no graph.

## The voyager and the seeker roles

- **`web.read`, `web.sites`, `web.branches` and `web.rulebooks` — the whole of the web namespace:** search
  and fetch, the site tools, the branch tool and the rulebooks, the rulebook reader standing in the research
  unit's namespace. The look at a picture is the eye service's `eye/see`, which the search and browse lines
  hold through their `eye` namespace (`policy/run-lines.toml`).
- **The voyager runs on the search line, the seeker on the browse line.** A session's caller names the
  role and narrows the bundles (`docs/api.md`, 「sessions」): an apart session keeps only the capabilities
  its start's `toolsAllowed` names, so the research unit decides whether a search, a browse, a round or a
  branch holds the branch tool and the rulebooks.
- **Neither carries a prompt:** a run on the search or the browse line opens on the harness opening and
  the instructions its caller hands in, and no role prompt is read there (`docs/launch-lines.md`, 「Search
  runner」). The voyager answers a question by searching; the seeker visits and reads the sites and pages it
  is named.
- **The search and browse lines hold the shared brain's writing tools** (`docs/launch-lines.md`, 「Search
  runner」): `brain_learn` and `brain_similar` of the `bmo` namespace, on a supervisor's brain: the brain of the
  supervisor that delegated the run, and for a run no supervisor delegated, the brain of the supervisor
  agency's `researchSupervisor` setting names (`docs/api.md`, 「settings」), none where it names none. The
  line's session record names that supervisor as the turn the brain tools run for, so a run writes what it
  establishes into the same brain the supervisor reads, while the run itself stays the role's.

## The translator and the transliterator roles

- **The translator is asked on demand** — a document's part, a text a job reads, a record the owner reads
  — and goes out on the translate line (`docs/launch-lines.md`, 「Translate runner」). Its prompt, seeded
  from `prompts/roles/translator.md`, stands before the instructions of the purpose it is asked for. It holds
  no bundle: no web, no tool.
- **The transliterator is the voice unit's,** asked for a slot under its name with the search line, and
  holds no bundle (the voice's `docs/boundary-api.md`). It carries no seeded prompt: the voice unit hands
  every session its own instructions, and the search line reads no role prompt.

## The upkeep role

- **Held by agency's own runs that stand for no one's work,** the usage reader's renewal turn among them. It
  gives no bundle and carries no prompt: what such a run holds of a role is one of its agents and its
  sessions setting.

## Co-workers

- **A co-worker is a run of a role, called by the role's name from another role's run** — a co-work order's
  branch, a search's branch, or a native call from a job's program — and is let in on one of that role's
  agents (`docs/role-slots.md`, 「Who takes a slot」).
- **Only a role with a callable agent is called,** and **an unknown role is an error:** a call naming a role
  the store does not hold, or one none of whose agents that are switched on is marked callable, is refused
  with the name, and nothing starts. A CLI default type such as `general-purpose` is no role and is refused
  the same way. The call is let in on a callable agent of the role and on no other.
- **The definitions a job's program is handed** are the roles with a callable agent, each with its
  description and the model and effort of the role's first callable agent on the provider that program stands
  on; a role with no such agent on that provider is not defined there, and the caller does that part itself (`docs/role-slots.md`, 「Native
  co-worker slots」).
- **The builder role is the work order's own turn,** never called as a co-worker: a co-work order's builder
  stage runs as `builder` in the order's checkout.

## A role's sessions

- **Whether a run goes on in the session an earlier run left is its agent's setting,** the agent row's
  `sessions`: `keep` or `fresh`. Nothing reads a line's name or a run's caller to decide it; a run whose
  agent cannot be told keeps none.
- **`keep`: a run resumes the session the last answered run of the same role left at the same path on the
  same provider and model, while that session is valid** — inside the cache lifetime the provider declares
  for the model, and on the prefix it was opened on — and opens a new one otherwise (`docs/sessions.md`,
  「A role's session」).
- **`fresh`: every run opens a session of its own,** which no later run resumes.
- **A changed setting holds from the next run;** a run already going keeps the session it started on.

## Which role a run holds

- **A job's run:** the role its item names, `worker` where it names none (`docs/repositories.md`, 「A job」);
  an item whose work changes a repository's code names `coder` (「The coder role」).
- **A delegated task:** the role the ask names, `doppelganger` where it names none (`docs/tools.md`, 「The
  delegate」).
- **A worker ask:** `worker` (`docs/tools.md`, 「The worker ask」).
- **A co-worker:** the role the calling run names.
- **A session a caller starts:** the role the start names (`docs/api.md`, 「sessions」): the research unit
  names `voyager` or `seeker`, the voice unit `transliterator`, a translation's caller `translator`.
- **A run standing for no one's work:** `upkeep`.
- **A role the store does not hold gives nothing,** and every one of these refuses it with the name.

## Where the role is read

- **The session record.** Every record is issued for the run and holds its line's capabilities with the
  bundles its role gives, read from the store as the record is issued (`mcp-host.md`, 「The session
  record」). A run names its role among its turn's variables, which a standing program's own environment
  does not hold, so the record of a program standing in a place for a run is issued from the program's
  environment with the run's variables laid over it. A run a supervisor asks holds that supervisor's record
  beside its role's (`docs/tools.md`, 「The delegate」).
- **The catalogue a call is served.** The system's own MCP server serves a record the whole catalogue where
  it holds `room.tools`, and otherwise the catalogue's tools whose bundles it holds.
- **The send guard,** for every sending tool and script.
- **The slot,** the agents of the run's role, read from the store as every run asks to be let in: the run
  is let in on one of them, which is its seat and its road (`docs/role-agents.md`, 「Which agent a run
  takes」).
- **The launch,** which reads the role's prompt from the store as it composes the run's prompt.
- **The console's role tab,** which lists, adds, changes and removes the roles, and shows each role's
  session history (`docs/api.md`, 「roles」).

## Changing the roles

- **`GET /roles`** lists every role the store holds, in the order a screen lists them, each with every field
  of its row, how many of its runs go now and how many wait; beside them, every bundle a role may give and
  every line a role may name (`docs/api.md`, 「roles」).
- **`POST /roles`** adds a role, and refuses a name out of the pattern or already taken, a bundle the policy
  does not declare, a line agency does not carry, and an
  order, a cap, a forced provider, a sessions setting, a compaction threshold or a callability, none of
  which a role holds. A role added holds no agent, so it runs
  nothing until one is added (`docs/role-agents.md`, 「Changing the agents」).
- **`PATCH /roles/{roleName}`** changes any field of the row, each judged as the add judges it.
- **`DELETE /roles/{roleName}`** removes a role with its agents, and is refused while a run of it goes or
  waits, and for a role the policy names as a default — `worker`, `doppelganger` and `upkeep`.
- **A role added or changed is read at its next use;** nothing running is stopped for it, and a run already
  let in finishes under the row it began under.
