# Repositories

A repository is where executing work happens: a code tree with its checkouts, and what is that code
tree's own to hand a run working in it. It is a place, never an agent (`docs/agents.md`, 「Two axes」):
no repository owns a role, a session or a program, and any role may run in any repository's path. How a
repository runs several jobs at once is `docs/worker-slots.md`; what a job holds outside its turns — the
preamble, the workbench, the dev server — is `docs/worker-bench.md`.

## What a repository row holds

- **One row of the `repositories` table a repository,** its whole declaration:
  - **its name,** the key every reader finds it by, the small alphabet a URL, a unit's name and a lock
    file's take without quoting (`policy/workers.toml`); and **its label**, what a screen shows it as;
  - **its owner,** the login account it belongs to, or none for the administrator's;
  - **its path,** the base checkout, the directory its jobs work in, which every add names: agency makes no
    checkout and no directory for a repository; and **its node,** where the base checkout stands on a work
    node that takes jobs rather than here (`docs/work-nodes.md`, 「Which nodes take jobs」);
  - **its checkouts:** how many support worktrees stand beside the base, and the checkouts it keeps on the
    work nodes that take jobs, set as one list (`docs/worker-slots.md`, 「Checkouts on the nodes」);
  - **its prefix documents,** the paths of the documents a run's prefix carries in this repository, in
    order (`docs/worker-bench.md`, 「Prefix documents」);
  - **its preamble,** the owner's text that stands before every job's prompt in this repository
    (`docs/worker-bench.md`, 「Preamble」);
  - **its rules,** the repository's own rules a run in it is given after the worker line's standing rules,
    from the prompt folder its row names under `prompts/agents/` or none;
  - **its dev command,** the one line its jobs' dev server is started with (`docs/worker-bench.md`, 「Dev
    server」);
  - **its pinned model,** a provider and one model of it, or none (「A pinned model」 below), with the
    sampling a run on a pinned registration is sent: the supervisors' generation settings by the same names
    and bounds, less the answer ceiling, each held or left out; one left out is the engine's own default,
    which agency neither sets nor reads (`docs/own-llm.md`);
  - **its downgrade heal,** on unless the owner turns it off, whether a job that fell to a lower model is
    launched on its own model again (`docs/cache-fallback.md`, 「A repository the owner keeps on its fallen
    model」).
- **Every repository is a row alike,** the platform's own services and the owner's other repositories —
  one kept outside the platform's tree among them. Nothing is built in: the operations repository is a row
  like any other.
- **A repository is added, changed and removed on the console's repository tab** or through the repository
  routes (`docs/api.md`, 「repositories, prefix-checks and room-holdings」), with no restart; a removal is refused while a job holds one of
  its checkouts. From a shell it is `bmo repository remove <name>`, which asks the same route and prints what
  it removed or refused; the checkout and the deploy tree on disk stand as they were.
- **A repository keeps no log of its own.** It is a place, so what ran in it is its jobs' runs, each with its
  own log; its jobs are listed by repository, newest first, and a screen shows the log of its job going, else
  of its latest.

## What a run in a repository is handed

- **A run working in a repository's checkout is handed the repository's own,** whatever its role:
  - its system prompt opens on the platform's harness opening and the role's prompt, then the worker line's
    standing rules, the repository's rules and its prefix documents, each headed by its path;
  - a job's prompt opens with the repository's preamble, then the item.
