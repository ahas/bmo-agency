# A person's PC

A person's PC is the node their BMO runs on. They turn the PC on, jake attaches it to the hub, and agency stands
their account's sessions there, through jake alone, with no SSH. The sessions run on the PC; what they run on — the
providers and their accounts, the model gateway, the brain, the tool host — is the deployment's. It is the shape of a
desktop coding agent whose models and tools are served from elsewhere.

## Which node is a person's

- **A person's node is the node agency's record names for their account:** the hub node of the machine their
  account was bound to when its invitation was proven, `u<user>` (the account service's `docs/accounts.md`,
  「Invitations and a person's machine」), or another the administrator bound it to. Agency keeps one record an
  account, set and taken away by the service that binds the account (`docs/api.md`, 「person-nodes」), or with
  `bmo-agency node person`; it names a node the node service keeps as a hub node reserved for no service. The
  node service lists it as any hub node (`docs/work-nodes.md`, 「In herdr」). Several accounts' records name one
  node only where the administrator declared it shared for each (「Several accounts on one node」).
- **An account with no record has its isolation node on this machine** (`docs/account-isolation.md`), and the
  administrator's account none: its runs are the owner's own. So every account but the administrator's works on a
  node of its own.
- **An account's runs go there and nowhere else:**
  - its supervisors' sessions: a supervisor of the account stands on the node as a worker of a node does, its
    head, its room panes and every turn of it in the node's herdr session;
  - its role agents' runs: a run whose slot belongs to the account goes the place-based road whatever its
    provider, its place opened on the node, its folder read as the node reads this machine's roots. On Claude
    Code the program in that place is Claude Code on the run's line, signed in by the token for the nodes of
    the first login the Claude Code road's walk offers among those that run on a node (`docs/work-nodes.md`,
    「The logins on a node」); the run goes out on that one login, and a run that fails on it ends failed
    rather than walking on. On another provider it is that road's own program;
  - the link mirrors each of these places as it mirrors a worker's: its program and settings sent, its running
    record and its transcripts brought back;
  - the folder each of these programs works in, its supervisor's own or its run's, is named in the spec, and the
    far end makes it where it is not, since nothing of it need stand on the PC before;
  - the status line and the hooks of each of these programs answer at home: the place's settings name
    `node-link home bin/bmo agent-status` in place of this machine's verbs binary, which hands the call to the home
    bridge to run with the CLI's `bmo` here, so the readings the door waits on before it puts a prompt — the status
    line drawn, the hooks called — are written into the place here.
- **While the node is not attached, its runs wait:** a turn of the account asks again every few seconds whether the
  node's link has been heard from, and starts once it has; a head or a room pane of a supervisor of the account is
  not stood until then. Nothing of it runs as this machine's user or on another account's node. A run of no
  account, or of the administrator's account, is placed as before.
- **A waiting turn says so, can be stopped and does not wait for ever:**
  - its `waiting` names the wait, `node-link` with the node and the reason (`docs/api.md`, 「sessions」), published
    as `turn.changed` as it begins and as it clears, and its run log says so;
  - a stop standing over its run — the interrupt (`docs/api.md`, 「sessions」) or `bmo-agency run stop`
    (`docs/agent-control.md`, 「Stopping one from a shell」) — is read at each ask, and the turn ends `stopped`;
  - one that has waited past the bound the policy names (`policy/person-nodes.toml`) ends failed, its reason
    naming the node and how long its link went unheard, so a turn its caller has left behind ends too;
  - while it waits the node is kept beside its kept record, so a restart neither waits on it nor loses it silently: the
    deployment's quiet does not count it, and the next server ends it failed with why (`docs/sessions.md`, 「A
    restart hands the session over」).
- **A waiting turn is placed by the record standing now:** before each ask it reads its placement again, so a
  record taken away or set to another node while it waits places it on the node the account's present record
  gives, the isolation node where none stands, and it waits on that node instead, or starts at once where that
  node is heard. A role run's place is marked again with that node. A turn whose supervisor was removed while it
  waits waits no more and ends with it (`docs/supervisors.md`, 「Adding and removing a supervisor」).

## Several accounts on one node

