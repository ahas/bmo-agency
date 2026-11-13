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
  node service lists it as any hub node (`docs/work-nodes.md`, 「In herdr」). An account with no record has no
  node.
- **An account's runs go there and nowhere else:**
  - its supervisors' sessions: a supervisor of the account stands on the node as a worker of a node does, its
    head, its room panes and every turn of it in the node's herdr session;
  - its role agents' runs: a run whose slot belongs to the account goes the place-based road whatever its
    provider, its place opened on the node, its folder read as the node reads this machine's roots;
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
  not stood until then. Nothing of it runs on this machine or another account's node. A run of no account, or of an
  account with no node, is placed as before.

## Who reaches a node

- **An agent runs a command on a node through the node service's tool alone,** the tool host's `bmo/node_exec`
  (the node service's `docs/tools.md`), asked with `bmo use bmo/node_exec`. jake's console, the hub's exec route
  asked by hand and a script holding the hub's token are no way an agent takes. Agency's own setup, check and sync
  of a PC's workplace and the link it holds reach the hub for the deployment (「The workplace on a PC」, 「The link
  over jake」), and run no command an agent names.
- **A caller reaches the nodes its own account owns, and the administrator's supervisors reach every node.** The
  tool judges each call; agency supplies what it judges by, from its records:
  - **the node's owners,** the accounts whose record names the node; a node no record names is no person's and is
    reached by the administrator's supervisors alone;
  - **the account the caller acts for,** read off its agent: the supervisor it is, or the one a double stands for,
    whose record is held under that supervisor's name, and its `ownerAccount`; else the role agent of that name and
    its `ownerAccount`; else the seat its run holds at its role and folder (`docs/role-agents.md`, 「What a run
    reads off its agent」) and that seat's `ownerAccount`. A caller none of these names an account of is the
    deployment's own: the owner's supervisors, his jobs, his runs;
  - **the administrator's grant,** which a supervisor of no account holds, as it is and as a double stands for
    it, and no other caller does: not an account's caller, and not a role agent, a seat's run or a job of the
    deployment's own, which owns no node and so reaches none through the tool. Agency writes it into the record of
    every session it issues for such a supervisor: the grants `policy/person-nodes.toml` names, the resource the
    node tool reads as reaching every node. A record holds what was true when it was issued, and a session issued
    again reads its caller afresh.
- **The tool reads the first two over agency's routes and the third off the caller's record** (`docs/api.md`,
  「person-nodes」): the node's owners from the list of these records, the caller's account from the supervisor or
  the role agent its record is held under, and the grant from the record itself, which agency wrote when it
  issued it. A run of an account's seat whose record is held under neither name holds no grant and names the
  tool no account, so it reaches no node at all, its own account's included.
- **One read answers all three for a caller and a node,** `GET /nodes/{nodeName}/access` (`docs/api.md`,
  「person-nodes」), judged as the record's grant is: the node's owners, the caller's account, whether it holds the
  administrator's grant, and whether it reaches the node.
- **The hub's settings file is read by the units that reach a hub, and by no agent:** the node tool, run by the
  tool host for a call a caller makes; agency's server and its node verbs, for a PC's workplace and its link; and
  the node service, for its checks and its readings. No session is handed the file's path or anything read from
  it: no record, run variable, place or link spec of a session names it, and the link sends none of it to a PC.
  An account's sessions stand on its PC (「Which node is a person's」), where the file never is, so their shells
  reach a node through the tool alone.
- **Agency reaches a hub for one node with a credential scoped to that node's agent alone,** never with the hub's
  control token: `<agent>.<mac>`, the mac the lowercase hex of HMAC-SHA256 over the agent's name keyed by the
  hub's control key, `JAKE_CONTROL_KEY` in the hub's settings file (jake's `docs/architecture.md`, 「Configuration
  and secrets」). The workplace's setup, check and sync present it on every request to the hub, and the link on
  its listens and to the hub's connect program for every byte stream, so the hub refuses a request naming any
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
    administrator's grant lets reach every node, and its checks and readings.
- **A person's PC asks the home bridge for the verbs `policy/person-nodes.toml` names alone** (`bridge_verbs`): the
  status line's and the hooks' `agent-status`, a hosted tool's `use`, and the verbs of other units a supervisor's
  manual names. The bridge runs a call of a node any account's record names only where it asks for the CLI's
  `bmo` with one of those verbs as its first word, and refuses every other verb, every script and a call naming
  its verb after an option, saying so with exit code 2. A node no record names is the deployment's own and asks
  for any verb, as an SSH node does. So no session of a PC runs agency's node verbs, the testbed's checks or a
  script here, any of which would reach the hub's settings file as this machine's user.
