# Model tier configuration

The execution integration uses the tier store for primary launches and fallback attempts.
Co-work schemas and execution resolve configured tiers. The agents console edits shared mappings and work-key assignments through `/model-tiers`.
`Archive.modelTiers()` explicitly opens/migrates the new configuration; merely opening an
archive does not change live model settings.

The six tiers are `BMO` (the chat agent's tier; the name is a stored key of the tier tables and the
policy), Frontier, Reliable, Fine, Light and Simple. Provider routing and
provider enablement remain independent. A tier mapping contains one provider, model and
nullable effort. Work keys (including `role:<name>` and runner depth keys) select a tier.
Workers share `job-worker`; runtime agent observations are never configuration seeds.
An agent's row in the `agents` table carries no model. What an agent is configured to run on is
read as the tier store's answer — by agency's agent routes — for the agent's work key on
the provider its road begins on: the chat agent, each gate and each editor on its own key's road, every
worker on `job-worker` on the provider it is pinned to or else first in the provider order, and the searcher and the browser on their own
keys on the first enabled provider carrying their line. Agency's agent list (`docs/api.md`, 「Agents」), the console
screens built on it and the `bmo-agency agent` listing all show that answer, and an agent with no
work key, or one the settings refuse, shows none rather than a remembered value. The model a held
chat session runs on is that session's own road record, not the row either.

The initial representatives are bmo, job-worker, digest, role:digest, role:scout and
output-format respectively. `BMO` and Frontier inherit complete stored provider overrides.
Reliable uses editor high effort; Fine uses the co-work digest's Sonnet low effort; Light
uses the scout's Haiku low effort. Simple uses the small gate baseline. Codex Fine uses
Terra low, following the co-work digest effort, below Reliable high.
A tier's mapping for the registry's provider carries an answer-token ceiling beside its model and
its effort, and both are read by every key assigned to the tier (`docs/own-llm.md`, 「Settings per
tier」). Its effort is null, `thinking` or `no-thinking`, read by the gates, the chat, the workers and
the editors, where null and a null ceiling leave thinking and the ceiling to the registration the
mapping names; a gate's null ceiling is the gate's own default. Only those mappings carry a ceiling;
every other provider's is null. The registry's provider lists the registrations as its models, read
afresh from agency's store on every question, so a registration is choosable the moment it is saved,
and it is seeded on the registration the deployment's configuration names for its own engine, with
thinking off on every tier whose older own-engine mapping named none. A provider declared after the
first seed is seeded by a later seed of the plan: every tier mapped to what the plan names for it,
written once, the first time the store is settled with that provider in the plan, and recorded as
given, so a mapping the owner removes afterwards stays removed and one the owner set is never touched.
OpenRouter inherits the chat-agent and worker free choices when present; otherwise its initial choice
is `google/gemma-4-31b-it:free`, observed in the public catalogue by the inventory stage.
Catalogue presence is not account availability. No paid substitute is selected implicitly.
The later requested Codex Frontier Sol/max change is not a migration seed.

Migration is one immediate SQLite transaction and is versioned. All legacy route rows and
all effective legacy work-key pairs are retained as JSON evidence, including partial Codex
fields, route choices and the older own-engine answer limits. A partial Codex override is not an effective
pair. Conflicting owner model/effort overrides produce unresolved conflict rows; resolution
refuses that work key/provider until `assign` explicitly accepts its tier. Code/document
default differences adopt the shared standard under version 2, as described below. This is an intentional
review boundary, not an invisible per-agent concrete-model override. Compatibility reads
expose the snapshot for review; legacy model-write API actions are retired and return HTTP 410. Route-mode and provider-order controls remain available.
Legacy route model fields are migration evidence only. Fallback display and standing values resolve the tier store; unresolved or missing selections are unavailable rather than replaced with legacy defaults.
A gate whose provider is Laya reads no tier at all: Laya is not a provider of the tier store, no
mapping names a Laya model, and the gate is answered by the checkpoint the Laya service names for the
gate's own key (the gate unit's `laya.md`, 「A gate whose provider is Laya」). Its assignment stays and is read
again the moment the gate is put back on an LLM.
Assignments and mapping edits never erase this evidence. The final acceptance stage must
settle any remaining owner-override conflicts before publication. Shared tier edits affect every assigned work key on its next
resolution; already returned values and historical execution identities do not change.

