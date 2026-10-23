# Own LLM services

Agency reaches models of the deployment's own choosing, outside the Claude, Codex and OpenRouter
roads, as registered models.

- **The registered models** are every road to a model the owner registers — an OpenAI-compatible chat
  completions address, an optional key, the model's name and its sizes — which one provider of
  agency's, the registry's, offers on every line it carries. A registration is the whole declaration
  of such a model: one is added, changed and removed with no commit and no deployment, and no model,
  address, window, answer size or wait of one is written in the code.
  - A run with tools — the chat, a worker, a companion's turn — is Claude Code pointed at
    agency's gateway under the registration's name (「How requests go」 below).
  - A request with no tools — a gate — is one request to the gateway's chat completions path under the
    same name, asked in the agent's host (「A request with no tools」 below).
- **The model catalogue** holds the model files the deployment's own engines serve, each with where it
  comes from, where it lies and how it is asked by default; choosing one for an engine fetches, checks
  and installs it (`docs/model-catalogue.md`).
- **A gate's own classifier provider,** where a gate's profile offers one, answers at an address of the
  same shape but is no registration: it answers a gate's question with probabilities rather than text,
  and is a gate's provider of its own (`laya.md`).

## A request with no tools

### What it answers to

- **The call:** every such request goes to the gateway's `/v1/chat/completions` with the
  registration's name as its model, the system prompt as the first message where it is not empty, then
  the input, `max_tokens`, and `enable_thinking` in `chat_template_kwargs` as the run says. No request
  names a sampling setting unless the run carries one, so otherwise the engine's own defaults hold.
- **No line of a run with tools:** a gate is asked with no tools, so the request is the whole of the
  run and no program stands in a pane for it.
- **The model list:** the gateway's authenticated `/v1/models` names every registration, and a
  request's registration must stand in it (「The model list」 below).
- **The context window is its registration's:** every request is sized to the window of the
  registration it names (「The context window」 below).
- **Size refusals** come back as a 400 that names the context, and are read as too large (「Input
  size」 below).
- **The key:** the gateway's own, read for each request (「The keys」 below).
- **No state on agency's side:** no session, no prompt cache, no keepalive. An engine behind a
  registration may keep its own state, such as a per-conversation slot.

### What the code still declares

- **The providers are programs, so they are code:** Claude Code, Codex, OpenRouter and the registry's
  provider are each a program or a client with its own launch line, its own records and its own reading
  of an answer, and a build carries the ones it was written with.
- **Which lines the registry's provider carries:** the chat, the worker, the gates and the
  companions. A line is a shape of the deployment's own work, so it is the deployment's to say
  which provider carries it.
- **The gateway's address and key file,** read in one place by every caller (「The gateway」 below).
- **Nothing else:** no model, address, window, answer size, wait or engine is named in the code beyond
  the seeds the store starts with (「The seeds」 below), which are the owner's once written.

### The own model and the engine behind it

A registered model is what every line on the registry's provider runs on. Its model refuses nothing of
its own, so what it is told is the line it holds, and the line a run goes out on decides what that is,
never the model's name.

- **On the worker line it is a worker.** A worker's jobs and the words said to it are handed the job
  worker line whole, and what that line appends closes on that worker's own rules
  (`docs/launch-lines.md`): it answers as a worker of the deployment's, never as the harness it runs
  on, whichever model that is; and it takes no destructive action and none that harms anyone else
  without the owner's order or permission, which only the owner's own words in the item give.
- **Its system prompt opens on its base prompt** (`docs/base-prompts.md`): where the base prompt is a
  harness's own prompt, that prompt is kept whole in the store and handed to the program as the whole
  of its system prompt, with the worker's rules after it. The harness, the tools, the rules on
  confirming and reporting, and the context management stay, since the program needs them to run; so
  does the program's own opening line, which the identity layer the rules carry answers. Another prompt
  chosen in its place — one written for another harness — is handed the same way.
- **On the chat line it is the chat agent.** A chat round on a registered model is handed the chat
  agent's system prompt exactly as that agent is, with nothing of a worker's or a companion's in it.
- **As a companion,** on the chat line, what a turn is given is the companion's own to say: its base
  prompt, then the pieces of its settings and its persona, in place of a harness's own prompt
  (`docs/companions.md`). Nothing of the chat agent's prompt or of the worker's rules is among them.
- **The rule against destroying the system is on every line that acts,** since every agent that acts is
  given the self-preservation rule (`prompts/rules/self-preservation.md`): the worker and chat lines,
  a companion's prompt after its base prompt.
- **An agent's rules follow the agent, not the model:** a support worker is handed its base's rules,
  and another agent's run on the same registered model is not.
- **A worker's session closes with nothing handed over.** As a worker's job ends, the session it ran
  in is handed over — closed — and the next run opens on its own words alone: no note of the closed
  session, the worker's own or any other, is carried (`docs/sessions.md`, 「Session replacement」).
- **A research run keeps to the registration where its registration keeps its research.** A search or
  a browse launched by a run on a registration that keeps its research goes out on that registration
  too, rather than on the research keys' model, and a deeper depth has it search more itself instead of
  opening co-workers (`research-runs.md`). What any other run sets off is untouched.
- **Thinking is the only reasoning setting of a registered model:** each registration carries a
  thinking default, which a chat turn on it follows, and an agent's own thinking setting overrides it
  where it names one (`docs/model-tiers.md`). A registered model takes no effort level.
- **Each line samples by its own settings:** a worker by its own sampling, a companion by its
  generation settings (`docs/companions.md`), a gate by its own profile's (「Sampling per gate」 below);
  a chat round names none, so the engine's defaults hold.
- **The model behind a registration is the engine's, and the engine's model is a catalogue entry.** The
  deployment's own engine serves the file its placement names — an entry of the model catalogue
  (`docs/model-catalogue.md`) — and every request reaches whatever file that is, since the gateway
  sends the registration's name and the engine serves one model. How the engine is served, the hardware
  it stands on and how fast it reads and writes are the node's own (`docs/work-nodes.md`,
  `docs/model-catalogue.md`), not agency's: agency knows a registration, its address and its sizes, and
  nothing of the weights behind it.

### The model list

- **The gateway's list names every registration** and every node of one, and a request takes from it
  its own registration alone: the list must name it, and nothing asks another.
- **Checked on call:** a request whose registration the list does not name is refused as a changed
  list, and the run returns to the existing run policy.
- **Caching the list:** the list is kept for one minute, and the first request after that minute
  fetches it again, since the list rarely changes and fetching on every request would make each gate
  send two.
- **One time limit:** the list fetch and the request share the same time limit.

## Registered models

### What a registration holds

- **Its name:** lowercase letters, digits and hyphens, beginning with a letter, unique in the registry.
  It is the name the gateway serves it under, the model the registry's provider lists, and what a tier
  mapping, a worker's pin and a held session name. It is fixed once made.
- **Its label:** how a screen shows it.
- **Its addresses,** one or more, each a node the same model is served on: an OpenAI-compatible chat
  completions address ending in `/chat/completions`; a short label, unique within the registration,
  that names the node; and a key or none (「The keys」 below).
- **Its model:** the model's name as every one of its addresses serves it.
- **Its context window:** a whole number of tokens within the bounds the policy sets (「The context
  window」 below).
