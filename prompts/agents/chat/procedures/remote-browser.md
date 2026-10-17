# Remote browser

Read this before driving a browser on one of the owner's machines. It is owner-only work (ABSOLUTE
RULE 2), like everything else on those machines.

## Never step by step from a turn

- A browser on a jake node is driven with your own tool `remote_browser` and nothing else. Never
  chain `jake exec` calls, a CDP script of your own or `curl` to the debugging port inside a turn:
  each round trip costs seconds while the room waits on your turn, so a login and a menu or two
  hold it for minutes.
- Checking whether the node is up, or the browser answers at all, is still one command of your own;
  driving a page is not.

## Writing the steps

- Write the plan as the JSON the tool takes in `plan`: a list of steps, or an object with the list
  under `steps` and, where needed, `port` and `profile`. Defaults: port 9222 and the profile
  `bmo-edge`, which keeps the browser's logins; name another profile only for a browser that must
  start clean.
- The steps, each an object with its kind under `do`:
  1. `page` with `match` (words of the page's address or title) or `new` (an address to open)
  2. `goto` with `url`
  3. `click` with `selector`, or `text` (the words it shows), or `x` and `y`
  4. `type` with `selector` and either `text` or `secret` (a secret's name)
  5. `key` with `key` (Enter, Tab, Escape, the arrows)
  6. `wait` with one of `ms`, `selector`, `text`, `url`, or nothing for the page to load
  7. `read` with `js`, an expression evaluated in the page, and `as` for its name
  8. `shot`, with `as` for its name
  9. `close` for the page, `quit` for the browser
- `frame` on a click, type, wait or read runs it inside the frame with that name or address: for
  an old site that opens each screen in an iframe.
- A waiting step gives up after 30 seconds unless it carries `timeoutMs` (at most two minutes).
- End a run whose result you need to see with a `shot` or a `read`: those are what come back.
- Submit a form by clicking the page's own button, not Enter: an old page may not have wired its
  keys when the typing ends, and Enter then does nothing.
- A select a site hides behind its own drop-down is set with a `read` that assigns its value and
  fires `change`, then checked by the value that `read` returns.

## Secrets

- A password is never in the plan or on a command line. A `type` step names it with `secret`, a
  name of capitals and underscores, and its value, the one the owner gave you, goes in `secrets`
  as `NAME=value` lines:
  `remote_browser` with node <name>, plan '<the steps>' and secrets 'NAME=value'.
- With no secret in the steps, give no secrets.
- Never repeat a secret in the room, and never read a password field back with a `read` step.

## Launching and what comes back

- `~/.local/share/bmo/deploy/bin/bmo node list` lists the nodes; a remote browser needs a jake node.
- When what it did reaches you is `mode`, as the manual's TOOLS RUN APART says: `sync` is the
  default, `async` for a plan you expect to run long (long waits, many pages). Launching `async`,
  say in one short line that you started it, then end the turn.
- Either way, never drive anything yourself while it runs.
- What it did is each step, the values read, the dialogs a page raised, a failure with its reason
  and a screenshot of that moment, and the screenshots' paths on this machine. Open a screenshot
  with the Read tool when what the screen showed matters, and tell the room then.
- Where the work goes on (the next screen, a correction to a failed step) write the next plan and
  launch another run. It lands on the same browser and page state, so a login is not done again.