Seeds run once, not on every read. Deleting a mapping leaves a missing mapping that fails
closed, rather than silently restoring a code default. New roles/work keys require an
explicit assignment. A work key that its fallback profile declares after the store was seeded
(the document viewer's `translator` and `document-seek` gates, for example) receives the assignment
its profile implies — Simple for a Haiku-default gate, Reliable for any other gate — once, the
first time the store is opened without it; an existing assignment is never touched, and a key no
profile declares still fails closed. Configuration writes validate names and provider-specific effort
syntax, then require caller-supplied model capability evidence for that exact pair. What only the
named provider can answer — which effort words a mapping of it may hold, whether a run of it may go
out with an effort or without one, whether a mapping of it carries an answer-token ceiling at all —
is asked of the provider, at the write boundary and again at the launch boundary. The tier store
itself validates what holds whoever the provider is: the tier, the provider's membership, the shape
of the model identifier and the ceiling being one of the offered sizes. It stands below the
providers and reaches none of them.
Catalogue unavailability prevents new selections; it does not erase existing stored
settings or prevent reading them. Launch-time provider eligibility and harness capability
checks remain mandatory at the launch boundary. A nullable effort means no
explicit effort; unsupported effort must never be silently discarded. A gate's timeout lives in
`gate_options`, and its thinking and answer limit come from its tier, never below the gate's own
floor.

## Execution integration decision

Schema version 2 settles differences inherited only from code/document defaults by adopting
the common tier standard. The old pairs remain in accepted conflict records: for example,
Claude Reliable gates now use high rather than medium, and Frontier runner depths share
high rather than separate medium/high/max values. This is an explicit normalization needed
for one mapping per provider/tier, not a claim that all previous pairs were identical. Stored
owner model overrides that disagree remain unresolved and fail closed; they are not erased.

A consumer that walks the providers itself asks agency for the mapping a work key goes out on at a
tier, on one provider or the first that serves it, under the same refusals a run meets (`docs/api.md`,
「Models, registry, tiers, roads and fallback」).

Each fallback attempt resolves before session lookup. The requested tier stays fixed across
the walk, while a new provider uses its current mapping. Worker recovery carries a complete
selection to the physical attempt. Execution and job history retain tier and effective effort;
unknown native-child settings stay null. Configuration refusal is not a provider outage.
An OpenRouter mapping names no effort, since which levels a routed model offers is not known to the tier, and a run of it records the effort an agent's own level sent, or none (「An agent's own thinking and effort」 below).

## Co-work compatibility

New plans select an abstract tier in `model`, or null for the DB role assignment. Each
task retains `tier` independently of its last physical `model`. Before a pending task
launches, legacy concrete values are copied to `legacyModel` and its role assignment is
adopted. Running, completed and historical tasks are never migrated in place. Reruns
retain an already selected tier; editing a pending tier explicitly replaces it. Planning
and synthesis read role assignments from the archive. Common co-worker role rows and worker
command definitions resolve from that store; document front matter seeds a new archive once
and is not a second live configuration source.

Branch prefix digests include effective effort. A co-work reservation for a task whose explicit tier differs from the role assignment reserves, counts and warms under the task's tier, and the task launches with that tier. Keepalive and compaction record the effort of the session they ran.

## Common co-worker capacity

- **Co-worker tiers.** A co-worker is borrowed from the pool only at Reliable, Fine, Light or Simple.
  `BMO` is the chat agent's own tier and Frontier the job worker's: the builder stage is the worker itself, never a
  borrowed co-worker, so neither tier has a pool limit, and a builder turn is bounded by the number of
  workers rather than by a count. Both keep their model mappings like every other tier.