- **Its answer size:** the answer ceiling of a run on it where the key's tier names none, a whole
  number of tokens within the policy's bounds and smaller than its window.
- **Its request wait:** how long one request is waited for, long enough for a whole default answer at
  the model's speed. Claude Code waits it, and so does the gateway behind.
- **Its thinking default:** on or off, what a run on it does where its tier mapping leaves thinking to
  the registration.
- **Whether it is walked:** whether the automatic walk asks the registry's provider when a key's tier
  maps to it (「Own services in the provider order」 below).
- **Whether it keeps its research:** whether a search or a browse set off by a run on it runs on it as
  well, opening no branch and searching more itself as the depth rises (`research-runs.md`). Off, a run
  on it sets off its research as any other run does.
- **Nothing of a run's prompt:** what a run on it is told is its line's, and an agent's base prompt is
  the agent's own choice (`docs/base-prompts.md`); the gateway passes every prompt as it stands.

### Where it is kept

- **In the store:** one table of registrations and one of their addresses, each address row belonging
  to its registration and removed with it.
- **At the boundary:** `/models` is the model resource — the list, the create, the read, the change and
  the take-away all read or write these rows, so what is listed is what can be changed and removed. The
  model catalogue is another resource, `/model-catalogue` (`docs/model-catalogue.md`).
- **Read afresh by every run and every screen,** so a saved change reaches the next run with no
  deployment and no restart of the bot. What the gateway carries — the addresses, the keys, the model
  and the wait behind — reaches it as 「The gateway」 below says.
- **A registration with no address** is listed, and offered nowhere: no tier, pin or run may name it,
  and the gateway's table leaves it out.

### The context window

- **Each registration's own,** read afresh by every run on it: the window Claude Code is told (「How
  requests go」 below), and the room the answer of a request with no tools is given (「Input size」
  below). A change reaches the next run, with no deployment.
- **Never empty:** a registration holds a window from the moment it is made, and a save that leaves it
  out or gives what it does not take is refused, so what the console and the command show is what runs.
- **The model list's own window sizes nothing:** a `max_model_len` a list carries is not read, so the
  owner may hand a run less than the model serves.
- **A request with no tools run in its agent's host** is handed the window the bot read, since the host
  holds no store.
- **A process holding no store** has no registry to read and refuses a request with no tools, naming
  its registration; nothing stands in for the window.

### The seeds

- **The registrations the deployment's own models start with** are written once, when the registry's
  tables are made, and are then the owner's like any other; a seed removed stays removed.
- **A field the registry gains later** is given each seed's value once, as its column is made, on the
  registration of the seed's name where it still stands; every other registration takes the field's off
  value.
- **The window each seed starts at** is the value its seed holds, or the one an older store's own
  setting held for the same model where it held a window the registration takes.
- **Every node takes its key:** a request to a node's front with no key is refused, so a seed's address
  carries a key even where its model runs on the deployment's own machines.

### A worker's own registration

- **A registration the owner makes** with the registry's command, like any other and not a seed, may be
  the model a worker is pinned to (「A worker pinned to a provider」 below), while a companion stays on
  another. What this document says of the worker line holds there alike: the worker reads the job worker
  line and the model it is pinned to answers as that worker.

### Where it is set

- **The console:** the registered-models section of the agent management screen's general tab lists
  every registration and adds, changes and removes one; the same screen's tier table and a worker's
  settings dialog offer every registration where a model is chosen.
- **The command:**
  - `bmo-agency agent --registered` lists every registration and its addresses, each address with
    whether its key is usable.
  - `bmo-agency agent --register <name> [--label <label>] [--model <model>] [--context <tokens>]
    [--answer <tokens>] [--wait <seconds>] [--thinking on|off] [--walked on|off] [--research on|off]`
    makes a registration, every field but the label named, or changes the named fields of one.
  - `bmo-agency agent --register-address <name> <node label> <address> [--key-stdin | --no-key]` adds
    an address or changes one; `--key-stdin` reads the key from standing input, and a key is never
    taken on the command line.
  - `bmo-agency agent --unregister <name> [<node label>]` removes a registration, or one of its
    addresses.
- **What a removal is refused for:** a registration a tier mapping, an agent's pin — a worker's or a
  companion's — a work key's own registration or a model catalogue entry names is not removed, and the
  refusal names each of them;
  each of those answers for itself whether it names the registration. The last address of a
  registration something names is not removed either.
- **The rules a save is held to** — what a name, a node label, a model and a key file may be, the
  bounds of the window, the answer size and the wait, and how long a label may be — are agency's policy
  (`policy/`), so the console's save is checked alike whichever side answers it.

### Nodes and sessions

- **Why a session keeps to one node:** a node saves the state of the conversation it last answered to
  its own disk and loads it back for the conversation's next request (`docs/work-nodes.md`). A request
  that reaches another node is computed from its first token, which on a long conversation is minutes
  of reading where the saved state loads in a fraction of a second.
- **The gateway serves a registration twice over:** under its name, spread over all its addresses, for
  a request that belongs to no session — a gate, a model list; and under `<name>@<node label>`, one
  address alone, for each address of a registration that has more than one.
- **A program is placed on one node:** a Claude Code program started for a new session on a
  registration with several addresses is given the name of one node, and every request of that session
  asks that node alone.
- **Which node:** the one given a new session least recently, which the registry keeps for each
  address, so sessions spread over the nodes. No node is probed first: a card another service holds
  shows nothing a probe could read until a request waits on it.
- **A new session its node refuses goes to the next node:** where the first run of a new session is
  refused by its node before any answer — a 502 or a 503, as a node answers when another service holds
  its card past the wait — the same run is placed again, by the same rule, until one answers or every
  address has been tried once in that run. The refused node was just given a session, so the next ones
  pass it by too. A resumed session is never moved, since its state is on its node, and a run that timed
  out is not placed again, since another node would have to compute it from the start.
- **A resumed session keeps its node:** the record of a held session keeps the node beside its model,
  as it keeps its login, and a program started to resume the session, or to fold it, is placed there
  again. A session whose node has left the registration is not resumed; the run opens a new one.
- **A registration with one address** has no nodes to choose from, and its runs name it alone.

## What leaves the machine

- **A registration sends its runs to its addresses,** wherever the owner points them: an address off
  the machine is a service that receives everything its runs carry — a gate's system prompt and input,
  and a run with tools' prompt and every file and command output its tools read.
  - Where that address is an external operator's service, its retention is the operator's word, not a
    boundary agency enforces locally, and the owner is warned of it where the service asks for no
    personal data.
- **A registration on the deployment's own machines stays on them:** its requests go to loopback — the
  gateway, and behind it the engine on its node and the cards it uses on the internal network — and
  what each hop logs of a request is the node's own concern (`docs/work-nodes.md`).
- **What the gates that may stand on an external registration send about people:** the person-book
  search sends person-book indexes and pieces; the translator sends parts of documents
  the console asked to translate, a person book's text as it is where that is the document; the document
  seek gate sends document indexes and the owner's question.

## The keys

- **Where they live:** under the data root's `env/`, outside the store and the checkout, each file at
  permission `0600`.
