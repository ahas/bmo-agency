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
| the node's herdr session socket | here → PC | a byte stream to the PC, joined there to the socket |
| the tool host's port | PC → here | a reverse stream: a port the PC's agent listens on, each connection carried to the tool host here |
| the home bridge's socket | PC → here | a reverse stream, as the tool host's |
| the mirror | both ways | a channel to the serve program started on the PC: its standard input and output carried both ways |

- **jake gains what this needs, as capabilities of the agent the machine's configuration allows:** a reverse stream,
  to a target the hub's configuration names, from a port the agent listens on at the PC's loopback alone; a channel,
  a command whose standard input is carried as well as its output, held for as long as the link holds it, with no
  ceiling of time; and a stream's far end that is a local socket as well as a port.
- **Nothing of the deployment's credentials is sent to the PC** beyond what the SSH link sends a work node: the run's
  own variables, the CLI's account token for the nodes, its place and its tool token.

## Order of the work

1. jake: reverse streams, channels and socket ends (jake's `docs/protocol.md` and `docs/architecture.md`).
2. agency: the link over jake, beside the SSH link, chosen by the node's kind.
3. agency: the workplace laid down on a PC through jake.
4. agency: an account's sessions and runs placed on its person's node.