- **Only a provider that lends co-workers.** Each provider declares beside its harness whether
  co-workers are borrowed on it; no list of providers is kept for the pool.
  - A provider offering several models — Claude Code, Codex, OpenRouter — lends them, each at its
    own tier.
  - A provider that is a single model on its own engine lends none, since nothing on it is a
    co-worker apart from the run itself: the registry's provider, every registration of which is one
    such model (the deployment's own engine among them).
  - The pool shows, counts, admits and warms only the providers that are turned on and lend
    co-workers. A turned-off provider's limits are kept and hold once it is on again.
- **A child never lands on a single model.** A selection naming one is passed over for a child
  exactly as a turned-off provider is, and the child takes the first enabled provider in the order
  that lends co-workers; a caller naming one outright is refused. A worker's own turn is the worker
  itself, not a child, and stands on whatever its selection names.
- **A run on a single model opens no co-worker.** A worker pinned to one does that part itself: the
  native co-worker gate refuses its `Agent` call and the reservation command refuses its reservation
  (`docs/coworker-pool.md`, 「Native co-worker places」), as a research run set off on a registration
  searches more itself instead of opening branches (`research-runs.md`).
- **The limit.** `coworker_pool_tier_limits` answers how many children of one co-worker tier may be
  alive on one provider. A limit is keyed by provider and tier, never by the model the tier currently
  resolves to, so editing a mapping or a role's assignment never leaves a request without a limit, and
  two tiers that resolve to the same model each keep their own count. Every provider and co-worker tier
  has a row, a turned-off provider's included, and no action removes one; capacity 0 closes that tier
  on that provider. The store holds no row for `BMO` or Frontier.
- **Limits start** at 12 for Light and Simple, the Haiku class, and 6 for Reliable and Fine, the same
  on every provider.
- **The request snapshot.** The queue copies the resolved provider, model, effort, the tier and the
  capacity into its durable request before launch: the model and effort are that request's launch
  identity, the tier is where it waits and counts, and later mapping or capacity edits affect only new
  requests. A request admitted under a model its tier no longer resolves to stays counted under its
  tier, and the status names the model and capacity it holds, until it is terminal.
- **Independent resolution.** A pooled request never inherits the parent run's model or reads the
  `job-worker` mapping to choose its provider; shared provider order and the role's own work key are
  the two independent inputs, and an explicit task tier replaces the role's assignment for both the
  model and the count.
- **Roles.** `coworker_pool_roles` is the role catalogue. Its initial rows come from
  `prompts/agents/worker/coworkers/*.md`; after migration, edits survive reopening. The role row contains no
  model field: its `role:<name>` assignment and the provider/tier mapping are the only model source,
  and the assigned tier is the only thing that decides a role's places. The pooled flag says whether a
  role may be dispatched as a child — a co-work branch, a research branch or a native co-worker.
- **The builder role.** `builder` is the required control role that is never dispatched as a child.
  Its assignment may name any tier, since it chooses the worker's own model. A builder task a plan
  puts in a stage before the build is the exception: it runs at the queue policy's
  `stage_builder_tier` rather than this assignment, which the job unit names in the request it
  enqueues (`docs/coworker-pool.md`). Its co-work turns take a
  place in the pool keyed by the worker's checkout and are never counted against a tier limit
  (`docs/coworker-pool.md`, 「Prior requests and yielding requests」).
- **Every other role is a co-worker.** Its assignment names a co-worker tier; the tier settings refuse
  `BMO` or Frontier for it, and the pool refuses a child request or a reservation at either tier
  whatever asked for it. Removing a side role removes its live `role:<name>` assignment in the same
  transaction; queued and completed requests retain their snapshots.
- **The console** edits the limits and the reset through agency's pool routes (`docs/api.md`,
  「Co-worker pool」), on
  the providers the pool shows and on no other; it never edits a role's model through the pool. The reset sets every limit back to its starting value
  and removes nothing, and admitted snapshots are untouched.
- **The older table.** The store also holds `coworker_pool_limits`, keyed by provider, model and
  effort, which admission never reads. The tier table was built from it once: every row starts at its
  tier's starting value, and a row whose capacity differs from what the role seeds give that bucket
  under the assignments of that moment — an owner's value — is carried to every co-worker tier of that
  provider whose mapping is exactly that bucket. The older table and `coworker_pool_meta` stay as they
  are, so a build that still reads them opens the same store unchanged.