- **The hub's settings file is the owner's user's,** readable by that user alone, and every unit and every session
  of this machine runs as that user, so the file's mode keeps no process here from it. The sessions here are the
  deployment's own: its supervisors hold the administrator's grant and reach every node through the tool already,
  and its role agents' and jobs' runs reach none through the tool while their shells read the file; a run of an
  account with no node is placed here as well, and its shell reads the file too. The token is closed to every
  session only once it is held by a user of its own that only the hub, the tool's executable, agency's node verbs
  and the node service run as (「What stands and what is left」).

## What the PC needs

- **A Linux shell:** WSL on a Windows PC, a Linux PC as it stands. herdr and the programs run there, as on every work
  node (「The workplace on a node」 in `docs/work-nodes.md`).
- **jake, installed by the invitation,** and nothing else installed by hand: agency lays the workplace down through
  jake — the tools, herdr, the programs and the data root — as setup lays one down on an SSH node.
- **A user manager of its own that runs while nobody is logged in:** systemd in the Linux shell, with lingering on
  for the person's account, since every long step and the far end run as units of it.
- **jake's agent offering the link's ports:** the far end's mirror and herdr ports among its stream ports, and the
  tool host's port and the far end's bridge port among its listen ports. The far end's three ports are agency's
  policy (`policy/person-nodes.toml`); the tool host's is the deployment's.

## The workplace on a PC

`bmo-agency node work setup <node> --install`, `check` and `sync` stand, read and bring up a PC's workplace with
the steps an SSH node's take (`docs/work-nodes.md`, 「The workplace on a node」), through the hub the node service
names for the node: its control address, its settings file, and the agent's name, every request carrying a
credential scoped to that agent (「Who reaches a node」).

- **Each command of the setup is a transient unit of the person's own:** a command through the hub writes the step
  into a script under the PC's cache directory and starts it with the user manager, and further commands watch it
  until it ends, each well inside the hub's ceiling of time on one command, then read back what it wrote and the
  code it ended with. So no step is cut by that ceiling, however long a cold build takes, and a step outlives the
  jake agent restarting: its watch is asked again, for a couple of minutes, until the agent attaches again. A
  program a step starts apart outlives the step's unit, as it outlives an SSH session.
- **The commands run in the shell the agent offers for Linux:** its own where it runs inside the Linux shell, WSL's
  where it runs on Windows.
- **The copy goes through the hub's uploads:** the same files an SSH copy sends are gathered here into a mirror
  under the node's directory, packed into one archive, uploaded with its length and checksum declared, and laid over
  the PC's copy there with the same files left out, so what is built or installed on the PC stays. A PC without
  herdr is given this machine's the same way.
- **A PC is not saved as a machine of this herdr,** which reaches a machine over SSH alone; its herdr session is
  reached through the link.
- **The far end stands as a user service, `bmo-node-link`,** written by the setup into the person's user units and
  enabled, and started again after every copy so it runs the binary just built: the link's serve program on the
  workplace's root, listening at the PC's loopback on the mirror port and the herdr port, joining the bridge socket
  to the bridge port, and keeping the herdr session and the agent host standing. Stopping it stops the far end
  alone; the herdr session and the agent host it started stay, so a restart ends no agent.
- **What the hub's agent does not offer is said** with the rest of what is missing: a mirror or herdr port not among
  its stream ports, the tool host's or the bridge port not among its listen ports, or no Linux shell.

## The link over jake

The link of a person's node carries what the SSH link carries (`docs/work-nodes.md`, 「The link」), over jake:

| What | Direction | How jake carries it |
|---|---|---|
| the mirror | both ways | a byte stream to the far end's mirror port on the PC |
| the node's herdr session socket | here → PC | a byte stream to the far end's herdr port, which the far end joins to the socket |
| the tool host's port | PC → here | a stream back: the PC's agent listens on the port at its loopback, each connection joined to the tool host here |
| the home bridge's socket | PC → here | a stream back to a port the near end opens here and joins to the bridge's socket; the far end's bridge socket joins the PC's loopback port |

