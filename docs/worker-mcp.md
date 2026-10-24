# The worker's own tools

## What they are for

- A job asks the graph and the queue more than anything else it does: over two weeks of this
  repository's own traffic, `scripts/graph.ts` was the worker's most-called script.
- Every one of those calls is a shell command a session had to compose. Composing it costs the
  round trips that come with composing anything: a verb spelled wrong, an argument in the wrong
  place, a `--help` read to find out what the arguments were.
- Served as MCP tools instead, the names and their arguments stand in the session's own tool list.
  There is nothing to look up and nothing to spell, so those round trips do not happen.
- **They answer in English.** A worker reads English (`docs/worker-language.md`), so the two queue
  tools give their labels in English and every text a person or the bot wrote as its translation out
  of the table, never asking the translator themselves.
- **They answer as the scripts do.** The tools are Rust, placed in `tools/graph/` and `tools/todo/`
  and served by the tool host (`mcp-host.md`, 「Placed tools」); the scripts are TypeScript until their own
  port. A parity test runs each tool and the script's rendering on the same fixture and asserts the
  same answer, so a tool and the script cannot answer the same question differently. The script
  run by a job answers in the same English; run by a person or by the chat agent, in the reader's language.

## What they serve

| Tool | What it answers |
|---|---|
| `graph_search` | Nodes matching a word, as id and title |
| `graph_get` | One node: its files, symbols, aliases, and the names either side of it |
| `graph_list` | The areas and their counts, or one area's nodes |
| `graph_impact` | What else a change to one node would reach, with the files to open; every job asks it on the node it changes before its first edit |
| `todo_list` | The queue, or what is pending, held or merged |
| `todo_show` | One item as a worker reads it: its state, what jobs wrote onto it, and the bot's brief with the changes handed, never the conversation or the words people added (`docs/worker-language.md`, 「The queue tools, as a worker reads them」) |

- **Six tools, and every one of them read-only.** They are marked read-only in the protocol's own
  words, which is what lets Claude Code send several of them at once; a tool that stopped saying so
  would put a turn's questions back in single file.
- **No verb that writes.** Nothing here creates, updates, deletes, aliases or closes anything. A
  change to the graph or the queue is typed as a command, where it is read before it runs.
- **Starting a job and committing a deployment are not among them and are not to be put there.** They
  are the owner's to run. A tool standing in a session's list is a thing that can be called by name
  alone, and those two are not to be callable that way.
- **Stopping a run is not among them either, for the same reason and one more.** A stop is a
  write — it ends runs that are not the asking session's — and the reading it does of what is
  running is already agency's running report's (`docs/agent-control.md`, 「Reading everything running at
  once」). It is typed as a command, where it is read before it
  runs.
- The scripts keep every verb they had. They are what a person types and what a chat session calls,
  and the reading verbs answer there exactly as they answer here.

## Why they are the worker's alone

- **They are shown under the `worker` namespace to a session holding `graph.read` and `queue.read`,**
  and the worker role is the only role giving those two bundles (`docs/agent-roles.md`), and no line
  declares them (`mcp-host.md`, 「Namespaces and capabilities」). So a tool added with those
  capabilities reaches job workers and their co-workers and nothing else: by its own name where it is
  resident, with `bmo use` otherwise.
- **They are a namespace of their own rather than more web tools.** The web tools are shared with
  the search and browse runners, whose business is the open web; the graph of this repository and
  the owner's queue are no part of that, and a research run has no reason to be shown them.
- The distinction is not politeness. Every tool's name, description and argument schema stands in
  the fixed prompt of every run that is shown it, so a tool shown to a line that will never call it
  is a cost that line pays on every turn for nothing.

## What they cost, and why the descriptions are short

- The tools' schemas join the worker's fixed prompt, which already carries tens of thousands of
  characters of documents before a job is told anything.
- So each description says what the tool answers and stops. What a verb means, when to reach for
  it and the order to ask in are in `graph.md` and the worker's own documents, which are read
  once rather than restated in six schemas.

## When the tool host is not there

- **A worker whose tools would be missing is not launched.** Claude Code given a configuration naming
  a server that does not answer comes up anyway, with that server's tools absent and nothing on its
  error stream: the run ends with `result.subtype` reading `success`, and the one trace is the
  server's entry in the session's opening `init` event, which reads `{"name":...,"status":"failed"}`
  (measured). A job cannot tell a tool that went missing from one that never existed.
- **So the absence is caught before the program starts.** The launcher asks the tool host for the
  session's token first, and where the host does not answer the launch is refused with the host
  named (`mcp-host.md`, 「When the host is not there」).

## When the configuration file is not there

- **This fails loudly.** A `--mcp-config` naming a file that is missing costs the worker its whole
  run. The CLI names the file on its error stream and exits 1 in under a second, before it has read
  a word of the prompt.
- **Nothing of that reaches the job's record.** A job is started in a pane and read back out of its
  own record afterwards, and a run that never began wrote none — so what stands there is that the
  job answered nothing, with no reason anywhere near it.
- **The file is the one thing this line hands the CLI that no checkout holds.** It is written where
  the line reads it, under the agents tree, by the same code that names it. A commit that gives it
  another name therefore stands in the checkout with the file still under its old name until the
  next deployment, and every job launched in that window points at nothing.
- **So the launch writes it.** A run that finds no configuration under the name its own code uses
  composes one and writes it there; one already standing is left exactly as it is, since two trees
  writing one file would move the worker prefix back and forth between them. The same holds for
  every other line's configuration.
- **And a path that is still not there refuses the command.** The command is not built at all, and
  what the run ends with is a sentence naming the line, the file and the deployment that supplies
  it (`deploy.md` 「When the deploy tree is behind the checkout」).

## Where the parts are

- The tools are served by the tool host, under the `worker` namespace of the worker line's one
  configuration, `worker.mcp.json`, beside the `web` namespace.
- The graph a call answers out of is the one in the checkout the job works in, which the launcher
  records in the session record's `labels` when it issues the token: a support worker's worktree is
  read as itself rather than as the base checkout the environment names
  (`docs/worker-slots.md` 「A job on a support worker」). It is read on every call, so a job's own
  edits to it are in the next answer.
- The queue a call answers out of is the archive the record names, read on every call.
- The answers stand under the standing ceiling on what a verb of ours returns
  (`job-tools.md` 「What a verb answers with」), cut by the same units the scripts cut by.
- Which namespaces the worker's line is handed, and what else stands in its fixed prompt, are in
  `docs/launch-lines.md`.
