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
- **Only a program that carries the line is a road of it.** A research line reaches the web and the brain
  through the run's MCP servers alone (`policy/run-lines.toml`). Claude Code carries the search and browse
  lines. No provider of this build carries a research line but Claude Code; a key whose tier maps to another
  provider walks on to the first provider that carries it. Where nothing can carry the run — every provider
  of it off, its key missing, or every login of it closed — the run is refused, and the refusal names each
  provider's reason and says that no other provider's program carries the line, with what it needs.
- **On Claude Code a run goes out in a pane of its own** as a run apart (`docs/agents.md`, 「Runs apart」):
  the runner's line as `docs/launch-lines.md` 「Search runner」 and 「Browse runner」 compose it, its
  standing instructions as the system prompt and the request as its words.
- **The dsh places are kept for a rollback,** and no run of this build enters them: the first answer stood in
  a place named after the ledger's run, `search-<runId>`, and the rounds in a second, `search-<runId>-rounds`,
  so the rounds read a session of their own.
  - **Its program** is bmo-tui on a dsh session, its overlay writing the search's standing instructions
    as the persona suffix after dsh's own prompt, every dsh tool row off, and the run's MCP servers — the
    tool host's `bmo` and `web` namespaces — on a session token issued for that place, whose record
    carries the run's research settings as a Claude Code run's does (「What a run is held to」); the
    rounds' place is issued its token on the rounds' narrower settings. A place's token stands until the
    moment the run's web tools close and half an hour past it, or half an hour from its issue where that
    moment has passed: the bound a token keeps where nothing takes it back.
  - **Its words** are put into the place's inbox, the request first and each round's question after, and
    its answer is read back out of the place's record.
  - **What it spent** is the record's own figures for each turn, the input, output and cache-read tokens
    DeepSeek's API answered, summed over the run's turns as the ledger keeps them.
  - **Each place keeps a record naming the run** while the run stands in it, `research-place.json`, so it
    stands under no agent's row and no sweep of leftovers takes it down.
  - **Both places are ended with the run,** however it ends — answered, cut or stopped — their programs
    released, the session tokens they carried taken back from the tool host, their workspaces closed and
    their folders taken away, records and all. Until then they outlive agency's restart, and the run with them
    (「One run, step by step」, step 7).

## What a run is held to: the research settings