- **A registration's key** is one per address. The address row holds the name of the file under `env/`,
  never the key.
  - A key given on the console or the command is written by agency into a file of its own under
    `env/registered/`, named for the registration and the node, at `0600`, and a key given again
    replaces it.
  - A seeded address names a file that was already there — a node's token.
  - A removed address or registration takes away the files agency wrote for it, and leaves a file it did
    not write where it is.
- **Who handles the files:** the harness-machine crate writes, takes away and checks them, for agency's
  server (`docs/harness-machine.md`, 「The keys」).
- **Where a key never goes:** the store, logs, API responses, the browser, a command line. The console
  and the command receive, for each address, only whether its key is usable; a key reaches the bot once,
  in the body of the save that gives it, and is never sent back.
- **How Claude Code is handed one:** its settings name a key helper that prints the gateway's key file,
  which the program runs for its requests, so the key is read out of the file each time and stands in no
  command, environment or record of agency's.
- **How the gateway is handed the registrations' keys:** its launcher reads each file into its child's
  environment alone (「The gateway」 below).

## The gateway

- **What it is:** a LiteLLM proxy behind a front of agency's, standing as its own user unit on loopback
  alone. It serves the OpenAI-compatible API and Anthropic's Messages API in front, and asks an
  OpenAI-compatible backend at its chat completions path behind, translating the Messages API both ways:
  tool-use and tool-result blocks against tool calls, the streamed events, and an error in the shape the
  caller reads.
- **Where its callers find it:** every caller — a run with tools and a request with none alike — reads
  the gateway's address in one place, an environment variable where the process names one and a loopback
  default otherwise. The bot names none. A check that stands a launcher of its own on another port names
  it for its own runs, so they reach that launcher and never the one serving the bot.
- **Who goes through it:** every request on a registered model.
  - A run with tools, on the Messages API: Claude Code speaks nothing else, and a registration is an
    OpenAI-compatible address.
  - A request with no tools, on the chat completions path and the model list.
- **Its table is written from the registry:** the unit's process is agency's gateway launcher, the
  `llm-gateway` binary. It reads the registry out of the store, at the address `BMO_ARCHIVE_URL` names
  or else the one the data root's `env/archive-url` holds, opened read-only for each reading, so it never
  holds a lock the bot waits on. It writes the table under the data root, private and renamed into
  place, copies the hooks beside it from the gateway crate's `hooks/` folder in the tree it runs from,
  and starts LiteLLM on it as a child on a free loopback port. The table names each key by a variable
  the launcher sets from the key's file in that child's environment alone, and never a key; the child's
  environment holds nothing else but its `PATH` and `HOME`.
  - A registry not settled yet reads as nothing to serve for now, and is read again at the next beat.
    A launcher with no store address named at all does not start.
  - A child is ready once its readiness check answers; one that ends or stays silent first is given up,
    and the child serving goes on serving.
  - Tables left by a launcher that ended without stopping its children are removed as it starts.
- **The launcher is the front:** it listens on the gateway's loopback port itself and streams every
  request through to the child it serves from, and every answer back, as they stand, reading neither. It
  counts the requests in hand through each child.
  - The hop-by-hop headers are not handed on either way, a request asks for its answer unencoded, and an
    answer's content-encoding is not handed back.
  - Before any table stands it answers `503`, and where the child cannot be reached `502`, each with a
    line saying so.
  - A caller that leaves closes the front's connection to the child and its place among the requests in
    hand; LiteLLM still holds the node until its own wait ends.
- **What the table holds for a registration:** one entry under its name for every address, and, for a
  registration of several addresses, one more under `<name>@<node label>` for each (「Nodes and
  sessions」 above). Each entry asks its address's chat completions path, with the address's key, for the
  registration's model, waits the registration's request wait for a whole answer and for a stream alike,
  and sends `enable_thinking` off in `chat_template_kwargs` where a request names nothing.
- **A request is waited for as long as its caller waits, and never longer.**
  - A node sends nothing, not even its response headers, until it has read the whole prompt, so a long
    prompt is most of an hour of silence.
  - The gateway does not pass on a caller that leaves: a request whose caller has gone goes on holding
    the node's card until the gateway's own wait ends, and its own wait ending closes the connection to
    the node.
  - So every request names its caller's wait in the gateway's per-request wait headers, and the gateway
    gives up where the caller does. The gateway's address and these words are read in one place by every
    caller.
  - An entry's own wait is that of a request naming none. The gateway-wide request wait is not read on
    the Messages path, so the table sets none.
- **What holds for every entry:**
  - An OpenAI-compatible backend is asked at its chat completions path; left to itself the gateway asks
    the Responses API, which an engine's front may answer with a 404.
  - A parameter a backend does not take is dropped, and a failed request is not retried there: the run
    that asked reads the failure.
  - Thinking off where a request names nothing, and a request that names `enable_thinking` in
    `chat_template_kwargs` — a gate's, as its tier says — is asked as it named it. Left to the template's
    own default a model may think for minutes even on a turn that asked for none, so the default is never
    the template's.
  - **The caller decides thinking on the Messages path, and the gateway carries it.** LiteLLM translates
    a Messages request's `thinking` into an OpenAI reasoning effort the template does not read, so a hook
    of the gateway's own rewrites a Messages request before it is routed: `thinking` of type `enabled` or
    `adaptive` becomes `enable_thinking` on, `disabled` becomes it off, and a request with no `thinking`
    keeps the table's off. The `thinking` field and the effort are taken out, the Anthropic form of a
    choice already carried, and the model's thinking comes back as the answer's thinking block, streamed
    or not.
  - **The system prompt passes as it stands.** What a run is told is composed before the program starts
    — its line's prompt, or the agent's base prompt with its rules (`docs/base-prompts.md`) — and no hook
    reads or rewrites it.
  - **A system turn inside the conversation is folded into the person's turn beside it.** Claude Code
    sends some of what it tells a run as system-role messages among the conversation's own, and LiteLLM
    turns each into a user message, since a backend's chat template takes a system message only at the
    head; standing after the person's words, such a message is the last thing the model reads, and a
    model may answer it by repeating it. So a hook rewrites a Messages request before routing: a run of
    system-role messages right after a user message becomes text inside that message, each wrapped in
    reminder tags, ahead of the person's words and after any tool results it opens with; a run with no
    user message before it goes into the one right after; a run between two answers is left to LiteLLM's
    own turning. The hook reads nothing but the request's own order, so the conversation sent again on
    the next request is folded the same way and the engine finds the state it saved.
  - **A run's sampling settings are carried from a header into the request.** Claude Code has no word for
    most of what the backend samples with, so a run that names sampling settings sends them on every
    request in a sampling header — a JSON object of the backend's own parameter names and numbers, with
    the chat template's words under `chat_template_kwargs` — and a hook writes each into the request body
    before routing, over the program's own: every numeric entry so named is written; the template words
    are laid over the request's own and those over its table entry's, word by word, since LiteLLM sends a
    request's template words in place of its entry's whole; which settings are sent and their bounds are
    the caller's (`docs/companions.md`), and the hook keeps no list. A request with no such header is
    asked with the program's own values.
  - **Past thinking goes where the template is told it goes.** A request whose template words name
    `preserve_thinking` off has the thinking of every answer before its last question taken out before
    routing, so the program's history carries no more of it than the template renders; a request naming
    it on, or naming nothing, keeps the history whole.
  - **A run's long tools are carried from a header into the request.** A run whose program names the
    tools that wait long sends them in a long-tools header (「How requests go」 below), and a hook copies
    the header's value into the request body as one string of comma-joined names; the engines on the node
    pool take the field out before the request goes on to the model server behind them. LiteLLM's own
    forwarding of a caller's headers is left off, since it hands the backend every such header with no
    way to name one alone and a caller's key may stand in one.