- **The far end stands on the PC as a service of the person's own,** started by the workplace's setup and kept
  running there: the link's serve program, listening at the PC's loopback alone for the mirror and the herdr socket,
  and joining the bridge's socket to the stream back. A machine reached through the hub keeps nothing open to the
  world, since jake's agent dials out and listens at its loopback alone.
- **The near end makes no SSH connection:** it reaches the far end's ports through the hub's byte streams, and asks
  the hub to listen on the PC for the tool host and the bridge, naming targets the hub's `JAKE_REVERSE_TARGETS`
  allows (jake's `docs/architecture.md`, 「Carrying a byte stream back」). What the mirror says is the SSH link's,
  frame for frame.
- **Each connection asks the hub for every listen it wants:** the hub keeps one the agent already listens on as it
  stands, joined to the target named this time, and asks the agent only for one it does not listen on yet.
- **The bridge's stream back is joined to the bridge socket both ways,** each side's close passed on to the other
  as it comes: the hub carries what the bridge wrote ahead of its close to the asker on the PC whole.
- **The spec writer writes a jake spec for a node with no SSH login and a hub reach:** the agent the node service
  names, the hub's connect program run as the hub's executable the deployment names in `AGENCY_JAKE_HUB` with its
  `connect` mode, the hub's settings file, the far end's three ports from agency's policy, and
  the bridge's relay port here. A node whose hub program is not named, or whose setup left no relay port, has no
  spec, and the pass says why.
- **Each PC has a bridge relay port of its own here,** since each joins its own bridge socket: the setup gives a PC
  the lowest port from the policy's base that no other node's setup record holds, keeps it on every later setup,
  and writes it into the node's setup record. The hub's `JAKE_REVERSE_TARGETS` lists it beside the tool host's
  port, and the setup says either missing where the hub's settings do not.
- **Nothing of the deployment's credentials is sent to the PC** beyond what the SSH link sends a work node: the run's
  own variables, the CLI's account token for the nodes, its place and its tool token.

## What stands and what is left

| Part | Stands | Left |
|---|---|---|
| jake: streams back | the hub's listens by agent and port, refused for a target outside `JAKE_REVERSE_TARGETS`; the agent's listens at its loopback on `JAKE_LISTEN_PORTS`; each accepted connection carried to its target | — |
| the link over jake | a link spec may name a person's PC (`jake`): the near end speaks the mirror over the hub's connect program, joins the herdr socket and the bridge, and asks the hub to listen for the tool host and the bridge; the far end may listen at the PC's loopback for the mirror and the herdr socket and join its bridge socket to the stream back | — |
| the workplace on a PC | setup, check and sync reach a PC through its hub, each command a transient unit of the person's own watched until it ends: the deploy tree's commit uploaded and laid out there, the workplace's own runtime, the node binaries built there, the data root and the herdr session, and the far end standing as the user service `bmo-node-link` on the mirror, herdr and bridge ports; what the agent does not offer is said | — |
| an account's runs on its PC | agency's record of an account's node, set by the binding service or the verb; the spec writer writes a jake spec for a PC, its bridge relay port given at setup; an account's supervisors' sessions and its role agents' runs placed on its node alone, waiting while the node's link is not heard | — |
| who reaches a node | the administrator's grant written into every record agency issues for a supervisor of no account and into no other; the records the node tool reads a node's owners and a caller's account from, and one read of them with the grant and the reach; no session handed the hub's settings file's path or anything read from it, an account's sessions standing on its PC, and the home bridge of a person's node running the policy's verbs alone; agency's setup, check, sync and link reaching a hub with a credential scoped to the node's agent, made from the hub's control key, and refused 403 `agent-out-of-reach` on any other agent | the hub's settings file belongs to the user every unit and session here runs as, so a session of this machine, a run of an account with no node among them, reads the token without the tool; the token held by a user of its own, which the hub, the tool's executable, agency's node verbs and the node service alone run as, closes it |
| a check on a real PC | a PC attached as a jake agent with its stream and listen ports set; its workplace ready, set up through the hub and its copy kept on the deployed commit by the links pass's sync; the server writing its jake spec and holding its link, the mirror, the herdr session, the tool host and the bridge each answering through it; an account bound to it whose supervisor's folder is made on the PC, whose program starts there past its first-run screens, and whose turn is put in once its status line, answered through the bridge, is drawn, so its BMO answers from the PC | — |

