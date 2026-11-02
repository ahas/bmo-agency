# The keepalive schedule

When a kept prompt-cache prefix is worth a ping, and whose prefixes agency keeps warm. Agency warms
supervisors alone: each supervisor's resident program, on each provider its row switches warming on for. No
role, no repository and no standing branch checkout is warmed, and no switch warms or stops everything at
once. The general rule is the schedule library `bmo-cache-warmer-schedule` publishes, plain values that use
nothing outside themselves, which agency and any other repository that keeps prefixes warm build on
(`docs/api.md`, 「Calls agency makes」). Agency's own schedule stands in the server's keepalive module and
calls it; the ping itself goes into the supervisor's program (`docs/agent-control.md`, 「The keepalive ping
goes into the same program as the turns」).

## The general rule

The library judges only what it is handed, `now` included: it reads no clock, no store and no policy of
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

## Whose prefix is warmed

- **A supervisor's, on the providers its row switches on.** A supervisor row carries, for each provider of its
  order, whether its prefix is kept warm on that provider (`docs/supervisors.md`, 「What a supervisor row
  holds」). The keepalive warms the program the supervisor stands on now, while the provider it stands on is
  one switched on; a supervisor whose current provider is switched off is passed over and its prefix left to
  cool.
- **Nothing of a role.** A role's runs are spawned per run and keep no program between runs, and a reservation
  of a role's slots warms nothing (`docs/role-slots.md`, 「Reserving slots」). A role's kept session is resumed
  while its cache is still warm and opened afresh once it has gone (`docs/sessions.md`, 「A role's session」).
- **Nothing of a repository or a place.** A standing branch checkout's list line is read to choose a place
  whose prefix may still be cached, never pinged.

## When a supervisor's prefix is worth a ping

The keepalive judges each supervisor by the general rule with clocks and a record of its own:

- **Its clocks:** the provider's idle stretch, the lifetime the provider publishes for the model the
  supervisor stands on, the margin taken off a lifetime wherever agency counts one (`policy/pool-warm.toml`),
  failures that do not touch, and no idle override. A provider whose lifetime leaves nothing after the margin is treated as one that publishes none.
- **Its record:** the supervisor's last turn as its last use, and its last answered ping as its last warmed
  answer, so a restart never pings a supervisor twice inside its stretch.
- **A ping that finds a turn working is skipped:** that turn is keeping the prefix warm itself.

## Whether a ping goes

- **The supervisor's switch for the provider it stands on is read again before each ping,** so one thrown during
  a long pass holds from the next ping on.
- **A provider that cannot answer now is not warmed** — off, its credential gone, its balance spent, every login
  it holds closed — since no turn goes out on it until it can.
- **Claude Code's accounts:** a ping goes out on the login the supervisor's session was opened under; an account
  that is off or resting is not pinged and its prefixes are left to cool, and an endpoint account is never
  pinged, since its endpoint keeps its own cache and a ping would only be a paid request
  (`docs/cache-fallback.md`, 「Warming only enabled providers」).
- **None goes during a rest** on a provider whose pings rest with the account.
- **The clock:** the keepalive pass runs on agency's own clock at the period and after the start wait
  `policy/keepalive.toml` names, never beside itself.