- **A saved change reaches it within seconds, and no request is cut:** the launcher reads the registry
  every few seconds. Where the table it would write differs from the one it serves, it starts a new child
  on a fresh port with the new table and waits until that child answers its readiness check; it then
  sends every new request to the new child, and stops the old child once the requests in hand through it
  have ended, however long they take. A change that leaves the table as it stands starts nothing.
- **What the console is told:** the launcher writes a small status file under the data root — which
  table it serves, whether a new child is starting, and whether an old one is draining — and the
  registered-models section says the gateway is settling while a new child is starting, and nothing once
  it serves.
- **A new table is a new child** because LiteLLM loads the models of a table only as it starts; it takes
  models in while running only from a database, which this gateway does not keep.
- **Why LiteLLM:** it is a translator and nothing more, serves both APIs from one table in one file, and
  runs as one process with no database.
- **Its own key:** a key file read by the launcher into each child's environment as it starts it.
  Without that file, or with one that is empty or not `0600`, the launcher does not stand.
- **A request without the key is answered `500`, not `401`,** since LiteLLM's own refusal path imports a
  database client the installed LiteLLM does not carry; a wrong key is answered `400`. The front hands
  both back as they came; a caller with the key is answered.
- **How it stands:** the gateway's unit is linked into the user's systemd directory and enabled by
  the deployment, and carries the restart mark, so every deployment restarts it with agency's other
  services. That restart is the one moment the front itself goes down, and the requests in hand then
  are cut with it.
- **A changed hook needs no restart:** the table's digest covers the hooks' contents, so the next
  reading of the registry starts a new child on them and hands over the way a changed registry does.

## Routes and modes

### The route table

- **One row per work key** in the run route table.
- **What a row holds:** the key's run mode, and — where the owner pins one — the registration the key
  runs on at the registry's provider (「A key's own registration」 below).
- **Run modes:** automatic, or one provider — Claude Code, Codex, OpenRouter, the registry's provider
  (`docs/cache-fallback.md`, 「The per-key choice is automatic or one provider」). A key's row set to a
  provider that does not carry its line is refused on save.
- **What the row does not hold:** the model on any other provider, the answer size and thinking — the
  tier settings decide them (「Settings per tier」 below, `docs/model-tiers.md`) — and a gate's time
  limit, which is a separate table (「Per-gate time limit」 below).
- **A key with no row** reads the profile's initial values.

### A key's own registration

- **One key may name one registration** in place of its tier's registry cell. It stands for that key
  alone: every other key assigned to the tier keeps the cell, so a gate can move to a registration of
  its own without moving the tier.
- **What it may name:** a registration the registry holds with an address, checked on saving; none takes
  the pin away, and the key reads its tier's cell again.
- **Where it stands:** wherever the key is resolved on the registry's provider — its own runs on that
  provider, the model a gate's request names to the gateway, whether the automatic walk asks the
  registry for the key, and the route `GET /work-keys/{workKey}` answers. A resolution that names a tier
  of its own reads that tier's cell. The registration's answer size and thinking default are its own,
  unless the key's tier cell maps to that same registration, whose answer size and thinking then stand,
  as a worker pinned to a registration takes them (「A worker pinned to a provider」 below).
- **It is no run mode:** the pin chooses the registration and nothing else, so a key on automatic
  reaches it only where its walk reaches the registry's provider, and a key set to another provider
  runs there.
- **Where it is set:** the route row's change on the console, or with this command, `auto` taking the pin
  away; the command may set the key's provider in the same line:

```
bmo-agency agent --route <work key> [--provider registered] --registration <registration|auto>
```

### Initial modes

- **The registry's provider:** the reference gates and the other gates each start on the registration
  their tier maps to there, as the tiers are seeded (`docs/model-tiers.md`).
- **Automatic:** the voice gates.
- **A gate's classifier provider:** the formatting gate, and the reference gates and matchers that offer
  it, each start on their provider and are offered the classifier too (`laya.md`).
- **No key that is no gate starts on the registry's provider:** the chat or the worker key is
  put on it only by the owner's choice, and the automatic walk reaches it as 「Own services in the
  provider order」 below says.
- **Owner changes persist:** rows the owner changed stand as they are on the next start.

### Choosing

- **Where:** the run-route and gate tabs of the agent management screen, or with this command:

```
bmo-agency agent --route <work key> --provider <auto|claude-code|codex|openrouter|registered|laya>
```

- **Model check:** a registry cell's registration is checked against the registry on saving.
- **Display:** the management screen and the `bmo-agency agent` listing show the configured
  provider/model beside the road that actually answered recently; the gate test screen's title bar reads
  the same settings.

## Limits, retries and walking

### Request limits and retries

- **Limits:** a request with no tools is limited in input size, output tokens, the byte count of success
  and failure responses, retry count, and time.
- **Retried:** only transient network errors, 408, 429 and 5xx.
- **The time is the request's own:** each attempt is waited for the time the request has left, and for
  no less. The runtime's own cut of a connection silent for minutes is switched off for it, since an
  engine reading a long prompt sends nothing for longer, and the gateway is told the time left
  (「The gateway」 above).
- **No body in the log** for a missing key or permission error, 401/403, timeout, overflowing response,
  empty answer, or malformed JSON.
- **A retry asks the same registration again;** it is not a move to another model.

### When a registered model does not answer

- **A key set to the registry's provider does not walk.** If it does not answer, that run fails.
- **An automatic key** reaches a registered model only as a turn of its walk, and moves on to the next
  turn when it does not answer (`docs/cache-fallback.md`).
- **Gates do not walk** (「A gate has one road」 below).
- **A provider disabled in the provider order:** even a key set to it does not ask it; it walks like
  automatic.
- **A job worker's run** uses only the one provider it was given; moving to the next provider is job
  recovery's job.

### Own services in the provider order

- **The registry's provider is one turn,** after OpenRouter: the default order is Claude Code → Codex →
  OpenRouter → registered models. Its switch there governs every key set to it, a gate's and a
  companion's among them.
- **The walk asks it only for a walked registration:** an automatic key reaches the registry's provider
  only where the registration its tier maps to there is walked, and otherwise passes it by. A walked
  registration is the walk's last turn, so a key whose other providers have all failed is answered on it,
  sharing its one slot with everything else on it. A registration that is not walked stays registered,
  reached only where the owner names it.
- **How it answers there:** with the registration, thinking and answer size of the key's tier.
- **The walking rules:** order, enablement and walking are in `docs/cache-fallback.md`.
- **Nothing to keep warm:** a registered model has no known cache lifetime, so it sends no keepalive and
  produces no cost or account window.

## A gate has one road

