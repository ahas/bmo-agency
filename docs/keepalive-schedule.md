# The keepalive schedule

Whether agency's keepalive pings go, and when a kept prefix is worth a ping: the `cache-warmer` crate.
The co-worker pool asks it before it judges a reservation's places and again before each ping goes
(`docs/coworker-pool.md`, 「Reserving places and keepalive」); which places stand, the ping itself and its
record stay the pool's.

## Whether pings go

- **Pings of a kind go on a provider unless the owner's warming switch stands.** The switch stops every
  keepalive ping of the deployment's at once, whatever the kind and whatever the provider; while it
  stands the pool pings nothing and writes nothing down, and its reservations keep holding their places.
- **The switch is the owner's, thrown through the control port** (`docs/api.md`, 「Co-worker pool」, the
  warming switch) and kept in the store's settings as `keepalive_stopped` (`docs/store.md`, 「The settings
  table」). It stands while that row holds `1`, and a store that holds no such row warms.
- **The switch is read again before each ping,** so one thrown during a long pass holds from the next
  ping on.
- **An agent's own switch and a worker's pause are no part of the schedule:** they are kept on the agent's own
  row, and the pool warms no agent.

## When a prefix is worth a ping

- **The usable lifetime** of a prefix is the lifetime its provider publishes for it less the pool's
  margin (`policy/pool-warm.toml`): a place is chosen at one instant and its run opens its prefix some
  minutes later, so a lifetime counted to its last second promises a read the provider has already let
  go of. A lifetime no longer than the margin leaves no usable lifetime, and such a prefix is treated as
  one whose provider publishes none.
- **A prefix is due** when its idle time, now less its last write, has reached its provider's idle
  stretch and is still under its usable lifetime. One not yet idle enough is not worth a read; one past
  its usable lifetime is already gone, and pinging it would rebuild it at the cost of the work it waits
  for.
- **A place pinged within its idle stretch is not due,** by the pool's record of its last answered
  ping, so a restart never pings a place twice inside its stretch.
- **The margin is the schedule's one subtraction,** the same for every provider; the lifetime and the
  idle stretch are each provider's own declaration (`docs/coworker-pool.md`, 「The list」).

## What stays with the pool

- **Which places a reservation's pings are for,** by the kind of place each reservation's source names.
- **Whether a provider is on and lends co-workers,** read again before each ping.
- **Whether a rest holds the pings:** none goes during a rest on a provider whose pings rest with the
  account (`docs/cache-fallback.md`).
- **The ping and its record:** the run a ping becomes and the row the pool's ping table keeps of each.
- **The clock:** the pool's warming runs on agency's own clock at the keepalive pass's period, after the
  same start wait, never beside itself (`policy/keepalive.toml`).
