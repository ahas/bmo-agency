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
    record and its transcripts brought back.
- **While the node is not attached, its runs wait:** a turn of the account asks again every few seconds whether the
  node's link has been heard from, and starts once it has; a head or a room pane of a supervisor of the account is
  not stood until then. Nothing of it runs on this machine or another account's node. A run of no account, or of an
  account with no node, is placed as before.

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
names for the node: its control address, the file its control token is kept in, and the agent's name.

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
- **A listen is asked for only where it does not stand:** each connection reads the hub's listens for the agent
  first, leaves one standing on the target wanted as it is, since asking the agent again for a port it already
  listens on ends that listen, and takes away and asks afresh one the hub holds that does not listen.
- **The bridge's stream back stays open after the answer until its asker closes it,** at most half a minute: the
  hub ends a stream back on its target's half-close ahead of what the target wrote just before it, so the bridge's
  end is not passed on at once.
- **The spec writer writes a jake spec for a node with no SSH login and a hub reach:** the agent the node service
  names, the hub's connect program run as the hub's executable the deployment names in `AGENCY_JAKE_HUB` with its
  `connect` mode, the hub's token file as the hub's settings, the far end's three ports from agency's policy, and
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
| a check on a real PC | a PC attached as a jake agent with its stream and listen ports set; its workplace ready, set up through the hub; the server writing its jake spec and holding its link, the mirror, the herdr session, the tool host and the bridge each answering through it; an account bound to it having its supervisor's place marked for the PC, mirrored there and its program started in the PC's herdr session | the account's BMO answering there: the supervisor's own folder is not made on the PC; the configuration directory the PC's programs run under is neither marked as opened nor trusting that folder, so the program stops at its first-run screens; a program past them is put no prompt; the supervisor's hooks name a `bmo-agency` a PC workplace does not build; and the sync the links pass starts for a PC finds no node service |