### How the services behave

- **One request is one HTTP POST.** There is no session and no prompt cache, so the gate's system prompt
  is carried whole every time.
- **The request is the gate's alone:** its system message is the gate's own instructions and its one
  user message the gate's input. Nothing of the agent whose turn it judges goes with it — no persona, no
  room context, no companion's settings — and it joins no conversation on the same registration.
- **An engine with one slot answers one request at a time,** shared by every line on it — companions'
  rooms, workers' jobs, every gate on it and every key the walk brings to it. A gate waits behind
  whatever holds the slot, a long answer being written holds it for minutes, and a gate that takes the
  slot from a conversation leaves the front to save that conversation's state and load it back on its
  next request. The engine stays up between requests; one that finds it stopped waits for its start.

### A gate runs on its one provider

- **Which provider:** if the gate's run mode is a provider and that provider is enabled, the gate's road
  is that provider; otherwise it is the first enabled provider in the provider order.
- **One ask, and that is all.** It never moves to another provider or model — not on timeout, busy,
  refusal or error.
- **A gate whose provider is a classifier has one LLM behind it, and only that.** Where the classifier
  gives no verdict, the gate's fallback LLM is asked once, on its own time limit, and its verdict stands;
  where it gives none either, the gate's default does (`laya.md`). That LLM is the gate's own setting:
  unset, the provider the gate's road would begin on with its tier for the gate; none, no LLM at all; or
  a provider by name, with its tier for the gate. The LLM's own ask follows every rule here — one
  provider, one ask, nothing behind it.
- **Why:** waiting for one verdict costs a room more than the value of that verdict, and with one road
  the time a verdict took is the time that road took, so what is slow shows plainly in the record.

### Sampling per gate

- **Each gate samples by its own profile,** in agency's policy, each setting by the name the engine reads
  it under: never the engine's defaults, which are a conversation's, and never a chat's or a companion's
  settings. A verdict gate samples cold, a faithful rewrite a little warmer, and a gate rephrasing for
  the ear warmer still, each with the sampling its model's family is advised for a run without thinking.
- **How it travels:** a request with no tools names it in the gateway's sampling header, which the
  gateway's sampling hook writes into the request's body (「The gateway」 above).
- **A road with no such words** — a gate on Claude Code — takes none: the program fixes its own.

### A gate that did not answer is a gate with no answer

- **A failure ends as a failure on the spot.** The gate run is not rerun whole, and the same thing is
  not asked of another model.
- **Retries within the same provider** — transient network errors and 408, 429, 5xx — follow that
  provider's rules. If they bring no answer, the gate has no answer.
- **Each caller keeps its own fallback** for a gate with no answer: a formatting gate not answering sends
  the answer without a formatting verdict; a reference gate or the person-book search not answering
  attaches the index that gate read instead of pieces.
- **What is lost is one verdict,** not the room's answer.

### A gate somebody stopped is not a gate with no answer

- **A stop is not a verdict that did not come.** The run was ended on purpose, so nothing stands in for
  it: the road records a stop and the gate raises it to whoever asked (`docs/agent-control.md`,
  「Stopping」).
- **It is carried nowhere.** Not to the next login of the same provider, not to another provider, not
  asked again. Whatever the gate had walked up to that point is the whole of its record.
- **Each caller's own ending under a stop,** since it arrives as a raised stop rather than a missing
  verdict: a formatting gate ends where it stands and the message goes out as the writer wrote it; the
  reference gates and the person-book search end the round as a stop rather than going on with the index;
  the sanitiser ends the round as a stop rather than redacting locally; the translator and the voice
  reading write or read nothing and say they were stopped; and every other caller catches nothing of its own, so the
  stop ends the piece of work that asked for the judgement.

### Each gate uses its whole time limit

- **Nothing to share it with:** there is no road after a gate, so a gate's time limit has no reason to be
  divided.
- **All of it goes to its provider:** the gate gives its whole time limit to its provider, and ends in a
  timeout if no answer comes within it.
- **Set per gate** (「Per-gate time limit」 below).

### Which road answered is recorded

- **One line per road walked** by a gate run, in the gate road record.
- **What a line holds:** work key, room and message, provider and model, outcome (answered, timed out,
  size refused, error, none), the time taken, and the input, output, cache-read and cache-write tokens
  the run reported (empty where it reported none), and, on a line of a session's own run, the share of
  its context window that run ended carrying (empty on a gate's line and wherever the session reported
  none). The room screen's context chip reads it.
- **What a registration reports:** the chat completions path answers with its token usage, and that is
  kept.
- **A chat turn is a line too,** under its own work key, with the room and the last message the turn
  read, so a turn's figures and its gates' figures are read from one record. A second asking the round
  makes of itself is a line the same way, under the key for the kind of second asking it is. A turn or a
  rewrite whose call failed still writes its line, with the outcome error and no tokens.
- **The round and the run:** a line written inside a round of a room's session names that round, so
  everything one answer cost is read by that name, and names the run its log lines are kept under; a line
  written outside any round names none.
- **How many lines:** there is one road, so one run has one line. Only a run split by size has one line
  per part.
- **Two records together:** the time in the judgement record is the whole stage's time; the road record
  says which road that time was spent on.

## Input size

### The limit is the context

- **The model runs only within its context limit,** counting system prompt, prompt and answer together.
- **The limit is the context window of the request's registration** (「The context window」 above), read
  at call time.
- **Token-heavy text fills context fast:** some scripts are one or two characters per token.

### Sizing the answer

