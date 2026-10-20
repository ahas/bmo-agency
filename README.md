# agency

The service that supplies, delivers and controls agents. A caller asks for a session of a role and a
model; agency places it, starts it, carries its turns, reads what it says and ends it. herdr is one
backend behind the session contract, and the co-worker pool hands out places under the limits each
provider and tier is given.

## Documents

- `docs/api.md`: the boundary contract, every route and event agency exposes or calls.
- The rest of `docs/`: sessions, places, launch lines, the co-worker pool, workers, accounts, the
  model registry and the links to other machines.

## Folders

| Folder | Holds |
|---|---|
| `.graph/` | the code dependency graph |
| `crates/` | the Rust crates |
| `deploy/` | the service units |
| `docs/` | the canonical documents |
| `packages/` | the TypeScript packages, one Bun workspace |
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
| `crates/store` | agency's tables, their migrations and the statements over them |
| `crates/tool-call` | agency's side of the tool host's call protocol, which its tools share |
| `crates/server` | the service: sessions, places, lines, the co-worker pool, control routes, verbs; its binaries `agency-server` and `bmo-agency` |
| `tools/` | the tools agency provides to the tool host: the procedures and the rulebook reader |
| `packages/` | the TypeScript packages, one Bun workspace |
| `policy/`, `prompts/` | the fixed values and the prompts agency runs by |
| `deploy/` | the service units: agency's server, the agent host and the model gateway, which every deployment restarts, and herdr's, which no deployment of agency keeps |
| `testbed.toml` | the testbed's declaration: how the repository is tested, checked, built and deployed, into agency's own deploy tree under `~/.local/share/bmo-agency` beside the data root it shares, `~/.local/share/bmo/data` |
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

The version is the workspace's, in semver.
