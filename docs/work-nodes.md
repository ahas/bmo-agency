# Work nodes

A work node is another machine agency places agents on and sends heavy runs to. Agency stands a
workplace on it, holds a link to it, carries each account's login to it, and reaches an agent that
stands there through its link. This document covers the workplace, the link, the logins, the bot's
side of a remote agent, the rotation, and what a dropped node does.

## The workplace on a node

- `bmo-agency node work` lists every workplace with its state, its link and what is missing.
  `setup <node> [--root <path>] [--install]` stands one, `check <node>` reads the same without writing
  anything on the node, `sync <node>` copies and builds the code again, `runs <node> on|off` says
  whether heavy runs go to it, and `remove <node>` makes it no workplace. The root is a default under
  the node's home unless named.
- **Every command of a setup goes over one connection,** which the first makes and the rest share,
  held under the runtime directory: a node may refuse a run of logins made in quick succession, as a
  firewall limiting SSH does. Each command is bounded on the node itself, since the shared connection
  holds a command's output for as long as it runs there.
- **What setup makes, each step checked before the next, and everything still missing said:**
  1. the login, and the node's home read through it;
  2. the tools: git, rsync and curl are the machine's to have; the JavaScript runtime, the Rust
     toolchain and Claude Code are installed into the node's account with `--install`, each installer
     fetched into a file and run with nothing on its standing input. The runtime is the version this
     machine runs, since a newer one may refuse the repository's own configuration, and the Rust
     toolchain is this machine's version, installed beside the account's own and named for the node's
     build alone, so the account's default stays as it was;
  3. the processor: the runtime, and Claude Code built on it, need one with the vector extensions
     they are compiled for, so a node whose processor offers none — a virtual machine on a generic
     processor type — is said not ready, as is one where the runtime is installed and still does not
     run;
  4. the code: the deploy tree's commit copied to `<root>/deploy` — its installs, build output, logs,
     env files and the data root left out — installed there, and the node's own host, pane and link
     binaries built there, since this machine's binaries are linked against a newer C library than a
     node may have. They are built in a workspace of their own under `<root>/build`, the crates they are
     made of (the agent host, the store beneath it, the pane head and the node link) each at the path it
     holds in the repository, with the node link's node build manifest and the repository's lockfile,
     so a node builds those crates alone and nothing of the server's; the manifest's members and
     dependencies are the repository workspace's own, which a test holds it to. The testbed binary a
     node runs is the testbed unit's, not one agency builds; a build ends the agent host standing on the
     old binary;
  5. the node's data root, `<root>/data`, which holds the node's agent places and nothing of the
     store;
  6. herdr: a node with none is given this machine's, which is one static binary, so a matching one is
     found there instead of asking a person to approve installing it, and the node is saved as a
     machine of this herdr once, so herdr's own sidebar shows it; the node's own session, apart from
     any session a person keeps there, is started where it does not stand, and its name and socket
     written into the node's setup record here, which the link reads; the name is the setup verb's,
     the packages under it taking it as given;
  7. each node checkout of a worker placed there, cloned from its base's remote at its path, the
     repository's own setup run in it, trusted in every account's directory on the node so no first
     turn stops at the folder check, and its push to the remote tried. The trust is written by the
     node's own host binary, built in step 4, since the node's deploy tree holds no `bmo-agency` and
     the node runs nothing out of its scripts (`docs/agent-control.md`, 「A first interactive run stops
     at the trust-this-folder check」).
- **A credential file is never copied.** A subscription login renews itself by a token that changes
  as it is used, so two machines sharing one sign each other out. So:
  - Claude Code on a node runs on each account's token for the nodes, which this machine hands each
    run (「The logins on a node」 below); an account with no such token here needs a login made on the
    node in the node's directory for that account, and the setup names the command that makes the
    token;
  - pushing to the repository's remote is the node's own credential, and the setup tries it;
  - an endpoint account (`docs/cache-fallback.md`, 「The Claude Code road is a list of accounts」) is
    not offered on a node at all: its key file is a credential file, it has no token for the nodes,
    and its runs stand on this machine alone.
- **The row says what is missing,** one thing per line, and a node with anything missing is not ready:
  nothing is launched or run on it.
- **The node's copy follows the deployment.** The setup writes down the deployed commit it copied.
  Where the deployed commit has moved past it, the process keeping the links (「The link」 below) starts
  a sync of its own as a unit; a sync stops the node's link, which that process stands again on the
  new code.

## The link

- **One link per ready workplace:** a connect program of the crate workspace, under a unit of its own,
  which agency's server starts with the workplace's spec — as it starts, once a minute and whenever
  the workers move — and stops for a workplace no longer ready. Agency's server keeps the links; a
  process standing on a copy of the data root keeps none. The node's SSH login is asked of the node
  service, which keeps the nodes (`docs/api.md`, 「The node service」).
- **Its spec is the server's:** a link record of the node's under the data root, written whole
  whenever what it says moves — the login, the forwards, what is mirrored each way, how this machine's
  roots read on the node, which variables of a program go there. The link reads it again as it moves,
  and makes a new connection only where the connection itself changed.
