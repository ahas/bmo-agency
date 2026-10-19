# cache-warmer

The keepalive schedule's general rule: when a kept prompt-cache prefix is due a ping, and in what order a
pass pings its targets. A provider keeps a prefix for a while after its last touch and then drops it; a
consumer that wants a prefix read warm rather than rebuilt pings it once it has stood idle long enough and
before the provider lets it go. This crate answers when that is and in what order, and nothing else.

It is a library of plain values with no dependencies. It reads no clock, no store and no policy: every
moment and span is handed in, in epoch milliseconds, and every answer is a value. The canonical statement
of the rule is agency's `docs/keepalive-schedule.md` (「The general rule」), and its public names are part
of agency's boundary, `docs/api.md` (「The keepalive schedule library」).

## The rule

- A target's **touch** is the latest of its last use, its last warmed answer and, where its registration
  says failures touch, its last failed attempt. Its **idle** is now less its touch. Its **idle stretch**
  is its own, or the pass's idle override where the pass carries one and the target honours it.
- Its **usable lifetime** is its lifetime less its margin; a lifetime no larger than its margin leaves
  nothing.
- It is **due by its clocks** when it has no touch at all, or its idle has reached its idle stretch, and
  in either case its prefix is still there: below the usable lifetime where a lifetime is declared. A
  target never used and never pinged is due at once unless its lifetime leaves nothing after the margin;
  a target that declares no lifetime is due by its idle stretch alone.
- A **candidate** is due in a pass when it is on, not paused, the round is not stopped and its clocks
  make it due.
- A **pass's plan** goes by rank, lowest first, then consumer, then target name. A group, keyed by its
  consumer and its name, goes together where its first target stands; of its due targets the oldest touch
  goes first, one never touched oldest of all, up to the group's limit. A stopped round plans nothing.

## The public API

Every name stands at the crate's root.

- `Clocks`: a target's registration, with `idle_ms`, `lifetime_ms` (none where no lifetime is declared),
  `margin_ms`, `failure_touches` and `honours_idle_override`.
  - `touch` of a `Record`: the moment idle is measured from, none for a target never touched.
  - `idle_stretch` under an optional override.
  - `due`: whether a record makes the target due at a moment, under an optional override.
  - `due_at`: when the target is next due, judged without an override; the moment handed in for a target
    never touched and due, none where its prefix is gone or its stretch falls past its usable lifetime.
  - `kept`: whether a prefix idle a given span is still there.
- `Record`: a target's `last_used_at`, `last_warmed_at` and `last_failed_at`, each optional; its default
  is a target never used, warmed or failed.
- `Group`: a group's `name` and its `limit` of pings a pass; a limit of 0 pings none of it.
- `Candidate`: one target as a pass judges it, with `consumer`, `target`, `rank`, an optional `group`,
  its `clocks` and `record`, whether it is `on` and whether it is `paused`.
  - `is_due` in a `Pass`.
  - `due_at` for a list: none while it is off or paused.
- `Pass`: what one pass judges by, `now`, an optional `idle_override_ms`, and whether the round is
  `stopped`.
- `plan`: the candidates a pass pings, as indexes into the slice handed in, in the order it pings them.
- `usable`: a lifetime less a margin, none where nothing is left.
- `is_on`: whether a target warms, from its subject's switch where thrown, else its registration's
  default, else the policy's default.
- `use_clears_pause`: whether a use whose work began at one moment clears a pause set at another; only
  work begun after the pause does.
- `first_units`: a text cut to a count of UTF-16 units, the way a script counts a string; a surrogate
  pair the cut splits ends as the replacement character.

## Depending on it

The crate lives in the agency repository under `crates/cache-warmer`. Another repository depends on it by
git, naming the package `cache-warmer` at the agency repository's URL, a `file://` URL to a local
checkout where the repository has no remote, and pinning a `rev` or a `tag` where it wants one commit
held. The command that writes that dependency is:

    cargo add cache-warmer --git <agency repository URL>

A checkout standing beside agency's may name the crate by `path` instead. Either way it builds alone: no
other crate of agency's is fetched beside it, and it carries agency's version.

What stays the caller's: the clock, the store a target's record and switches are kept in, the policy its
defaults and margins come from, the ping itself and what its answer writes.

## Tests

The rule's tests stand in agency's `tests/cache-warmer/` and are its contract; run them from the agency
repository with `cargo test --test cache-warmer`.
