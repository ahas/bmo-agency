# Working in this repository

This repository is agency: the service that supplies, delivers and controls agents. It starts agent
sessions, carries turns into them, reads their state and output, interrupts, folds and ends them, keeps
the co-worker pool, the workers, the accounts and logins, the model registry and the links to other
machines. It decides nothing about a turn's content: no turn assembly, no room, permission or rule
judgement, no reference selection and no output check. Which role and which model a piece of work needs
is the caller's decision; agency supplies it.

## Principles

- **Each service runs on its own,** with no context from another repository.
- **Services talk only through documented APIs.**
- **References run one way:** the base never knows what is below it, the lowest service owns a shared
  policy value, and each repository holds only what it uses.

## Rules

- **`docs/` is canonical.** `docs/api.md` is the boundary contract, every route and event agency
  exposes or calls. Read the document covering a change first. A change that would make it wrong
  repairs the document first, stating what is true now, and the code is then written to match. A change
  to the boundary API changes `docs/api.md` in the same commit and follows the version rule below.
- **herdr is one backend.** `crates/agent-session` declares the session contract, eleven operations:
  start, switch a session's provider and model, send a turn, push words into a working turn, read state,
  answer the question a turn stands on, read what the latest turn has written so far, interrupt, fold,
  end, list. Every caller reaches agents through it. `crates/herdr` is only herdr's socket client; the
  herdr implementation of the contract is the server's `herdr_backend` module, since it is built over
  the server's door, places and lines.
- **The co-worker pool is a module of the server, not a crate.** Its dispatch shares the request, the
  tiers, the roads and the routes with the control routes, so a crate boundary between them would be a
  cycle.
- **No other service's code.** Among other repositories only the base libraries `bmo-chat`,
  `bmo-chat-client` and `bmo-ui` may be depended on or imported, and the Rust side may also build on the
  published clients and libraries of the upstreams it reaches (`docs/api.md`, 「Calls agency makes」);
  `tests/dependencies.rs` and `tests/packages/dependencies.test.ts` fail on any other. Another service is
  reached through its API; a call upward is an event agency publishes or a hook the consumer registers,
  with a default that does nothing; data a higher service holds is handed in by the caller.
- **A variation belongs to its member.** A value that differs per provider, model or consumer is
  answered by that member, never by a central table in the layer below.
- **No hand-written parser for a general format.** TOML, JSON, YAML, markdown, shell lines and command
  lines are read by a maintained library; where none fits, the reason is written beside the code.
- **Tests live in `tests/`.** The root package holds them: a group of tests is a directory under
  `tests/` with its own `main.rs`, the TypeScript tests stand under `tests/packages/`, and the checks that
  span the repository are files at the top of `tests/`. No test module sits in a source file.
- **English only** in code, comments, strings, tests and documents, and in commit messages. Korean stands
  in `prompts/` alone, where it is what the agents are told and answer in; `tests/literals.rs` fails on
  Hangul anywhere else. Messages a service shows are English sentences; machine-read outcomes are status
  codes and kebab-case codes.
- **No names of agents, owners or hosts, and no personal paths** in code, prompts, documents or commits.
  Write the role instead: the owner, a worker, a node. A path is written from `$HOME` or from a variable.
  `tests/literals.rs` fails on the account name, the host name and the committer's address of the
  machine it runs on.
- **No personal data**, as `docs/PERSONAL-DATA.md` states it: not a name, handle, account id, mail
  address, host name, home path, device or other network detail of the owner or of a person they know.
  A value that differs by installation is read from settings; an example is neutral.
  `tests/personal_data.rs` fails on a mail address, a home path, a telephone number, a token and a
  private network address in any tracked file, naming the file, the line and the kind but never the value.
- **No directory named after its parent.** `tests/layout.rs` fails on one.
- **Only Cargo manifests at the root,** beside `testbed.toml`, the testbed's declaration. The root
  `Cargo.toml` is the workspace and the root package, which is never published and exists to hold
  `tests/`. The TypeScript packages are a Bun workspace of their own under `packages/`. `tests/layout.rs` fails
  on another build tool's manifest at the root.
- **Documents state what is true now**: no change history, no quoted source and no file-and-line pins.

## Layout

- `crates/`: the Rust crates. `agent-session`, `herdr`, `agent-host`, `pane-head`, `harness-machine`,
  `harness-providers`, `node-link`, `llm-gateway`, `policy`, `store`, `tool-call`, `cache-siren` (the
  accounts' usage windows and the cache-lifetime judgement, `docs/usage-windows.md`) and `server`.
- `tools/`: the tools agency provides to the tool host, `procedures`, `worker_ask` and `delegate`, each a folder
  with its `tool.toml` and the crate of its executable (`docs/tools.md`).
- `packages/`: the TypeScript packages, `agent`, `archive` and `support`, one Bun workspace.
- `policy/`: the TOML files agency's fixed numbers and words stand in, read by `crates/policy`.
- `prompts/`: the prompts agency composes an agent's launch from.
- `docs/`: the canonical documents.
- `deploy/`: the service units. agency's server, the agent host and the model gateway carry the restart
  mark; herdr's carries none, since herdr is a runtime agency stands on.
- `testbed.toml`: how the testbed tests, checks, builds and deploys the repository, the tools' placement
  among it.
- `tests/`: every test.

## Checks

- Static: `cargo check --workspace --all-targets` and, in `packages/`, `bunx tsc --noEmit`.
- Review: `cargo fmt --all --check` (108 columns, 2-space indentation, as `rustfmt.toml` sets) and
  `cargo clippy --workspace --all-targets -- -D warnings`.
- Tests: `cargo test --workspace`, and `bun test` for `tests/packages/`.
- The version is the workspace's, and it moves only at a release the owner declares by name (bmo-chat's
  `docs/conventions.md`, 「Versions」): a commit, a deploy, a fix or a contract change moves none, and a
  deployment is told apart by its commit. A sibling is a path dependency and is never pinned to a version.