- **What the repository hands is part of the prefix,** so a changed prefix document, rule or preamble moves
  the prefix of every session kept at that path, and the next run there opens a new session
  (`docs/sessions.md`, 「A role's session」).
- **A run whose path is no repository's** — a delegated task in a supervisor's scratch folder, a search, a
  translation — is handed nothing of a repository.

## A job

- **A job is one run of an item in a repository.** The item names the repository its work belongs to and
  may name the role it runs as; where it names none, the role is `worker` (`policy/roles.toml`). Any role the
  store holds may be named, free choice, and the run holds that role's prompt, bundles, order and cap. An
  item whose work changes a repository's code names `coder`; one that names no role stays `worker`
  (`docs/agent-roles.md`, 「The coder role」).
- **A job goes out when two things are free:** a slot of its role (`docs/role-slots.md`) and a checkout of
  its repository (`docs/worker-slots.md`, 「The queue with support checkouts」). Jobs on three repositories
  whose role is capped at two run two at a time, each in its own repository's checkout on the role's
  provider and model; the third waits for a slot.
- **It runs on its role's active provider:** the provider the owner forced on the role, else the first of
  the role's order with quota above the role's share (`docs/provider-order.md`), unless its item names a
  model, or, where the role has no forced provider, its repository pins one (below).
- **It goes on in a kept session** where one is valid for its role at that checkout on that provider and
  model, under the role's session policy (`docs/sessions.md`, 「A role's session」), and its runner may hand
  the session over as the job ends.
- **Its checkout, its branch and its landing** are `docs/worker-slots.md`'s; its workbench and dev server
  are `docs/worker-bench.md`'s.

## A model an item names

- **An item may name the provider and model its job runs on** with its label `model=<provider>/<model>`,
  the model the item was filed to run on: a provider agency carries whose program carries the worker line,
  and a model it holds — the values a repository's pin takes, judged the same way (「A pinned model」 below);
  a registered model is `registered/<name>`. The provider is what stands before the first `/`, the model all
  after it. A `model` label that is `auto` or empty, or none at all, names no model, and the job goes out as
  「A job」 above says.
- **It carries that item's job alone,** whatever role it runs as: the run goes out on the named provider
  and model instead of its role's active provider and instead of its repository's pin, at the effort the
  role's order names for that provider where it names one, else the provider's own default.
- **A named model that is unavailable makes the job wait,** as a pin does: it is held with the reason and
  goes out once the named provider can answer again, and it never falls back to another provider or
  model, since the item chose it.
- **A named model agency does not hold refuses the launch** with the label's text, and the item's orderer
  is told once (`docs/agent-control.md`, 「An item's change is told to its orderer」); a label that does not
  read as a provider and a model is refused alike.
- **The job holds a slot of its role,** so the role's cap counts it like any other run.
- **What chose a job's provider and model is the first of four that stands:** its item's `model`, else
  its role's forced provider (`docs/provider-order.md`, 「A provider the owner forces」), else its
  repository's pin, else its role's order. The job says which (`chosenBy`, `docs/api.md`, 「assignments and
  jobs」).

## A pinned model

- **A repository may pin a model:** a provider and one model of it — a model the Claude CLI's catalogue
  names on Claude Code, a member of a provider's own road, or an offered registration on the registry's
  provider (`docs/provider-order.md`, 「Models a provider holds」).
- **A pin carries every job in the repository whose item names no model and whose role has no forced
  provider,** whatever role the job runs as: the run goes out on the pinned provider and model instead of its
  role's order, at the effort the role's order names for that
  provider where it names one, else the provider's own default.
- **A pinned model that is unavailable makes the repository's work wait.** Where the pinned provider has no
  quota above the role's share, cannot answer now, or the pinned registration's engine is down, the job is
  held with the reason and goes out once the pin can answer again; it never falls back to another
  provider or model.
- **A pinned job still holds a slot of its role,** so the role's cap counts it like any other run.
- **The pin is set and cleared on the repository's row;** the next job reads it.

## The words said to a repository

- **A supervisor's worker ask** runs a `worker` run in the folder it names, the asking supervisor's scratch
  folder where it names none; a repository's checkout may be named only in the owner's turn, and the run is
  then handed that repository's own (`docs/tools.md`, 「The worker ask」). It takes a slot of the worker role,
  and a repository's checkout like a job, and waits for both however long it takes.
- **A person typing into a run's pane** speaks to that run while it stands; between runs nothing of the
  repository stands to be spoken to.