- **The administrator may bind several accounts to one node, declaring it shared for each.** A record carries
  `shared`, false unless the binding says so (`docs/api.md`, 「person-nodes」, and `bmo-agency node person
  --shared`). A record naming a node another account's record names is refused, `node-taken`, unless both
  records are shared, so no account lands on another's PC by a slip, and an account bound alone keeps its PC to
  itself.
- **An account whose record is shared has a workplace root of its own on that node:** `accounts/<account>` under
  the node's workplace root, `<node root>/accounts/<account>`. What agency names for the account's programs
  stands under it, each at the path it holds under this machine's data root:
  - each supervisor's own folder, `<account root>/supervisor-homes/<supervisor>`, in place of the one under the
    node's data root; the link spec reads the supervisor's folder here as that folder, so its program, the
    records the link brings back and the folder the far end makes all follow (`docs/work-nodes.md`, 「The link」);
  - a role run's folder is its caller's — a job's checkout, a worker ask's folder — and reads on the node
    through the roots as every other path, since agency names none of it.
- **An account whose record is not shared keeps its folders where they stand,** under the node's data root, so
  no path moves for a PC with one owner.
- **The rest of the node is one for all its accounts:** its data root with every place, its herdr session, the
  agent host, the CLI's records under the node's home, and the user every program runs as.
- **The node tool runs an owner's command inside that owner's root.** The access read answers `root`, the node
  path of the caller's account's root where its record names the node shared, null otherwise (`docs/api.md`,
  「person-nodes」). Confining the command to it is the node service's tool's.
- **One account's programs still read every other account's root on the node.** No launch wrap hides them, since
  one that hid `<node root>/accounts` alone would not confine a program running as the node's one user:
  - every program of that user that stands outside the wrap — the herdr session, the agent host, the far end —
    shows the whole filesystem through its `/proc/<pid>/root` unless the wrapped program has a process namespace
    and a `/proc` of its own;
  - the herdr session's socket and the agent host start programs outside any wrap for whoever asks them;
  - the places under the node's shared data root and the CLI's records under its home hold every account's
    programs, transcripts and settings side by side.
  So hiding needs all of these closed at once — a process namespace, the herdr and agent host sockets out of
  reach, places and records of an account's own — and a setup that installs bubblewrap on the PC and checks its
  user namespaces; it is left (「What stands and what is left」).

## Who reaches a node

- **An agent runs a command on a node through the node service's tool alone,** the tool host's `bmo/node_exec`
  (the node service's `docs/tools.md`), asked with `bmo use bmo/node_exec`. jake's console, the hub's exec route
  asked by hand and a script holding the hub's token are no way an agent takes. Agency's own setup, check and sync
  of a PC's workplace and the link it holds reach the hub for the deployment (「The workplace on a PC」, 「The link
  over jake」), and run no command an agent names.
- **A caller reaches the nodes its own account owns, and the administrator's supervisors reach every node.** The
  tool judges each call; agency supplies what it judges by, from its records:
  - **the node's owners,** the accounts whose record names the node, and for an isolation node the account it is
    named for; a node neither names is no person's and is reached by the administrator's supervisors alone;
  - **the account the caller acts for,** read off its agent: the supervisor it is and its `ownerAccount`; else the role agent of that name and
    its `ownerAccount`; else the seat its run holds at its role and folder (`docs/role-agents.md`, 「What a run
    reads off its agent」) and that seat's `ownerAccount`. A caller none of these names an account of is the
    deployment's own: the owner's supervisors, his jobs, his runs;
  - **the administrator's grant,** which a supervisor of no account or of the administrator's account holds, and
    no other caller does: not an account's caller, and not a role agent, a seat's run or a job of the
    deployment's own, which owns no node and so reaches none through the tool. Agency writes it into the record of
    every session it issues for such a supervisor: the grants `policy/person-nodes.toml` names, `node.admin`, the
    resource the node tool reads as reaching every node, and `platform.payment`, the platform's own card, which
    agency's DeepSeek top-up declares (`docs/tools.md`, 「The DeepSeek top-up」). A record holds what was true when it was issued, and a session issued
    again reads its caller afresh.
- **The tool reads the first two over agency's routes and the third off the caller's record** (`docs/api.md`,
  「person-nodes」): the node's owners from the list of these records, the caller's account from the supervisor or
  the role agent its record is held under, and the grant from the record itself, which agency wrote when it
  issued it. A run of an account's seat whose record is held under neither name holds no grant and names the
  tool no account, so it reaches no node at all, its own account's included.