- **It holds one SSH connection,** made the check's way, and carries over it:
  - the node's herdr session socket, forwarded to a socket of the node's here, which is where every
    herdr call of the bot's for that node goes;
  - the tool host's port, forwarded to the node's loopback, so the program's MCP tools reach the one
    host here;
  - the home bridge's socket, forwarded to a socket named for that connection under the node's data
    root, which the node's bridge points at. What a node asks of the bridge — the CLI's `bin/bmo` and
    a script under `scripts/` — is the CLI's, not agency's, so the bridge runs it out of the deploy
    tree of the services agency shares the data root with, `$HOME/.local/share/bmo/deploy`, never out
    of agency's own (`docs/agent-control.md`, 「The job's release」);
  - the mirror, spoken with a serve program started at the far end.
- **The far end makes sure of what stands on the node** whether a link holds it or not: the agent host
  of the node's data root, which the heads there ask, and the herdr session, each started apart from
  the connection where it does not stand.
- **The mirror keeps one direction per file:**
  - from here to the node: an agent place's program, its settings, its run and the handed mark, with
    this machine's data root, deploy tree, prompt tree and home rewritten to the node's, every file
    the program names under the data root or the prompt tree sent along — never one under the data
    root's credential directory — and of the program's environment only the run's own variables, the
    CLI's (the account's token for the nodes among them), its place and its tool token: no token of
    the bot's leaves this machine. The node's own search path and the bridge's socket are laid over
    it;
  - from the node to here: the place's running record and start failure; the program's transcripts,
    under the same configuration directory here as the account's own, sent as what they grew by; each
    checkout's graph, into a shadow under the node's directory here that the tool host's session
    record names as the checkout; and the node's load, memory, processor count and pressure, into the
    node's directory here, every few seconds;
  - which processes of a remote place's running record are alive, written beside it, and a signal
    asked for a process on the node through the link's control socket.
- **Its state** — connected since, last heard from, the node's home, the reason it last dropped — is
  written beside the node's directory here. A link that drops is made again, waiting a doubling bound
  up to a minute between tries; one heard from in the last twenty seconds is up.

## The logins on a node

- **Each directory and token account has one token for the work nodes:** the long-lived token the
  CLI's setup command issues for it. An endpoint account has none and is neither counted nor warned
  about here. This machine keeps it outside the repository, in the data root's own token directory,
  one file named by the account's id and readable by the bot's user alone; a token account's own token
  file is its token for the nodes as well. Where the files stand is Claude Code's declaration in
  agency's policy (`policy/`), read by every process that needs it. No token is written into the
  store, a log line, a document, a commit or a report.
- **What the CLI does with it:**
  - the setup-token command opens the subscription sign-in in a browser and prints a token to set as
    `CLAUDE_CODE_OAUTH_TOKEN`; it asks for a lifetime of a year and holds no refresh, so nothing
    renews it and no other machine's use of it signs anyone out;
  - the token is the account's that approves the sign-in in the browser, whatever configuration
    directory the command was run under;
  - it is inference-only: a run on it answers prompts, while a remote-control surface and the
    browser-driving surface refuse it, and no run on a node uses either;
  - a program handed `CLAUDE_CODE_OAUTH_TOKEN` signs in with it ahead of any login its configuration
    directory holds, so a node needs no login file.
- **The token goes with each run, not with the node.** A job's program on a node is composed here for
  the account its walk goes out on: a directory account's program is handed that account's
  configuration directory and its token for the nodes, and the link carries the program's
  `CLAUDE_CODE_*` variables to the node with it (「The link」 above). So:
  - a node registered, a workplace set up, a hub node newly installed and a herdr machine newly added
    need nothing more for Claude Code: the first job placed there carries the token;
  - an account switch — a full weekly window, the owner's choice — moves a node's next run onto the
    next account's token as it moves a run here, and the program on the node starts again on it;
  - a token made again reaches every node at its next run, since a program handed another token is
    another program;
  - an account with no token for the nodes goes out on a node with its directory alone, signed in by
    whatever login a person made in that directory there.
- **Making one is one command per account, run by the owner on this machine from any folder:**
  `bmo-agency node claude-token '<account name>'`. It runs the CLI's setup-token command under that
  account's directory, whose sign-in the owner approves in a browser signed in to that account, then
  takes the printed token pasted in, unechoed, refuses one not shaped as a Claude Code token, and
  writes it to the account's file. `--paste` takes a token already made, without the sign-in, from the
  terminal or from standard input.
- **A token that stops answering is told, once.** A run on a node that fails on its login is read as
  that account's token for the nodes gone — expired, revoked or never made — and never as the
  account's login here, which still answers:
  - the mark is a setting holding the token file's moment as it stood, and while it stands the account
    is passed over for every run on a node, its work there going to the next account in the walk;
  - a new token is a new file, so the mark lifts by itself, on nobody's word;
  - the owner hears of it once per mark, in a private chat: which account, and the one command that
    makes its token again;
  - a token's year is warned about a day before it is out, once, by the keepalive pass's login watch
    (`docs/cache-fallback.md`, 「A login that is about to end」).