- **The answer slot** (the request's `max_tokens`) is sized to the input every time.
- **Estimating input tokens from characters:** a non-ASCII character counts as 0.8 tokens, an ASCII
  character as 0.3, which comes out slightly above what the service actually counts, so nothing overruns
  from an underestimate.
- **Which answer size:** for a gate, the gate's answer size; for the chat and workers, the
  key's answer size; and if nobody chose, a floor for gates and a larger floor for the rest.
- **Fitting the context:** if the slot is larger than what remains of the context it shrinks to what
  remains, and if too little remains the request is not sent.
- **The model's speed bounds the answer:** a gate's answer must be shorter than what its registration's
  model can write within the gate's time limit.

### Too large

- **What counts as too large:** requests not sent because of size — an input with too little room left —
  and requests the model returned for size, a 400 for exceeding context.
- **How a splittable gate handles it:** it splits the request in two, each half becomes one run of that
  gate walking the same road again, a half still too large splits again, a half the model fails for
  another reason ends as that half alone having no answer, each half gets the gate's full time limit, and
  the answers are joined in original order the way the gate defines.
- **Answer room:** a splittable gate sends a request only when it can get its full answer slot, and
  splits as far as needed otherwise.
- **A gate that cannot split** sends with the answer slot that remains, and if it is still too large ends
  as a gate with no answer.

### The three gates that split

1. **The sanitiser gate** cuts text in two around the middle — at a paragraph boundary, else a line
   boundary, else by character count (`docs/cache-fallback.md`, 「The sanitiser gate」).
2. **The translator** splits the part it got in two, at a paragraph boundary else a line boundary, and
   joins the two translated halves in original order; if either half has no answer the whole part stays
   untranslated, since a half-translated section misleads the reader.
3. **The document seek gate** splits the index lines in half and joins the slugs each half picked in
   original order; a half with no answer picked nothing, and since seeking produces candidates one
   half's candidates are still useful. When split picks yield two or more candidates the same question is
   asked once more over only the collected candidates to set the order, and if that does not answer the
   joined order stands.

### What does not split

- **Gates that must read everything at once:** the formatting judgement of one answer, picking pieces
  for one turn.
- **Runs with tools** (chat, workers, a companion's turn): the whole conversation is one
  request, so they cannot split; a turn that meets the limit ends as 「A turn at the input limit」 below
  says.
- **A registration's turn of a provider walk** carries the prompt of the run the walk assembled, not a
  gate's request, so it does not split either.
- **Answer slots** are sized as above. A non-gate key's answer size is its tier's answer size on the
  registry's provider, and is the registration's answer size where the tier names none, never below the
  floor its own work declares: a work that writes a whole document in one tool call declares a floor
  that holds one. A run that thinks writes its thinking out of the same room, so a work may declare a
  larger floor for such a run, given as far as a fraction of the registration's window allows.
- **Only registered models limit input.** Claude Code, Codex and OpenRouter runs have no place where
  agency blocks by request size.

## Chat and workers

### Choosing a registered model

- **The registry's provider carries every line but research:** the chat, the job worker and the
  companions as a Claude Code road, and the gates as requests with no tools (「A request with no
  tools」 above). No search or browse key is offered it and no research run walks to it: a research run
  goes out on a registration only where it was set off by a run on one that keeps its research (「How
  requests go」 below).
- **Jobs:** manual job runs, items and work-order models pick a registered model with
  `registered::<registration>`; automatic jobs go down the provider order as far as the registry's
  provider where the registration the worker key's tier maps to is walked, unless the worker is pinned or
  the worker key is set to one provider, which is then the only one they run on.

### A worker pinned to a provider

- **Every worker shares one work key,** so the worker key's road is every worker's. A worker of its own
  may also be pinned, which then stands for that worker alone in place of the key's road; a worker with
  no pin follows the key.
- **What the pin holds:** a provider that carries the worker's line, and — for a provider that says it
  takes one — one of its models. The registry's provider takes one: a worker pinned to a registration,
  and a job naming `registered::<registration>`, runs on it with the registration's own answer size and
  thinking default, unless the worker key's tier maps to that same registration, whose mapping then
  stands; a pin to a registration the registry does not hold, or one with no address, is refused. A pin
  to a provider alone takes the model, effort and answer size from the worker key's tier mapping on that
  provider (`docs/model-tiers.md`). Every other provider takes the provider alone.
- **What runs on it:**
  - Automatic jobs of the worker run on the pin alone, and recovery tries no other; a job that names its
    own `<provider>::<model>` still asks that first.
  - The words said to the worker on its own screen wait in its log while it is pinned to a provider that
    is on, with a line saying so, since no run on the pin carries words (`docs/agent-control.md`, 「The
    keepalive ping goes into the same program as the turns」). A worker pinned to Claude Code, pinned to
    a provider that is switched off, or not pinned, is spoken to in its Claude session.
  - A worker pinned to a provider other than Claude Code is not kept warm on Claude.
- **Its sampling is its own,** apart from any companion's on the same registration: a worker holds
  sampling settings of its own, kept in the store's settings under its name, and every run of its jobs
  and of the words said to it names them (「How requests go」 below). They are the companions' generation
  settings, by the same names and bounds, less the answer ceiling, which stays the registration's
  (`docs/companions.md`); one left empty is the engine's own default. Only a run on a registration
  carries them, since only the gateway reads them; a support worker runs on its base's; a save checks
  each against its bounds, refuses one out of them, stands the worker's program again and lets the next
  run carry the new values; it is set in the sampling block of the worker's settings dialog, shown while
  its provider is a registration.
- **A pin to a provider that is switched off** reads as no pin, as a key set to one does.
- **A support worker** runs its base's jobs and is on its base's pin; its own row carries none.
- **Where it is set:** the provider field of the worker's settings dialog, which lists every
  registration beside the providers, or `bmo-agency agent --set <worker> --provider
  <auto|provider|registered::<registration>>`, where `auto` takes the pin away.
- **What shows it:** the worker's configured provider and model in the agent list and the `bmo-agency
  agent` listing are its pin's, and the answered road beside them is what last answered.

### How requests go

- **A gate** is one request to the gateway's chat completions path, asked in the agent's host (「A
  request with no tools」 above).
- **Runs with tools go to Claude Code,** the program every line was written for, pointed at the gateway
  under the registration's name (「The gateway」 above). The line is the one a Claude run of it takes — the
  chat's, the worker's, a companion's (「A worker on a registered model is composed as on
  Claude」 below). Its program carries the search and browse lines as Claude Code does, and a research run
  goes out on it only as `research-runs.md` says, since a registration's window decides whether the
  runner's instructions fit at all. The road changes only what follows, each value read from the
  registration the run's model names as the run is started.
  - **The endpoint:** the Anthropic base URL is the gateway, and every model Claude Code asks for a call
    of its own — its Opus, Sonnet and Haiku defaults and the subagent model — is the registration's
    gateway name, with its node where the run is placed on one (「Nodes and sessions」 above), so nothing
    the run does asks a model the gateway does not serve and no request of one session leaves its node.
  - **No login of the Claude road:** the run's records go under a configuration home of the
    registration's own under the data root, which holds no login, and no account's token or directory is
    handed over. It is the data root's since a session resumed across a deployment needs its record
    there, and the agents root is laid anew at every start of the bot. The key is the gateway's, from the
    key helper (「The keys」 above).
  - **The window and the answer:** the context-tokens variable is the registration's context window, and
    the output-tokens variable the key's tier's answer ceiling on the registry's provider, or the
    registration's answer size where the tier names none or the worker is pinned to the registration.
    Claude Code sizes a model it does not know at a large default window and answer, which a backend
    refuses.
  - **The fold point leaves the whole answer:** the auto-compact override is the share of the window the
    answer ceiling leaves (「A turn at the input limit」 below).
  - **The wait:** the API-timeout variables are the registration's request wait. The program's runtime
    otherwise drops a connection silent for minutes and the program asks again, and a node reading a long
    prompt keeps it silent far longer. The gateway is told the same wait with every request, in the
    Anthropic custom headers (「The gateway」 above).
  - **Its retries are Claude Code's own:** after a refused or dropped connection, a 408, 429 or 5xx, and
    a request that ran out its wait. With the wait longer than a node's reading of the prompt, a retry
    follows a node that went down or a card that stood still for the whole wait, and it reads the prompt
    again from its start. The gateway retries nothing. A new session its node refuses before any answer
    is placed on the next node (「Nodes and sessions」 above).
  - **The prefix stays still:** the attribution header is off, since the block Claude Code otherwise puts
    at the head of every request changes each time and defeats the backend's prefix cache. The traffic
    Claude Code sends to its own services beside the model is off.
  - **Thinking is an effort word:** a run with thinking on is sent `medium`, which Claude Code sends as
    `thinking` of type `adaptive`; one with it off is sent `low` with its thinking tokens at zero, so it
    names no thinking at all. The gateway carries either to the backend. Whether a run thinks is its
    agent's own thinking setting where that names one, the tier mapping's word where it names one, and
    the registration's thinking default otherwise.
  - **Sampling is the run's own:** a run that names sampling settings — a companion's turn, from its
    generation settings, and a worker's run, from its own sampling — sends them in the Anthropic custom
    headers beside the wait, as the header the gateway writes into each request; a run that names none
    sends no such header, and the backend samples with what the program sent and its own defaults.
  - **The long tools go with every request:** the program's adapter names the tools whose calls wait long
    (`docs/agent-control.md`), and a run sends them in the Anthropic custom headers as the long-tools
    header, the names joined by commas, which the gateway writes into each request. An engine reads it to
    give up its cards at once once an answer calls one of them, and to keep them while a short tool runs.
    A program whose adapter names none sends no such header.
- **What comes back** is Claude Code's own record — its transcript in a pane, a standing one or one of a
  run apart's own — read as a Claude run's is. A research run has each line of its
  record written into their run log as it is written; a gate's run in the host prints one object at its
  end, whose answer text is all its log is given; a job's turn in the pane has its transcript written
  into the job's run log as the program writes it, so a long turn on a slow engine is never a job
  standing silent. Each is filed under the registry's provider with the registration's name as its
  model, and the money Claude Code works out for a model it does not know is not kept: a registered model
  costs none.
  - **A run that resumed a session is counted by its own figures.** The result's model-by-model usage is
    the session's running total restored as the session is resumed, while its usage beside it is the
    run's own, so a resumed run on a registration is filed under the latter, under the one model it
    names.
  - **What a kept session holds is read off its transcript.** The run's own usage sums every request of a
    turn that called tools, and the stream carries no request's count of its own on this road, so the
    context a session ends a run holding is its last request's, as the transcript under the
    registration's home records it, against the registration's window.
- **A request the backend refuses or cannot answer** ends the run failed with Claude Code's own line
  about it, whether for size (a 400) or anything else, and work that could not be done is never treated
  as success. A refused request is the run's own failure, never the road's (「A turn at the input limit」
  below).
- **A run naming a registration the registry no longer holds** is refused before it starts, with a line
  naming the registration; nothing stands in for it.

### No keepalive

- **The registered models run no keepalive.**
- **Nothing else is warmed either:** keys, gates and workers running on the registry's provider do not
  warm earlier Claude or Codex sessions.
- **Calls only for real work:** a registered model is called only when there is a real work request.

### Claude Code in the pane

- **The registry's provider declares Claude Code's interactive program over the gateway as its
  interactive surface,** and nothing outside the provider names a registration for that. A gate stands no
  program until its stage (`docs/agents.md`, 「The stages」); a companion's turn goes into its own program
  as the chat agent's does.
- **The top of its screen names the registration by its label:** the program's settings carry a model
  picker row for the model it is started on, the node placement included, labelled with the
  registration's label, so Claude Code shows that label where it would show the gateway's name and its
  node.
- **Which program is whose:** a Claude Code program standing in a pane is a registration's where its
  records home is that registration's, and the Claude road's own only where it is pointed at no endpoint
  of another's. So a pin moved between Claude Code and a registration, or from one registration to
  another, is a different program in the pane.
- **Who stands it:** an agent whose line keeps a session — the chat agent, every companion, every worker
  — stands it in its pane for as long as its turns go out on a registered model. A job, the words said to
  the worker on its screen and a conversational agent's turn are each one turn the bot puts into it, and
  a person attached to the console types into it as into any Claude Code program.
- **A conversational agent's turn goes on in its scope's session,** the chat agent's and a companion's
  alike, on the node that session stands on: the agent's one session, or the room's where its scope is
  the room, a turn for another room then starting the program again on that room's session in the same
  pane (`docs/agents.md`, 「The session's scope」). A full session is handed over by the handoff this
  provider declares rather than folded (`docs/companions.md`, 「Its limit」).
- **The words said to a worker go on in the session its program stands on:** where the worker holds a
  session on the registration — on its model, effort and directory, on a node the registration still has
  — and the program standing in its pane stands on it, the words are one more turn of that session, on
  its node, and the program is not started again. Claude Code folds the session as it nears the
  registration's window (「A turn at the input limit」 below).
- **Every other turn the bot puts in opens a session of its own:** a job is stood up on a fresh session,
  placed on a node as a new session is (「Nodes and sessions」 above), so no earlier job rides along into a
  window that may hold tens of thousands of tokens; so are words that find no session to go on in, and
  words after a turn on the held session that gave no answer. A person's lines typed after a turn go on
  in its session, on its node.
- **Words mid-turn are not taken:** words for the agent that arrive while a turn works wait for the next
  turn.
- **A line a person types into the chat agent's program is composed by the program, not the gateway:**
  the chat's submit hook hands it what a round of the owner's chat would carry, on this road as on
  Claude's, which never passes the gateway. The gateway rewrites no prompt.
- **What stays in the host:** a gate with no tool is one request and one answer, asked in the agent's
  host (「How requests go」 above). A gate on a registration stays a host agent; only a gate on Claude Code
  stands programs of its own (`gates.md`). A runner's run on a registration is a run
  apart (`docs/agents.md`, 「Runs apart」).
- **Standing with nothing to do:** a worker pinned to a registered model has the program written into its
  place by the standing pass where the place holds no program, on the job line's run for the registration
  and a fresh session, so its pane shows Claude Code's input box before any job has run
  (`docs/agent-control.md`, 「A worker standing with nothing to do」). The registration is the one the pin
  names, or else the one the worker key's tier maps to on the registry. A place holding another
  provider's program has the registry's written in its stead and the old one ended by the same pass,
  without waiting for a turn on it. A program of the registry on another registration stands until the
  next turn, which opens a session of its own on the pinned one. A pane at work on a turn, or a checkout
  a job holds, is left until that turn has ended.

### A worker on a registered model is composed as on Claude

- **Every worker is handed one line, whatever its road:** a worker's run on a registered model is the job
  worker line (`docs/launch-lines.md`) — the CLI's own system prompt with the worker's rules appended
  (the owner's standing rules, the rule against destroying the system and the worker's own where it has
  them), or the worker's base prompt with the same rules after it where it chooses one
  (`docs/base-prompts.md`), the worker's plugin and its skills, the co-worker definitions, the worker's
  MCP server, the project and local setting sources, so the checkout's `CLAUDE.md` is read in, every tool
  the line shows, and the worker's settings with its hooks, the job guard among them.
- **What the road changes is the model and the endpoint alone:** the gateway's address and key, the
  registration's name, window, answer ceiling and request wait (「How requests go」 above), beside the
  effort word and the records home every run on a registration has.
- **It opens no co-workers:** a registration is a single model on its own engine, so the registry's
  provider lends none. The co-worker gate refuses a native agent call of a run on it and the reservation
  command refuses its reservation, and the worker does that part itself (`docs/model-tiers.md`, 「Common
  co-worker capacity」).
- **Its first request** is the whole line — the tool definitions, the MCP server among them, the system
  prompt, and the opening lines that carry the checkout's `CLAUDE.md` and the skill listing — plus a
  job's prompt where there is one, read by the engine before its first byte. A job writes nothing to its
  log while its request is read, so it can stand quiet for that long (`docs/agent-control.md`, 「A job gone
  quiet」).
- **A window smaller than the line is not refused:** the run is started as on any registration. Claude
  Code sends its first request whatever its window says, and a model that holds it answers. Past the
  first step the conversation is already over the point Claude Code compacts at, so it compacts and goes
  on. Only a request past the model's own window is refused (「A turn at the input limit」 below).

### A turn at the input limit

- **Within a turn every request carries the whole conversation** — the prompt, every tool call and every
  result so far — and the tool definitions, so a turn that reads much grows towards the registration's
  window.
- **A request holds the conversation and the whole answer ceiling.** Every request names the answer
  ceiling as its `max_tokens`, and an engine refuses one whose prompt and ceiling together pass its
  window rather than cut either, answering a 400 that names the prompt's tokens, the ceiling and its
  context.
- **Claude Code folds before the limit:** it takes the window from the context-tokens variable and
  compacts its conversation as it nears it, as it does on the Claude road. A companion's session is handed
  over earlier, in a turn of its own that writes its state for the next session (`docs/companions.md`,
  「Its limit」).
- **Its own fold point keeps room for a bounded answer,** which falls short where the answer ceiling is
  larger, so a request carrying the whole ceiling could still pass the window.
- **So every run on a registration is told to fold at the share the ceiling leaves** (「How requests go」
  above). Claude Code folds at that share of the room it counts, or at its own point where that comes
  first, so a conversation is folded before it holds the window less the whole ceiling, and the room it
  keeps for its summary stands as the margin for its estimate.
- **At the limit the turn ends, failed, on the request's own refusal.** A request the backend refuses for
  size comes back as a 400 naming the size, Claude Code ends the turn on that error, and the turn is
  recorded as failed with it. A job or a word on it is a run that did not answer, never a success.
- **A refused request is the work's failure, never the road's.** A 400, a 413 or a 422 is the backend
  answering, so a job whose turn ended on one ends failed with that line as its reason: its
  registration's road is not closed for an outage, and the walk does not move on to another road, where
  the same conversation would be refused again (`docs/cache-fallback.md`, 「Automatic walks only on
  providers, in order」).
- **The next turn is a new session:** a turn on the held session that ended without an answer lets that
  session go, so the words said next do not go on in it either.

## Settings per tier

### What a tier's mapping holds for the registry

- **Three settings:** the model, the answer size and whether thinking is on.
- **Where:** the registry's column of the tier table on the agent management screen's model tab; a cell's
  dropdown sets them for that tier, offering every registration by its name.
- **Who reads them:** every key assigned to that tier that runs on the registry's provider — a gate, the
  chat, a worker — save a key that names a registration of its own (「A key's own registration」 above),
  which reads its registration in place of the cell.
- **The registry's cell may leave two of them to the registration:** an empty answer size is the
  registration's answer size, and the default thinking choice is the registration's thinking default; the
  on and off choices override it.
- **A gate's model is its tier,** never a provider-specific model: a gate chooses a tier, and the tier's
  mapping on the registry's provider is the registration it runs on there.

### Answer size is the ceiling of the answer slot

- **Choices:** a small set of token sizes, or none.
- **None means the work's default:** a floor for a gate, and for the chat and workers on a
  registered model the registration's answer size.
- **A gate's own floor:** the translator never runs below a size that holds a part, whatever its tier
  says; the other gates'
  answers are one verdict line or a few.
- **The rules of 「Input size」 above still hold:** a ceiling larger than what remains of the context
  shrinks to what remains; with too little remaining nothing is sent; a splittable gate sends only when
  it can get its full ceiling.
- **So:** raising the ceiling splits long inputs earlier; lowering it shortens answers.

### Thinking is on or off

- **A request with no tools says it either way:** `enable_thinking` in `chat_template_kwargs`, on or off,
  as the run's thinking is.
- **The response** carries the thinking apart from the answer in `content`; agency uses only `content` as
  the answer, and keeps the thinking in neither logs nor records.
- **Thinking eats the answer's budget:** it is spent within the same `max_tokens` as the answer, at the
  same speed, so a gate with thinking on may not reach its answer within its time limit and then ends as
  a gate with no answer, and a tier with thinking on should have its answer size raised too.
- **A run with tools** is Claude Code's, and thinking reaches it as the effort word 「How requests go」
  above names.
- **The record:** an answer to a request with no tools, with thinking on, is recorded with effort
  `thinking`; one with it off has no effort.

## Per-gate time limit

- **Each gate has its own time limit, and the table is the only place it is set.** In the store's gate
  options table, one gate is one row, and every row holds a time limit. No code that calls a gate names a
  wait of its own, and no code carries a default a gate would fall back to.
- **The seed is one value for every gate.** The seed refresh, at every settling of the file and at every
  start of the bot on a server database, gives a gate with no row, or a row with no time limit, the seed,
  and leaves every time limit already written as it stands. So a gate new to the code has its row from
  the first start that knows it.
- **A missing time limit fails aloud and breaks no turn.** A gate asked to run where its row holds none —
  a process that installed no store, a store nobody settled, or a row taken away under a bot still running
  the code that names the gate — is never run on a wait nobody chose: the journal names the gate and why,
  and the run ends as a gate with no answer, so its caller goes its own way. Reading the gate's route
  never asks for the wait, so none of that fails on it either.
- **Where to choose:** the gate's row in the gate tab of the agent management screen, which shows every
  gate's time limit and offers only real values; a row cannot be emptied.
- **The table's thinking and answer-size columns are not read:** those come from the gate's tier.
- **The same row holds the switch of a gate that may be switched off:** only a gate whose profile
  declares it has one (`gates.md`), empty means on, and a gate switched off is not asked — its caller
  goes its own way without it. The gate tab shows the switch for such a gate and a dash for every other,
  and the bot refuses a switch for a gate that has none.
- **The same row holds the gate's classifier mode and threshold,** empty for the profile default
  (`laya.md`). The time limit is also how long the gate waits for the classifier, its wait in the
  classifier's line included; a gate whose provider is the classifier waits that alone, a shadow mode
  does not wait for it at all, and a filtering mode still gives the road its whole time limit after the
  classifier has passed the question on.

### The time limit is all a gate waits for an answer

- **It decides the verdict.** With no road after it, this value is where the gate gets or loses its
  verdict: short, the gate does not hang on a busy service and moves on without a verdict; long, the room
  is held until the busy spell passes.
- **Every run of the gate waits the same.** A turn's run, a keepalive ping and a test from the console
  all read the one row, so no caller waits longer or shorter than the gate's own value.
- **The waits are set against what a failure costs, not against the answer:** a gate a turn waits on, with
  a local stand-in for its verdict, waits less; a gate nothing stands in for and nobody in a room waits on
  is given the whole limit.
- **What one slot carries:** the bulk gates fill a single engine's slot for minutes at a time, and the
  gates behind them reach their limits, so they stand on Claude instead, with any gate whose readings a
  small model gets wrong where Claude gets them right.

### Who does not read the gate table

- **Runs with tools** (Claude Code on a registered model) and non-gate keys: their time is the time their
  walk gives them, and each request of theirs waits the registration's request wait.