- **One read answers all three for a caller and a node,** `GET /nodes/{nodeName}/access` (`docs/api.md`,
  「person-nodes」), judged as the record's grant is: the node's owners, the caller's account, whether it holds the
  administrator's grant, and whether it reaches the node. The node service reads it as its owners' read, asked on
  agency's control port for each run with the run's caller named as `agent`, and takes the node's owners alone
  from the answer (the node service's `docs/nodes.md`, 「A person's node」).
- **The hub's settings file is read by the units that reach a hub, and by no agent:** the node tool, run by the
  tool host for a call a caller makes; agency's server and its node verbs, for a PC's workplace and its link; and
  the node service, for its checks, its readings and the runs units make through it, reading the hub's control
  token for each run's one request and handing it to no caller. No session is handed the file's path or anything
  read from it: no record, run variable, place or link spec of a session names it, and the link sends none of it
  to a PC. An account's sessions stand on its PC (「Which node is a person's」), where the file never is, so their
  shells reach a node through the tool alone.
- **Agency reaches a hub for one node with a credential scoped to that node's agent alone,** never with the hub's
  control token: `<agent>.<mac>`, the mac the lowercase hex of HMAC-SHA256 over the agent's name keyed by the
  hub's control key, `JAKE_CONTROL_KEY` in the hub's settings file (jake's `docs/architecture.md`, 「Configuration
  and secrets」). The workplace's setup, check and sync present it on every request to the hub, and the link to
  the hub's connect program for every byte stream, so the hub refuses a request naming any
  other agent with 403 `agent-out-of-reach` and its status shows that one agent alone. A slip that names the
  wrong node then reaches no other person's machine.
  - **Agency makes the credential itself,** each time it opens its way to the hub, and keeps it nowhere: the key
    stands in the hub's settings file beside the control token, which every unit here can read already
    (「What stands and what is left」), so a credential handed in by the service that binds an account would close
    nothing more, and a hub node no account's record names is
    reached the same way.
  - **A hub whose settings hold no control key is not reached:** the setup and the link say the key is missing
    rather than falling back to the control token.
  - **The control token, which reaches every agent, is the node service's alone:** its tool, for a caller the
    administrator's grant lets reach every node; its runs, `POST /nodes/{nodeName}/runs` (the node service's
    `docs/api.md`, 「Runs」), each one command a unit that is not a session runs as itself with a token of its own,
    on a node whose callers setting names the unit and for an account among the node's owners, which the node
    service reads through agency's access read (「Who reaches a node」, above); and its checks and readings.
- **An account's node asks the home bridge for the verbs `policy/person-nodes.toml` names alone** (`bridge_verbs`):
  the status line's and the hooks' `agent-status`, a hosted tool's `use`, and the verbs of other units a
  supervisor's manual names. The bridge runs a call of a node an account owns, its PC or its isolation node, only
  where it asks for the CLI's `bmo` with one of those verbs as its first word, and refuses every other verb, every
  script and a call naming its verb after an option, saying so with exit code 2. A node no account owns is the
  deployment's own and asks for any verb, as an SSH node does. So no session of an account's node runs agency's
  node verbs, the testbed's checks or a script here, any of which would reach the hub's settings file as this
  machine's user.
- **The hub's settings file is the owner's user's,** under the owner's home, readable by that user alone. The
  sessions that run as that user are the owner's and the deployment's own, which keep the whole machine
  (`docs/account-isolation.md`, 「The scope」): its supervisors hold the administrator's grant and reach every node
  through the tool already, and its role agents' and jobs' runs reach none through the tool while their shells
  read the file. No session of any other account runs as that user: its runs stand on its PC, or on its isolation
  node under a user of its own, which neither reads the file nor runs `sudo` or docker.

## What the PC needs

- **A Linux shell:** WSL on a Windows PC, a Linux PC as it stands. herdr and the programs run there, as on every work
  node (「The workplace on a node」 in `docs/work-nodes.md`).
- **jake, installed by the invitation,** and nothing else installed by hand: agency lays the workplace down through
  jake — the tools, herdr, the programs and the data root — as setup lays one down on an SSH node.