- **One set of settings, the owner's,** kept by agency in its store and changed through `PUT
  /research-settings`; where nothing was changed, the policy's defaults stand (`policy/research.toml`,
  `[settings]`):
  - **the branch role:** the co-worker role whose tier a search's pooled branches go out on;
  - **each key's branches:** how many pooled branches a run of it may open — none at the basic depth to
    start with — and **its clock:** how many minutes its web tools stay open from the run's start;
  - **each key's budget:** how many pages its first answer may fetch and how many each branch it opens may
    — three and no branch at the basic depth, eight and two branches of four at the thorough, twenty and
    five branches of six at the exhaustive, none named for a browse. The request states it as a ceiling:
    once it is spent the run fetches nothing more and writes with what it has, what stayed open under its
    unconfirmed list. The record carries it as `pages` and `branchPages` for the web tools that hold a run
    to it;
  - **the fetch strategy:** `direct-then-browser`, `browser-first` or `direct-only`;
  - **the gaps:** the milliseconds between two direct requests to one host and between two requests to one
    search engine, each null where the web tools' own default stands.
- **The settings travel on the run's tool session record.** As a run is launched, agency writes them into
  the run's variables, and the record it issues for the run carries them under `labels.research` —
  `closesAt`, `branches`, `role`, `source`, `strategy`, `hostIntervalMs`, `engineIntervalMs`, `pages` and
  `branchPages` — the shape the web tools read (bmo-explorer's `docs/api.md`, 「What a placed tool reads of
  its record」). What the console shows is what the next run carries.
- **A branch's record says it is one.** A research branch is launched with its parent's variables and
  `BMO_RESEARCH_BRANCH_OF`, the ledger id of the run it is a branch of. Its record carries the parent's
  settings with `branchOf`, that id; its `pages` is the parent's `branchPages`, the budget each branch of
  the parent has, which the web tools hold its fetches to; and it opens no branch of its own, `branches`
  0, no `branchPages`, and the branch tool not shown.
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
5. **The rounds:** after the first answer the key's research rounds are asked of a session of their own,
   opened fresh on the same program and road, one turn each — at the basic depth only the final question, at the deeper ones every question in turn
   (`docs/launch-lines.md`, 「A research run's rounds」, `docs/api.md`, 「research drafts」); the last answer
   is the run's result.
6. **The end:** the run is closed in the ledger with what it spent, and its log says how it ended.
   Every result leaves its document, a brief one included, written for its reader and filed in
   bmo-artifact (its `POST /documents`, then a share link by `POST /shares` that ends a day after it is
   made — the document stays in the archive, and a person who wants it again takes a fresh link from the
   console); bmo-artifact draws it
   (its `docs/archive.md`, 「How a document is drawn」, 「Attachments」, 「References」). The result is the
   searcher's structure lines (`prompts/agents/search/task.md`, `prompts/rules/search-kinds.md`); the
   document is made of them and carries none of them:
   - **its head:** the title is the `TITLE:` line and the description the `TOPIC:` line, both in the
     question's language as the searcher writes them, the description handed as the document's own so a
     card or a preview reads it rather than the body; the kind is `learning` where `KIND: learning` says
     so and `search` otherwise; the labels are the run, its key, its question, its `KEYWORDS:` and
     its `RULE:` where it has one. The `ANSWERED:` line is the run's own and stands nowhere.
   - **its body:** each block a section under a heading in the question's language, the words of
     `prompts/agents/search/document.toml`: the verdict under 「the answer」, the summary as its
     numbered list under 「the summary」 — under 「the answer」 itself on a brief result, which has no
     verdict — each `SECTION:` under its own name — the product line's news under 「the news」, since its rulebook
     names that section in English — a section of grounds under 「the grounds」, the
     unconfirmed list under 「what was not confirmed」, and the sources last under 「the sources」, one
     numbered line each, the page as a link titled by what it supports, the snippet-only mark kept.
     Tables, lists and `mermaid` blocks stand as written. A set's entries stand as sections of their
     own under their names, with the address, the map and road-view links, the menu as its own list,
     the four product lines and the reviews each as a line of the entry in the same words file.
   - **its pictures:** every `IMAGE:` line's picture is fetched through the guarded direct fetch, at
     most twelve, within a bound for the whole keeping, and attached to the document under a name of
     its own (`POST /documents/{documentId}/attachments`), the body drawing it by that name with its
     caption; a picture that could not be fetched is drawn from its address.
   - **its references:** the sources are links of the body, which bmo-artifact keeps as the document's
     references with their previews, the list of pages read beside the document.
   - **the narration:** a deep dive's `LISTEN:` script is lifted out and never filed in the body; it is
     handed to the voice path as the document's head sound once that path takes one.
   The room is queued an instruction through the receiver's `POST /instructions`, as a job's report is,
   for the owner's private room where the ask named none: the answer in brief — the result's verdict,
   else its summary — and the link, with the shape the chat agent answers in: the answer in one or two
   short lines in the owner's language and its own voice, then exactly that one link, and no grounds,
   sources or document text in the chat, saying what was found and never how. Where the deployment
   names no artifact service or the filing fails, the instruction carries the result whole instead, and
   the run log says why it was not filed. A run that ended with no answer is told as such.
7. **A run that is cut** — its pane gone under it, the program in its place gone while a turn of it
   works, or agency's own restart ending a run whose place is gone or that went out in a pane — is closed
   in the ledger with the outcome `cut`, and the room is handed what the run had written where its
   session's record holds anything, as unfinished, and otherwise one line saying the work was cut off
   (the receiver's `docs/voiced-notices.md`, a stopped run).
   - **A run under way is kept** in agency's settings, `research_in_flight:<run>`, with its key, question,
     room, model and the record its session writes. A run in places keeps beside them everything it went
     out with — the ask, the road, the request, the moment its web tools close, its bound and its
     composed line — and where it stands, written before each of its turns goes in: the place the turn
     goes into, how many of the run's turns went into that place before it, the round it asks, the moment
     the rounds began, and the answers so far with what they spent. The place is marked as the run's
     before it is kept as the one the turn goes into.
   - **A run in places outlives agency's restart.** Its places are programs of their own, each with its
     inbox and record, which a restart leaves working; a server that starts takes up every kept run
     whose place still stands as the run's. The turn under way is read out of that place's record — the
     turn that went in under the run's name after as many as the kept count says — and waited on, under
     the place's lock, until its end is there; a turn the restart caught before it went in is put in
     then. The run then goes on as if nothing had happened: the rest of its rounds, its close in the
     ledger with what every turn spent, its filing and the room's instruction, and both places ended.
     Its log goes on under the same run from the moment it is taken up, which it says.
   - **A server that starts** closes as cut every other run the process before it kept, and every
     research run the ledger still holds open from before its start but the ones it took up, before the
     routes stand; a cut run's places that still stand are ended with it.

## The chat agent's ask

- **agency's plugin serves `run_research`,** which calls `POST /research-runs`: the chat agent names the
  depth and the question, and is told the run's id, its road and when its web tools close, or why the run
  was refused. The result comes back to the room as an instruction once the run ends.
