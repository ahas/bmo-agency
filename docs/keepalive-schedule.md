# The keepalive schedule

When a kept prompt-cache prefix is worth a ping, and whether agency's keepalive pings go. The general
rule is the `cache-warmer` crate, a library of plain values with no dependency of its own, which agency
and any other repository that keeps prefixes warm build on (`docs/api.md`, 「The keepalive schedule
library」). Agency's own schedule stands in the server's keepalive module and calls it: the owner's
warming switch and the co-worker pool's rule for its places. The pool asks it before it judges a
reservation's places and again before each ping goes (`docs/coworker-pool.md`, 「Reserving places and
keepalive」); which places stand, the ping itself and its record stay the pool's.

## The general rule

The crate judges only what it is handed, `now` included: it reads no clock, no store and no policy of
its own. Every moment and span is in epoch milliseconds.

- **A target's clocks** are its registration's: its idle stretch, an optional lifetime, the margin
  taken off that lifetime, whether a failed attempt counts as a touch, and whether a pass's idle
  override applies to it.
- **A target's touch** is the latest of its last use, its last warmed answer and, where failures
  touch, its last failed attempt. Its **idle** is now less its touch. Its **idle stretch** is its own,
  or the pass's override where the pass carries one and the target honours it.
- **The usable lifetime** is the lifetime less the margin. A lifetime no larger than its margin leaves
  nothing.
- **A target is due by its clocks** when it has no touch at all, or its idle has reached its idle
  stretch, and in either case its prefix is still there: where it declares a lifetime, its idle is
  below the usable lifetime. A target never used and never pinged is due at once unless its lifetime
  leaves nothing after the margin; a target that declares no lifetime is due by its idle stretch alone.
  A prefix already gone is left alone, since pinging it would rebuild it at the cost of the work it
  waits for.
- **When it is next due,** judged without an override: now for a target never touched and due, its
  touch plus its idle stretch while its prefix is still there and the stretch falls inside the usable
  lifetime, and none otherwise.
- **A candidate of a pass** is due when it is on, its subject has no pause standing, the round is not
  stopped and its clocks make it due. One off or paused has no next due moment.
- **Whether a target is on:** its subject's switch where one is thrown, else its registration's
  default, else the consumer's policy default.
- **A pause** is cleared only by a use whose work began after the pause was set.
- **A pass's plan** is the order its due candidates are pinged in. Targets go by rank, lowest first,
  then by consumer, then by target name. A group, keyed by its consumer and its name, goes together
  where its first target stands in that order; of its due targets the oldest touch goes first, one
  never touched oldest of all and equal touches in rank order, up to the group's limit. A stopped round
  plans nothing.
- **A reason is cut** to a count of UTF-16 units, the way a script counts a string; a surrogate pair the
  cut splits ends as the replacement character.

## Whether pings go

- **Pings of a kind go on a provider unless the owner's warming switch stands.** The switch stops every
  keepalive ping of the deployment's at once, whatever the kind and whatever the provider; while it
  stands the pool pings nothing and writes nothing down, and its reservations keep holding their places.
- **The switch is the owner's, thrown through the control port** (`docs/api.md`, 「Co-worker pool」, the
  warming switch) and kept in the store's settings as `keepalive_stopped` (`docs/store.md`, 「The settings
  table」). It stands while that row holds `1`, and a store that holds no such row warms. Its keeping is
  the server's; the library knows no store.
- **The switch is read again before each ping,** so one thrown during a long pass holds from the next
  ping on.
- **An agent's own switch and a worker's pause are no part of agency's schedule:** they are kept on the
  agent's own row, and the pool warms no agent.

## When a pool place is worth a ping

The pool judges each place by the general rule with clocks and a record of its own:

- **Its clocks:** the provider's idle stretch, the lifetime the provider publishes, the pool's margin
  (`policy/pool-warm.toml`), failures that do not touch, and no idle override. A place is chosen at one
  instant and its run opens its prefix some minutes later, so a lifetime counted to its last second
  promises a read the provider has already let go of; the margin is the same for every provider, while
  the lifetime and the idle stretch are each provider's own declaration (`docs/coworker-pool.md`, 「The
  list」). A provider whose lifetime leaves nothing after the margin is treated as one that publishes
  none.
- **Its record:** the place's last write as its last use, and the pool's record of its last answered
  ping as its last warmed answer, so a restart never pings a place twice inside its stretch.
- **Its prefix is still there** while its last write is inside its usable lifetime; a ping that
  answered writes the place again through its own record.
- **The usable lifetime a place is counted warm by** elsewhere in agency is the same: the published one
  less the pool's margin.

## What stays with the pool

- **Which places a reservation's pings are for,** by the kind of place each reservation's source names.
- **Whether a provider is on and lends co-workers,** read again before each ping.
- **Whether a rest holds the pings:** none goes during a rest on a provider whose pings rest with the
  account (`docs/cache-fallback.md`).
- **The ping and its record:** the run a ping becomes and the row the pool's ping table keeps of each.
- **The clock:** the pool's warming runs on agency's own clock at the keepalive pass's period, after the
  same start wait, never beside itself (`policy/keepalive.toml`).