- **A user manager of its own that runs while nobody is logged in:** systemd in the Linux shell, with lingering on
  for the person's account, since every long step and the far end run as units of it.
- **jake's agent offering the link's two ports to stream to:** the far end's mirror and herdr ports among its
  stream ports, which are agency's policy (`policy/person-nodes.toml`). The agent listens on nothing for the link:
  the ports a program on the PC reaches home through are the far end's own (「The link over jake」).

## The workplace on a PC

`bmo-agency node work setup <node> --install`, `check` and `sync` stand, read and bring up a PC's workplace with
the steps an SSH node's take (`docs/work-nodes.md`, 「The workplace on a node」), through the hub the node service
names for the node: its control address, its settings file, and the agent's name, every request carrying a
credential scoped to that agent (「Who reaches a node」).

- **A command goes through the hub whole, as bytes no shell on the way reads:** the hub is handed one line that
  decodes the command and gives it to the Linux shell, since an agent on a Windows PC hands its Linux shell a line
  another shell has already read once, and a variable the command sets and reads would come out empty.
- **Each command of the setup is a transient unit of the person's own:** a command through the hub writes the step
  into a script under the PC's cache directory and starts it with the user manager, and further commands watch it
  until it ends, each well inside the hub's ceiling of time on one command, then read back what it wrote and the
  code it ended with. So no step is cut by that ceiling, however long a cold build takes, and a step outlives the
  jake agent restarting: its watch is asked again, for a couple of minutes, until the agent attaches again. A
  program a step starts apart outlives the step's unit, as it outlives an SSH session.
- **The commands run in the shell the agent offers for Linux:** its own where it runs inside the Linux shell, WSL's
  where it runs on Windows.
- **A step that fails says why in what the PC wrote:** the code it ended with, what it wrote on its errors, and
  what it wrote on its output where its errors hold nothing, since WSL's own launcher, failing before any command
  runs, writes why on its output. Every command asks WSL to write its own messages as UTF-8, so they read here as
  written.
- **The copy goes through the hub's uploads:** the same files an SSH copy sends are gathered here into a mirror
  under the node's directory, packed into one archive, uploaded with its length and checksum declared, and laid over
  the PC's copy there with the same files left out, so what is built or installed on the PC stays. A PC without
  herdr is given this machine's the same way.
- **An upload is written where the agent's own filesystem reaches:** at the Linux path where the agent runs inside
  the Linux shell, and at WSL's Windows view of that path where the agent runs on Windows, asked of WSL for the
  folder the file goes into.
- **A PC is not saved as a machine of this herdr,** which reaches a machine over SSH alone; its herdr session is
  reached through the link.
- **The far end stands as a user service, `bmo-node-link`,** written by the setup into the person's user units and
  enabled, and started again after every copy so it runs the binary just built: the link's serve program on the
  workplace's root, listening at the PC's loopback on the mirror port and the herdr port, making the node's bridge
  socket, and keeping the herdr session and the agent host standing. Stopping it stops the far end
  alone; the herdr session and the agent host it started stay, so a restart ends no agent.
- **One PC's Linux shell holds one far end, and so serves one node's workplace.** The far end's ports are the
  policy's one pair and its user service has one name for the user, so a connection a program on the PC makes to
  the bridge socket or a port carried home is dialed home by whichever near end stood up last. Two agents of one PC,
  jake on Windows and jake inside its WSL, would each be given a link to that one far end, and a program of one
  node could be answered by the other node's near end, whose bridge takes other verbs. So:
  - the setup, check and sync read the Linux shell's machine id through the hub before anything else, and keep it
    in the node's setup record beside its herdr session;
  - a workplace whose machine id is the one another ready workplace's record holds is not set up and not made
    ready: the check says which node's workplace holds the PC's far end, and nothing is laid on the PC, since a
    setup would stand the far end again on its own root; it stays missing until one of the two workplaces is
    removed;
  - a machine id that cannot be read is said with the rest of what is missing, since nothing then tells whether
    another workplace holds the PC.
- **What the hub's agent does not offer is said** with the rest of what is missing: a mirror or herdr port not among
  its stream ports, or no Linux shell.
- **A port the far end could not listen on is said** with the rest of what is missing, by its number, what it is
  for and why, as the link's state last holds it (「The link over jake」), so the check fails on it.