- **Stage width is not a model limit.** A co-work stage holds as many planned tasks as its design asks
  for, while the allocator admits only the FIFO head requests that fit their tier's limit on their
  provider. Waiting is outside a task's execution clock. Standing branch checkout rows describe
  reusable prompt-cache locations and do not grant model capacity.
- **No in-process fallback for a branch.** A pooled branch never performs an in-process provider
  fallback under its old lease. Its supervisor records that attempt's terminal failure, gives back the
  old provider's place and enqueues on the replacement provider as a new attempt; ordinary worker jobs
  retain their existing fallback walk.

## An agent's own thinking and effort

- **Two settings of the agent's own, over its tier.** Every agent that runs a line with tools — the
  chat agent, each worker, the two editors, the searcher and the browser searcher — carries on its `agents`
  row whether it thinks (default, on or off) and its effort (default or one level). Default leaves the
  run exactly
  as its tier mapping and its line make it, which is where every agent stands unless the owner or a
  seed set it otherwise.
- **The levels are one scale:** `low`, `medium`, `high`, `xhigh` and `max`. An agent names a level
  once, whichever provider its run lands on, and the provider turns it into its own word.
- **Each provider answers for itself**, declared beside its harness, never in a table of providers:
  which levels a run of one model takes, whether the thinking switch reaches its runs, and the effort
  word a run goes out with.

  | Provider | Levels | Thinking off | Thinking on |
  |---|---|---|---|
  | Claude Code | those the model's catalogue entry names, every level where it names none | `MAX_THINKING_TOKENS=0` | the line's own |
  | Codex | `low` to `max`, as `model_reasoning_effort` | effort `low` | the line's own |
  | OpenRouter | `low` to `xhigh`, as the request's `reasoning.effort` | reasoning off | the line's own |
  | Registered models (the deployment's own engine among them) | none | effort `no-thinking` | effort `thinking` |
- **A level the run's model does not take leaves the tier's effort** for that run, since an agent's
  level is one word across providers whose models differ. The console names, for the provider the
  agent begins on, the levels that reach it, and says so where none do or the thinking switch does not.
- **It is applied wherever a tier mapping is laid on a run:** the first launch, every attempt of the
  walk, a pinned worker's standing program and a job's attempt on its own provider. What the run went
  out with is what its record, its session identity and its prefix digest carry, so a changed effort
  selects another session or prefix exactly as a tier edit does.
- **A support worker runs on its base's settings,** as it runs on its base's pin.
- **Gates are not in this list.** A gate's thinking and effort are its tier's and its own
  profile's (`docs/own-llm.md`, 「Settings per tier」), since a gate is one judgement asked under a
  time limit rather than an agent that works.
- **Changed in the console's general tab** (through `docs/api.md`, 「Agents」) **or with**
  `bmo-agency agent --reasoning <agent> [--thinking on|off|default] [--effort <level>|default]`;
  the change reaches the agent's next run.
- **Seeds.** As the two columns arrive, the agents the deployment's configuration names are given
  thinking on, and every other agent stands at default.

## Research runs

- **A depth chooses a tier.** Each search depth and the browse runner has its own work key, and that
  key's tier assignment is its tier; any tier may be chosen. The model and the effort both come from
  that tier's mapping on the provider the run launches on — the research profile carries neither
  (`research-runs.md`).
- **The provider** is the first enabled provider in the order whose program carries the run's line;
  a provider that cannot carry it is passed over.
- **The effort is checked before launch** against the resolved provider and model: where the Claude Code
  catalogue names the efforts a model accepts, an effort outside them refuses the run rather than handing
  the CLI a value it would silently lower, and where the catalogue names none for that model the
  harness's own effort words stand.

## The Claude catalogue

- **The list is not written in this repository.** Which Claude models a mapping may name is read from
  the catalogue the Claude Code CLI keeps under the configuration directory of the login a run would
  go out on, which the CLI fetches and refreshes on its own as it is used. A model the CLI has begun offering can
  be chosen here without a deployment, and no list of Claude model names is held anywhere in this
  repository to be kept in step with it.
- **The efforts are read from the same entry.** A model's catalogue entry names the efforts it accepts,
  and a mapping of that model is validated against those words rather than against the shape of the
  model's name.
- **Absence of evidence refuses nothing.** A model whose entry names no effort — one that does not think
  — and a model the catalogue does not name at all both take whichever effort words the harness itself
  declares, since a name missing from a list is not evidence against it. Where an entry does name
  efforts, those are the only ones accepted for that model, and that is where the refusal of an effort
  the CLI would otherwise silently lower belongs.
- **In service and superseded.** The catalogue separates the generation it currently serves from the
  ones it has superseded, and both are offered: a mapping that names a superseded model is read, shown
  and can be saved again, and the mark beside the name is what says which of the two it is.
- **A stale read is still an answer.** The catalogue carries when it was fetched and when it goes stale.
  Past that mark it is served as it stands — the screen keeps its list instead of emptying — and the
  answer says it is the last read and when it was taken, so nothing shown is taken for fresh.
- **A catalogue that cannot be read refuses,** exactly as Codex, OpenRouter and the own LLM services refuse: no file
  where one is expected, nothing that parses, or a catalogue naming no model at all is a listing failure
  rather than an empty list, and it reaches the screen as the same refusal their failures reach it as.
- **It is read from the login the walk would take first,** which is the login a run naming none of
  its own goes out on, and whose CLI therefore refreshes the file as agency uses it. A login that keeps
  no configuration directory of its own is read from the machine's, and so is the question asked
  where no store of logins is installed to put it to.

## Console contract

The agents page separates provider routing from model configuration. The shared mapping form
selects a provider, tier, registered model and supported effort, shows all affected work keys,
and saves the model/effort pair atomically. Disabled providers can be configured without enabling
them. A catalogue failure preserves saved values and drafts but prevents a new mapping save.
The control server rechecks the catalogue at save time; browser-supplied capability evidence
is never trusted. Claude Code uses the catalogue its CLI keeps on this machine;
Codex uses its installed catalogue; each own LLM service uses its one selectable service model. OpenRouter supports no explicit effort on this harness.

Each work key, runner depth and co-work role has a tier assignment. Workers share job-worker.
A gate on Laya shows its checkpoint in the tier cell instead of a tier, and the cell cannot be
pressed until the gate is put on an LLM again.
The assignment form shows effective selections for every provider, including disabled providers,
missing mappings and unresolved migration conflicts. Accepting a conflict requires an explicit
review checkbox and saves the selected tier for the work key, accepting all its provider conflicts.
The old selections remain visible as migration evidence. A mapping edit alone accepts no conflict.

Changes apply at the next physical resolution, including a retry on a different provider; an
active attempt retains its resolved values. Model/effort changes can select a different session
or prefix; retained-session maintenance keeps that session's configuration. Old execution records
are observations, not settings. The manual job/Builder concrete chooser is retained only as a
labelled compatibility control for existing job requests; shared agent defaults live here.

OpenRouter defaults prefer the preserved free choices, then the free seed above. Free catalogue
entries can disappear, be rate-limited, lack tool/context support, or be inaccessible to an account.
A listing is not a successful provider run. The console defaults its list to free entries and
requires an explicit filter change to list paid entries; no paid replacement is automatic.

Editor warmup groups prefixes by resolved model and effort, passes the actual editor work key to the resolver, and skips unresolved mappings. Provider disablement between groups does not increment counters for a skipped group.

Codex and OpenRouter keepalive skip a retained session whose tier configuration is unavailable or has changed. They preserve its saved identity and continue warming other eligible sessions.

The console retains an unsupported saved effort in the draft and requires an explicit supported choice; loading a catalogue or changing models does not silently replace it. Assignment previews label disabled providers.

The agent registry CLI retains provider order, enablement and route-mode operations. `bmo-agency agent --route ... --model ...` is retired and refuses without changing the route; model and effort edits use the shared tier settings.
