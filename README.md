# agency

The service that supplies, delivers and controls agents. A caller asks for a session of a role and a
model; agency places it, starts it, carries its turns, reads what it says and ends it. herdr is one
backend behind the session contract, and the co-worker pool hands out places under the limits each
provider and tier is given.

## Documents

- `docs/api.md`: the boundary contract, every route and event agency exposes or calls.
- The rest of `docs/`: sessions, places, launch lines, the co-worker pool, workers, accounts, the
  model registry and the links to other machines.

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
| `crates/server` | the service: sessions, places, lines, the co-worker pool, control routes, verbs |
| `ts/` | the TypeScript packages, one Bun workspace |
| `policy/`, `prompts/` | the fixed values and the prompts agency runs by |
| `deploy/` | the service units |
| `tests/` | every test |

## Commands

```sh
cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
(cd ts && bun install && bunx tsc --noEmit)
```

The version is the workspace's, in semver.