- **WSL's networking is read and said on a Windows PC:** its networking mode, and in mirrored mode the range of
  ports its Linux side takes for its own connections, which Windows keeps from every program of its own. Any of the
  far end's ports inside that range is said with it, since a Linux connection's own port may hold it when the far
  end starts again.

## The link over jake

The link of a person's node carries what the SSH link carries (`docs/work-nodes.md`, 「The link」), over jake:

| What | Direction | How jake carries it |
|---|---|---|
| the mirror | both ways | a byte stream to the far end's mirror port on the PC |
| the node's herdr session socket | here → PC | a byte stream to the far end's herdr port, which the far end joins to the socket |
| the tool host's port | PC → here | the far end listens on the port at the PC's loopback; each connection is dialed home over a byte stream of its own to the mirror port, joined here to the tool host |
| the model gateway's port | PC → here | the same way, each connection joined here to the gateway (`docs/work-nodes.md`, 「The model gateway on a node」) |
| the home bridge's socket | PC → here | the same way from the node's bridge socket, each connection joined here to the bridge's socket |

- **The far end stands on the PC as a service of the person's own,** started by the workplace's setup and kept
  running there: the link's serve program, listening at the PC's loopback alone, inside the Linux shell beside the
  programs. A machine reached through the hub keeps nothing open to the world, since jake's agent dials out.
- **The near end makes no SSH connection:** it reaches the far end's ports through the hub's byte streams alone.
  What the mirror says is the SSH link's, frame for frame, with the frames of the road home below added.
- **Everything a program on the PC reaches at home goes by the same road:** a byte stream the near end opens to the
  far end's mirror port, as the mirror and the herdr socket go. Nothing is bound on the Windows side. An agent on
  Windows listening at its own loopback stands where WSL's mirrored networking keeps ports for its Linux side that
  no Windows tool shows as held, the whole range Linux takes for its own connections among them, so such a listen
  can fail with no holder anyone can find and no turn of the PC's ever taken; the far end listens where the
  programs connect, on the Linux side, which its own check reads. It needs no listen port of the agent's and no
  target of the hub's.
- **The road home, connection by connection:**
  - once the mirror is up, the near end names the ports here a program reaches — the tool host's and the model
    gateway's — and the far end listens on each port any standing session names at the same number on its
    loopback, keeping one it already listens on, trying a refused one again every half minute, and answering
    which it could not listen on and why: to the session that named them, and to every standing session whenever
    that changes;
  - each connection the far end takes there, or on the node's bridge socket, is held while the far end asks the
    newest standing session's near end over the mirror to dial it home, naming it by a number of its own and what
    it is for, the ask going ahead of any file the mirror is still sending (`docs/work-nodes.md`, 「The link」), so
    a transcript on its way never holds a hook's call past the wait below;
  - the near end opens a new byte stream to the mirror port, reads the far end's greeting, names the connection on
    it, and joins the stream to the port here, or to the bridge's socket here; the far end joins the stream to the
    connection it held, each side's close passed on to the other as it comes. A connection not dialed within
    twenty seconds is closed;
  - the near end joins a connection to a port here only where it named that port itself, so the far end reaches
    nothing else here.
- **The hub listens for the link on nothing, and the near end lets go of whatever it holds:** as each connection
  is made, the near end reads the listens the hub keeps for the node's agent and asks it to drop each one, with the
  same credential scoped to that agent. A listen the agent holds at Windows' loopback takes the same port from the
  far end, since mirrored networking shares one set of loopback ports between the two sides, and nothing of the link
  asks for one.
- **A port the far end could not listen on is the link's state,** with why, written with the rest of the state, and
  said once in the link's log, which is agency's, each time what it could not listen on changes; a turn of a program
  on the node says it waits on that port, and ends on it (`docs/work-nodes.md`, 「When a node drops」).
- **The spec writer writes a jake spec for a node with no SSH login and a hub reach:** the agent the node service
  names, the hub's connect program run as the hub's executable the deployment names in `AGENCY_JAKE_HUB` with its
  `connect` mode, the hub's settings file, and the far end's two ports from agency's policy. A node whose hub program
  is not named has no spec, and the pass says why.
- **A connection the PC has not been heard on is dropped and dialed again** (`docs/work-nodes.md`, 「The link」),
  since the hub's connect program can stand open with nothing coming through it after the PC's agent was replaced.
