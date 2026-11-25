**English** | [한국어](README.ko.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md)

# agency

The service that supplies, delivers and controls agents. Two axes run in it: the supervisors, who own rooms
and talk with people, and the roles, which execute work. A caller asks for a run of a role at a path; agency
lets it in under the role's cap, gives it the provider its role's order holds quota on, starts it, carries
its turns, reads what it says and ends it. herdr is one backend behind the session contract.

## Documents

- `docs/api.md`: the boundary contract, every route and event agency exposes or calls.
- `docs/agents.md`: the two axes, supervisors and roles, a run, and the one interface every agent is reached
  through.
- `docs/supervisors.md`: what a supervisor row holds and how one runner carries every supervisor's turns.
- `docs/agent-roles.md`: what a role row holds, the roles the policy seeds, their bundles and sessions.
- `docs/repositories.md`: what a repository row holds, what a run there is handed, a job and a pinned model.
- `docs/job-review.md`: how a job's reports are examined by its orderer, stage by stage, before the job ends.
- `docs/provider-order.md`: how a run gets its provider and model: the order, quota, falling over and waiting.
- `docs/role-slots.md`: how a role's runs are let in under its cap: the line, reservations, a role that
  follows its supervisor and the standing branch checkouts.
- `docs/sessions.md`: how supervisor sessions and role sessions are kept, reused, folded and replaced.
- `docs/keepalive-schedule.md`: when a supervisor's prefix is pinged warm.
- `docs/PERSONAL-DATA.md`: the personal data the repository keeps out, and the check that holds it.
- The rest of `docs/`: putting words into a running program (`agent-control.md`), the launch lines, the
  accounts and their fallback, usage windows, the registered models and model files, a repository's
  checkouts and workbench, the work nodes, the tools and the run records.

## Folders

| Folder | Holds |
|---|---|
| `crates/` | the Rust crates |
| `deploy/` | the service units |
| `docs/` | the canonical documents |
| `packages/` | the TypeScript packages, one Bun workspace |
| `screen-kit/` | what a personal app's screen is built with on a person's PC: bmo-ui's bundle build on the release the console runs |
| `policy/` | the fixed values agency runs by |
| `prompts/` | the prompts agency composes an agent's launch from |
| `skills/` | the worker skills the repository carries |
| `tests/` | every test |
| `tools/` | the tools agency provides to the tool host |

## Layout

| Path | Holds |
|---|---|
| `crates/agent-session` | the session contract: start, send a turn, read state, interrupt, fold, end, list |
| `crates/herdr` | herdr's socket client |
| `crates/agent-host` | the supervisor of the executions placed in panes |
| `crates/pane-head` | the process that stands inside a pane |
| `crates/harness-machine` | what the harness reads, writes and starts on a machine |
| `crates/harness-providers` | what each provider answers about a machine |
| `crates/node-link` | the link between machines |
| `crates/llm-gateway` | the front of the registered models |
| `crates/policy` | the readers of `policy/` |
| `crates/cache-siren` | the accounts' usage windows, their readers and the cache-lifetime judgement |
| `crates/client` | the crate `bmo-agency-client`: every body and member of agency's API as a Rust type, and one method for each route |
| `crates/store` | agency's tables, their migrations and the statements over them |
| `crates/tool-call` | agency's side of the tool host's call protocol, which its tools share |
| `crates/server` | the service: sessions, places, lines, the role slots, control routes, verbs; its binaries `agency-server` and `bmo-agency` |
| `tools/` | the tools agency provides to the tool host: the procedures and the rulebook reader |
| `packages/` | the TypeScript packages, one Bun workspace |
| `screen-kit/` | what a personal app's screen is built with on a person's PC: bmo-ui's bundle build on the release the console runs |
| `policy/`, `prompts/` | the fixed values and the prompts agency runs by |
| `deploy/` | the service units: agency's server, the agent host and the model gateway, which every deployment restarts, and herdr's, which no deployment of agency keeps |
| `testbed.toml` | the testbed's declaration: how the repository is tested, checked, built and deployed, into agency's own deploy tree under `~/.local/share/bmo-agency` beside the data root it shares, `~/.local/share/bmo/data` |
| `install.toml` | the install declaration: what installing agency on a fresh machine needs, which `bmo install` reads |
| `tests/` | every test |

## Commands

```sh
cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
(cd packages && bun install && bunx tsc --noEmit)
(cd tests/packages && bun test)
```

## Licence

The repository's own code is MIT; its text stands in `LICENSE` at the root. The `cache-siren` crate is
derived from an upstream project whose licence could not be confirmed, so it carries no licence
declaration and is not covered by the repository's MIT licence.

## Version

The version is the set's, in semver, read off the git tag when a build is made; the workspace manifest's
version is a placeholder that never moves.
