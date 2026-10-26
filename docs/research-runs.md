# Research runs

How agency runs a research: a search at one of three depths, or a browse, asked of it by the chat agent
through agency's plugin, by the console, or by any caller of its route; on which road it goes out; what it
is held to; and how its result comes back.

## The work keys

The translator is a runner too, asked through its own route rather than this one, and runs apart in the
same way (`docs/launch-lines.md`, 「Translate runner」).

- **Each depth and the browse runner is a work key with a profile of its own:** `search:basic`,
  `search:thorough` and `search:exhaustive` on the search line, and `browse` on the browse line
  (`policy/work-keys.toml`). Like every other key each has a row of the fallback table (`GET`/`PATCH
  /fallback`), a tier assignment that chooses its model and effort (`docs/model-tiers.md`, 「Research
  runs」) and a road read by `GET /work-keys/{workKey}`.
- **The runner's own key is `search`,** a key with a tier assignment and no profile: what the keepalive
  warms the search line's prefix on. A run never goes out on it.

## The road

- **A research run's road is the road resolution's answer for its key** (`docs/cache-fallback.md`, 「One
  road resolution」), like every other key's: the key's row heads the walk, and the first provider that can
  answer now carries the run, on its first open login.
- **Only a program that carries the line is a road of it.** The search and browse lines need the tool gate
  and tools reached through MCP alone (`policy/run-lines.toml`), which only Claude Code's program offers,
  so every other provider is no road of a research key. Where nothing can carry the run — Claude Code off,
  or every login of it closed — the run is refused, and the refusal names each provider's reason and says
  that no other provider's program carries the line, with what it needs.
- **A run goes out in a pane of its own** as a run apart (`docs/agents.md`, 「Runs apart」): the runner's
  line as `docs/launch-lines.md` 「Search runner」 and 「Browse runner」 compose it, its standing
  instructions as the system prompt and the request as its words.

## What a run is held to: the research settings

- **One set of settings, the owner's,** kept by agency in its store and changed through `PUT
  /research-settings`; where nothing was changed, the policy's defaults stand (`policy/research.toml`,
  `[settings]`):
  - **the branch role:** the co-worker role whose tier a search's pooled branches go out on;
  - **each key's branches:** how many pooled branches a run of it may open — none at the basic depth to
    start with — and **its clock:** how many minutes its web tools stay open from the run's start;
  - **the fetch strategy:** `direct-then-browser`, `browser-first` or `direct-only`;
  - **the gaps:** the milliseconds between two direct requests to one host and between two requests to one
    search engine, each null where the web tools' own default stands.
- **The settings travel on the run's tool session record.** As a run is launched, agency writes them into
  the run's variables, and the record it issues for the run carries them under `labels.research` —
  `closesAt`, `branches`, `role`, `source`, `strategy`, `hostIntervalMs`, `engineIntervalMs` — the shape
  the web tools read (bmo-explorer's `docs/api.md`, 「What a placed tool reads of its record」). What the
  console shows is what the next run carries.
- **The search engines' state** is the web tools' own, read through the tool host's call of
  `explorer/search_engines`, and answered beside the settings; where the tool host cannot be asked, the
  answer says why instead.

## One run, step by step

1. **The ask:** `POST /research-runs` with the key, the question and, where they are known, who asked,
   the options and the room the result is for. A key that is not a research key is refused 400.
2. **The road:** the road resolution answers the provider, the model and the login; a closed road is
   refused 503 `no-login-open` with the resolution's sentence, and nothing is launched.
3. **The answer at once:** 202 with the run's ledger id, its key, its road and the moment its web tools
   close. The run goes on in the background.
4. **The run:** the request is the line's request template filled with the question, who asked, when,
   the options, the depth and the branches it may open (a browse: the task and its time); the program
   answers in its pane, on the logins the key's walk offers one after another where a login fails under it
   (`docs/cache-fallback.md`, 「Automatic walks only on providers, in order」).
5. **The rounds:** after the first answer the key's research rounds are asked of the same session, one
   turn each — at the basic depth only the final question, at the deeper ones every question in turn
   (`docs/launch-lines.md`, 「A research run's rounds」, `docs/api.md`, 「research drafts」); the last answer
   is the run's result.
6. **The end:** the run is closed in the ledger with what it spent, and its log says how it ended.
   Every result leaves its document, a brief one included: it is filed whole in bmo-artifact (its
   `POST /documents`, the kind `learning` for a result whose head says `KIND: learning` and `record`
   otherwise, titled by its `TITLE:` line, labelled with the run, its key and its question), and a share
   link is made for it (`POST /shares`). The room is queued an instruction through the receiver's `POST
   /instructions`, as a job's report is, for the owner's private room where the ask named none: the
   answer in brief — the result's verdict, else its summary — and the link, with the shape the chat
   agent answers in: the answer in one or two short lines in the owner's language and its own voice,
   then exactly that one link, and no grounds, sources or document text in the chat, saying what was
   found and never how. Where the deployment names no artifact service or the filing fails, the
   instruction carries the result whole instead, and the run log says why it was not filed. A run that
   ended with no answer is told as such.
7. **A run that is cut** — its pane gone under it, or agency's own restart ending it — is closed in the
   ledger with the outcome `cut`, and the room is handed what the run had written where its session's
   record holds anything, as unfinished, and otherwise one line saying the work was cut off (the
   receiver's `docs/voiced-notices.md`, a stopped run). A run under way is kept in agency's settings,
   `research_in_flight:<run>`, with its key, question, room, model and the record its session writes;
   a server that starts closes every run the process before it left there, and every research run the
   ledger still holds open from before its start, the same way, before the routes stand.

## The chat agent's ask

- **agency's plugin serves `run_research`,** which calls `POST /research-runs`: the chat agent names the
  depth and the question, and is told the run's id, its road and when its web tools close, or why the run
  was refused. The result comes back to the room as an instruction once the run ends.
