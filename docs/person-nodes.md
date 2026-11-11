# A person's PC

A person's PC is the node their BMO runs on. They turn the PC on, jake attaches it to the hub, and agency stands
their account's sessions there, through jake alone, with no SSH. The sessions run on the PC; what they run on — the
providers and their accounts, the model gateway, the brain, the tool host — is the deployment's. It is the shape of a
desktop coding agent whose models and tools are served from elsewhere.

## Which node is a person's

- **A person's node is the hub node whose agent is `u<user>`,** the machine their account was bound to when its
  invitation was proven (the account service's `docs/accounts.md`, 「Invitations and a person's machine」). The node
  service lists it as any hub node (`docs/work-nodes.md`, 「In herdr」).
- **An account's runs go there and nowhere else:** its supervisors' sessions and its role agents' runs. A run of an
  account whose node is not attached waits; nothing of it runs on this machine or another account's node. A run of no
  account is placed as before.

## What the PC needs

- **A Linux shell:** WSL on a Windows PC, a Linux PC as it stands. herdr and the programs run there, as on every work
  node (「The workplace on a node」 in `docs/work-nodes.md`).
- **jake, installed by the invitation,** and nothing else installed by hand: agency lays the workplace down through
  jake — the tools, herdr, the programs and the data root — as setup lays one down on an SSH node.

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
- **Nothing of the deployment's credentials is sent to the PC** beyond what the SSH link sends a work node: the run's
  own variables, the CLI's account token for the nodes, its place and its tool token.

## Order of the work

1. jake: streams back (jake's `docs/protocol.md` and `docs/architecture.md`).
2. agency: the link's far end listening at the PC's loopback, and its near end over jake, beside the SSH link,
   chosen by the node's kind.
3. agency: the workplace laid down on a PC through jake.
4. agency: an account's sessions and runs placed on its person's node.
