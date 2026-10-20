# Usage windows

How agency keeps the usage windows of every account that has them: the share of each limit an
account has used and when that limit resets. Directory and token accounts have them; an endpoint
account is paid by the request and has none, so it holds no slot and no reader asks it
(`docs/cache-fallback.md`, 「The Claude Code road is a list of accounts」). The account judgement, the stand-ins' walk, the fallback's view and the quota source
all read them from here (`docs/cache-fallback.md`, 「The account judgement」, `docs/api.md`,
「Accounts」). The keeping is the `cache-siren` crate, whose cache-lifetime judgement is
`docs/sessions.md`'s.

## A window and its slot

- **A window** is one limit of one account: its name, the fraction used (0 to 1), the moment it resets
  in epoch seconds or none, the moment it was seen, and, where the provider says them, how many minutes
  it runs and the name of the limit it belongs to.
- **A slot** is whose windows they are: the provider they were read of, and the login under it where the
  provider holds logins of agency's; a provider that holds none keeps its windows under the empty
  account. Two logins report windows of the same names, so a slot never overwrites another's.
- **A reading replaces its slot's whole set** in one transaction: every window the slot held goes, and
  the windows read are written with the moment they were seen. A window an answer did not state is
  gone from the slot rather than kept from an older reading.
- **The windows stand in the store** (`docs/store.md`), so a restart reads the last reading of every
  slot rather than nothing.

## Where readings come from

- **A turn's answer.** The windows a turn's answer carries — Claude Code's from its status line,
  Codex's from its rollout's `rate_limits` (`docs/agent-control.md`, 「The session record and usage are
  not squeezed out of the screen」) — are written under the slot of the login that produced them: the
  login a run was launched on, the login of the program that wrote the status record. A reading whose
  login cannot be named on a provider that holds logins is not written at all. A turn that read no
  windows leaves the slot as it was.
- **A reader on a clock.** Each provider whose windows can be asked outside a turn has a reader, and
  each reader declares its own provider, how often it passes and how long its reading stands. A pass
  never runs beside itself.

## The Claude account reader

- **Every registered directory and token account is asked on each pass,** on or off, with its own
  login, and an endpoint account never is: a GET of the
  provider's account-usage endpoint (`https://api.anthropic.com/oauth/usage`) carrying that account's
  access token as a bearer token. A request that has not answered in 15 seconds is a failure.
- **The token is read out of the account's own credential file and dropped:** a directory account's
  file holds it as `claudeAiOauth.accessToken` beside its `expiresAt`, and a token found expired is not
  sent; a token account's file is the token. The token goes into the one request header and is never
  kept, returned, logged or renewed; renewing a login is the CLI's.
- **The windows taken are `five_hour` and `seven_day`:** each `utilization` is a percentage and is
  kept as its fraction, and each `resets_at` is an RFC 3339 moment kept in epoch seconds. A window with
  no figure is left out rather than written as nought, and an answer with no usable window is a
  failure.
- **It passes every five minutes, and a reading stands for six passes,** thirty minutes: long enough to
  ride out a passing failure of the endpoint, short enough that a full window that came back early, a
  quota reset nobody could read, stops closing its login.
- **An account that cannot be asked keeps its last reading.** A failure is said once when it starts and
  its recovery once when it ends.
- **The unread mark.** Where an account's token is found expired, or the endpoint refuses it as
  unauthorised, the reader marks the moment it first found so under the account, and the next reading it
  takes lifts the mark. A reading seen at or before the mark is stale for the account judgement however
  young it is, since nothing can renew it until a run renews the login.
- **A window that came back early is said:** a window the last reading held full, not yet reset, that a
  new reading finds below its ceiling is logged as having come back before the reset its reading named.

## The Codex reader

- **Codex holds no logins of agency's,** so its windows stand under the empty account.
- **The newest rollout first.** Where the newest rollout file among the newest three days' folders ends
  in a token count written within the last minute, the windows it carries are the reading: each of its
  `primary` and `secondary` windows named `codex:<limit id>:<primary or secondary>`, its used percentage
  kept as a fraction, its reset, its length in minutes and its limit's name.
- **Otherwise the app-server is asked.** Codex's app-server is started over standard input and output,
  initialised, and asked `account/rateLimits/read`; the answer's windows by limit are read, or its
  single `rateLimits` where it gives none by limit, in the same names and fields. The process is ended
  with the reading, so none outlives it, and what it said on its error stream is carried beside a
  failure.
- **It passes every minute where Codex is installed, and a reading stands for six passes.** Only the
  public meter fields are kept: no account id, credit balance or plan detail.

## What reads the windows

- **The account judgement** closes a login whose own window stands at its ceiling, not reset, by a
  reading that stands: younger than its reader's span and later than the account's unread mark
  (`docs/cache-fallback.md`, 「The account judgement」). An endpoint account has no window for it to
  read, and the judgement asks it nothing here.
- **The stand-ins' walk** reads each login's week, the window its provider counts as its week
  (`docs/cache-fallback.md`, 「While a fallback stands, the room is not silent」).
- **The fallback's view and the quota source** read each slot's two headline windows (`docs/api.md`,
  「Accounts」).
- **A keeping that cannot be read is a fault, said with its cause,** never read as an account with no
  windows.