- **Where it runs:** the composing, the reading of a failure and the warning are agency's server's,
  whose job runner runs every job.

## The bot's side of a remote agent

- **Every herdr call for an agent on a node goes to that node's socket:** an agent's home names the
  herdr it stands in, and standing its pane, a turn, a stop, a confirmation and the leftovers sweep
  take it from there. Pane and agent ids are the node's, so they are always read and used together
  with the node. A silent herdr of a node is the node's link, and is told as no want of room here.
- **Only a resident program stands on a node.** A structured execution — a gate, an editor, a runner,
  a co-work coordinator — always runs here, so its cancel files and process groups are this machine's
  as ever.
- **A remote place's processes are the node's.** Every operation on a place's processes — whether its
  program or its head is alive, ending the program, interrupting it — is asked of the place, which
  answers locally for a place here and through the link for a place on a node: liveness off what the
  link mirrored, an end as a signal the link carries. A link that has told nothing lately says nothing
  either way, and the program is taken as standing rather than stood up again. Nothing here signals a
  process number of another machine.
- **The program is composed here, with this machine's paths,** and every file its command names is
  checked here as for any launch; the link rewrites them as it pushes. What it names in the deploy
  tree is the node's copy of the same commit.
- **The head's loop is composed with the node's roots,** so the pane on the node starts that node's
  pane head, which asks the node's agent host, on the node's own environment.
- **The job runner reaches the checkout through the node:** every git question of the job's lifecycle
  — the branch prepared, what is open, what the job committed, parking — runs there over one shared
  connection, main is the remote's, brought up before the job's branch is made from it, and the graph
  is read through its shadow here. The job unit asks agency where that shadow stands for its checkout,
  and reads the nodes and how SSH reaches one through agency, which hands on the node service's reads
  (`docs/api.md`, 「Workers」).
- **The records read here as they always did,** off the mirrored files: the turn's answer and
  activity, the session collection, the run log, the token usage, a job's quiet and stalled watch.

## The rotation

- **A launch goes round the machines.** Of a base's free checkouts, a support checkout already on the
  item's branch keeps the item on its machine, waiting where that machine has no room; a launch naming
  one checkout goes to that checkout's machine; any other takes the machine after the one the base's
  last launch went to, in the node list's order, passing over every machine with no room or no free
  checkout of the base, and takes the free checkouts there, the base's own first. Where no machine has
  room the launch waits, every machine's want of room said.
- **Heavy runs go round the same way,** in a rotation of their own over this machine and every ready
  work node that takes runs.
- **Where each rotation stands is the store's,** a rotation row per rotation — one for each base's
  launches, one for the heavy runs — naming the machine it last went to, so a restart carries on from
  there; and each machine's count of jobs and runs it was given is kept beside it.
- **Both are shown:** the console's live board in the machine popover, each machine with how many jobs
  and runs it was given and its room, and where the next heavy run goes, wherever any node takes work.

## When a node drops

- **The program keeps working:** it stands in the node's herdr session, which no connection holds.
- **A turn waits for the link** for up to ten minutes where the node's herdr does not answer, instead
  of the two a silent herdr here is given; the mirror catches the transcript up when the link is back,
  and the turn reads on from where it stood.
- **Past that wait the turn ends as unanswered,** the job ends as failed, and its work stays committed
  or parked on its branch on the node for the next launch on that checkout.
- **Nothing new goes to a node whose link is down:** neither a launch nor a heavy run.

## In herdr

- **A work node is a machine of herdr's own as well:** its setup saves it as a herdr machine, so
  herdr's sidebar shows the node's own session, where the agents placed there stand.
- **Every node has a workspace of its own in the bot's herdr session,** standing after every other
  workspace, in the list's order, so the nodes read as one block at the bottom of herdr's left side.
- The workspace carries the token naming the node and is labelled with the node's label and its state
  word, its name where it has no label. Agency's server stands the workspaces after every reading of
  the node service's list (`docs/api.md`, 「The node service」): it makes a missing one, renames one
  whose state changed, moves the block back to the bottom and closes the workspace of a node that is
  gone, and publishes `nodes-moved` where the list's shape moved since the reading before.
- The workspace's pane runs the deploy tree's node-pane command, which shows the node as the node
  service lists it and keeps it current: its state, and its kind, `ssh` or `jake` for a node reached
  through a hub. Agency tells a hub node apart by that kind alone and keeps no record of the hubs
  itself. On an SSH node Enter logs in, with the login the node service names to it,
  interactively — a person is at the pane, so a password is asked for there if no key is taken — and
  the pane comes back to the state when the login ends. Ctrl-C ends a login — a password prompt
  included — and never the pane: the loop and the pane verb let it pass, so only the login it was meant
  for stops.
- A hub node's pane shows its state alone, since the hub offers no interactive shell.
- A browser on a hub node is driven through the remote browser tool (`remote-browser.md`).