- **The far end speaks the mirror with each node's newest near end alone:** a near end names its node first on every
  mirror session it opens, and a session that opens for a node ends the one an earlier connection of that node
  holds. A near end that went away — its unit stopped, the connection it held dropped — may leave its stream
  standing on the PC with nothing to close it, and a session waiting behind it would greet each new near end and
  then say nothing, so the link would never be heard again. A session that names no node counts as one node of its
  own.
- **The sessions of different nodes stand side by side:** two agents on one PC, one on Windows and one inside its
  Linux shell, reach the same far end at the same loopback, so two nodes' links can hold one far end. Were one to
  end the other's session, each link would dial again and end the other's in turn for as long as both stand, and
  the one whose wait had grown longest, as an agent's outage grows it, would stay down nearly all the time.
- **Nothing of the deployment's credentials is sent to the PC** beyond what the SSH link sends a work node: the run's
  own variables, the CLI's account token for the nodes, its place, its tool token, the console's public address,
  its supervisor's owner account, and for a program on a gateway road the gateway's key file its key helper reads,
  written there readable by the PC's user alone as the tool token is (`docs/work-nodes.md`, 「The model gateway on a
  node」).

## What stands and what is left

| Part | Stands | Left |
|---|---|---|
| the link over jake | a link spec may name a person's PC (`jake`): the near end speaks the mirror over the hub's connect program, joins the herdr socket, and dials home each connection the far end takes for the tool host, the model gateway or the bridge, over a stream of its own to the mirror port, a connection unheard for its bound dropped and dialed again; the far end listens at the PC's loopback for the mirror, the herdr socket and the ports the near end names, and says which it could not listen on | — |
| the workplace on a PC | setup, check and sync reach a PC through its hub, each command a transient unit of the person's own watched until it ends: the deploy tree's commit uploaded and laid out there, the workplace's own runtime, the node binaries built there, the data root and the herdr session, and the far end standing as the user service `bmo-node-link` on the mirror and herdr ports; a second workplace on a PC whose far end a ready workplace holds, told by the Linux shell's machine id, refused and said; what the agent does not offer, a port the far end could not listen on and WSL's networking on a Windows PC are said; a look the PC did not answer said as such rather than as a tool missing, and a workplace not ready checked again by itself so its link stands again once a check finds it ready | — |
| an account's runs on its PC | agency's record of an account's node, set by the binding service or the verb; the spec writer writes a jake spec for a PC; an account's supervisors' sessions and its role agents' runs placed on its node alone, waiting while the node's link is not heard, the wait typed `node-link`, ended by a stop and bounded by the policy, and neither counted by the quiet nor lost silently across a restart | — |
| who reaches a node | the administrator's grant written into every record agency issues for a supervisor of no account or of the administrator's account and into no other; the records the node tool reads a node's owners and a caller's account from, and one read of them with the grant and the reach; no session handed the hub's settings file's path or anything read from it, an account's sessions standing on its PC or its isolation node, and the home bridge of an account's node running the policy's verbs alone; agency's setup, check, sync and link reaching a hub with a credential scoped to the node's agent, made from the hub's control key, and refused 403 `agent-out-of-reach` on any other agent | — |
| several accounts on one node | a record's `shared`, a second account's record on a node refused `node-taken` unless both are shared; each account of a shared record given `<node root>/accounts/<account>`, its supervisors' folders standing there through the link spec, and its root answered by the access read; a record not shared keeping its folders | the node service's tool running an owner's command inside the `root` the access read answers; a program of one account kept from every other account's root, places and records on the node, which needs a process namespace, the herdr and agent host sockets out of its reach, places and CLI records of each account's own and bubblewrap set up and checked on the PC, all at once (「Several accounts on one node」) |
| a check on a real PC | a PC attached as a jake agent with its stream ports set; its workplace ready, set up through the hub and its copy kept on the deployed commit by the links pass's sync; the server writing its jake spec and holding its link, the mirror, the herdr session, the tool host and the bridge each answering through it; an account bound to it whose supervisor's folder is made on the PC, whose program starts there past its first-run screens, and whose turn is put in once its status line, answered through the bridge, is drawn, so its BMO answers from the PC | — |

