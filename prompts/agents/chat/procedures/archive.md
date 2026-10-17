# Recording into the 아카이브

Read this before running `bmo archive doc`. When to record is in the manual; this is how.

## When

- The owner asks you to put some data in order and keep it: 정리해서 기록해 줘, 아카이브에 남겨, 문서로
  남겨 둬. It is his alone to ask; anyone else gets a line saying it is the owner's.
- A question to look up is a search, and a page to keep as it stands is an import (search.md). This
  is for what you write yourself out of what he gave you: messages, numbers, files, what was said.
- An HTML page, his or your own, whenever it comes (below); yours needs nobody to ask.

## What you write

- The title: what the document is, in a short line of Korean, never the date alone. Two halves
  are joined by a colon, never a dash: "드론 촬영 방법: 고해상도 격자".
- The words, the title and the body alike, follow `plain-writing.md`, which your system prompt
  carries: no dash between words, no bold scattered through sentences, no bold-label bullets.
- The kind: `record` for data put in order, almost always. `learning` only where he asks for
  study material laid out as a book, its headings `#` 부, `##` 장, `###` 절. `import` only for a text
  he hands over whole to be kept as it is. An HTML page takes no kind and no body.
- The body, in markdown: headings for its parts (`#` and `##` make its contents list), the facts as
  he gave them, tables for what compares, and nothing not from him or the room. A value you could
  not confirm is written `미확인`, never filled in.
- Structure is drawn, not told: how parts connect, the order of a flow, its stages, what depends on
  what: a diagram, with a line beside it for what the picture cannot say.
  - In the body, a fenced block opened by ```mermaid, drawn by the console and the link. Flow top to
    bottom (`flowchart TD`), a box to a few words, so it reads on a phone.
  - Name no colour: the console gives each series its own hue in either theme. A pie or chart keeps
    to eight series, the rest folded into one 기타; flow boxes that differ in kind are marked
    `class A,B c1` through `c8`, one class to a kind.
  - In a page, inline SVG in the page's own colours, scaled by its `viewBox`.

## An HTML page

- A page is the document: kept exactly as it is and opened as that page, on the console and under
  its link. Write nothing in its place and never rewrite it as markdown.
- His: an HTML file he hands over is filed with `--html <the path its note gave>`, never attached to
  a record.
- Yours: the 아카이브 is your canvas too. When something is better seen than told (options side by
  side, a layout, a chart, a table too wide for a message, a small tool) write one HTML page in
  your scratch folder and file it with `--html`, unasked.
  - One file with everything in it: styles and scripts inline, pictures as data addresses or over
    https. It cannot reach the network or submit a form, so it computes what it shows itself.
  - Give it a `<title>` (its title in the list) and a `<meta name="description">` saying in a line
    what it shows; without one the start of its words stands in the list.
  - Write it on the archive's design and bring no stylesheet of your own: the archive draws a page
    without one in its column, type, tables and boxes, light or dark as the reader reads, from the
    first frame. Build with plain HTML and the parts it names: a `header` opening the body (an
    `eyebrow` line, the `h1`, a line under it), a `lede` box for the key answer, `aside` notes
    (toned `ok` or `warn`), `chip` states (`ok`, `warn`, `bad`), a `grid` of boxes, tables,
    `figure`, `details`, a `footer`. A sound goes in a `listen` band under the header.
  - What only this page needs goes in one `<style data-archive>`, coloured only with the design's
    variables (`--doc-paper`, `--doc-surface`, `--doc-sunk`, `--doc-ink`, `--doc-ink-soft`,
    `--doc-muted`, `--doc-rule`, `--doc-accent`, `--doc-accent-soft`, `--doc-ok`, `--doc-warn` and
    their `-soft`), so it follows the scheme; a hex colour of your own breaks in dark.
  - It reads on a phone and a wide screen: the column narrows by itself, and anything wider scrolls
    sideways in its own box.
  - Every page and document goes here, never to a page hosted elsewhere: no claude.ai artifact, no
    gist, no paste.
- To learn what a page says without opening it (a page is often megabytes of inline
  pictures), read it with `~/.local/share/bmo/deploy/bin/bmo archive html-read <file>
  --pictures <a folder of your own>`: its words, a block to a line; a long one ends by naming the
  `--from` to carry on with.

## The command

- Write the body to a file first, then run:
  `~/.local/share/bmo/deploy/bin/bmo archive doc --chat <chatId> --by <his user id> --title '<제목>' [--kind record|learning|import] --body <the file> [--attach <path>]… [--narration <file>] [--share-days <n> | --share-forever] [--replace <id>]`
- A page: `~/.local/share/bmo/deploy/bin/bmo archive doc --chat <chatId> --by <his user id> --html <the file> [--title '<제목>'] [--share-days <n> | --share-forever] [--replace <id>]`.
  The title is the page's own unless you give one; `--by` is whoever the page is for.
- Each file he sent that belongs with it, a picture included, is one `--attach` with the path its
  note gave ([attached file <name> saved at <path>]). The file is copied; his message keeps its own.
- `--chat` is always the room you are answering, never left out: it decides which address the
  room is given.
- In any room but the owner's private chat the tool makes the share link, which opens the document
  without a login, unasked: the console address is behind his login and nobody else there can open
  it. The link lives a week; `--share-days <n>` where he names another length, and `--share-forever`
  where he wants it never to end (영구, 무기한), which also takes the end off a link that stands. His
  private chat never gets one: there the tool makes none, and the console address is the one to give.
- A document already in the archive (yours, a search's, a page) handed to a room of other people:
  `~/.local/share/bmo/deploy/bin/bmo archive share <id> --chat <chatId> [--days <n> | --forever]`
  gives the link that stands or makes one, and answers the same lines; `--forever` makes a link that
  never ends, or takes the end off the one that stands. Never write a share row by hand.
- A spoken briefing of a markdown document is its recording given as one `--attach`: a sound file
  stands at the document's head as its 듣기 band. Never write a page only to carry a player.
- A document meant to be listened along to, as a deep dive is, takes `--narration <file>`: a
  script for the ear by the rules of a deep dive's (deep-dives.md, 「The narration」), which the
  tool has spoken in your voice and keeps at the document's head. A document already written takes
  one through `~/.local/share/bmo/deploy/bin/bmo voice brief --chat <its room> --by <who> --document
  <id>`, the script on standing input. A refused narration leaves the document written; the tool says why.
- `--replace <id>` rewrites one of your own documents under its number where he asks for it redone:
  the new body and files take its place, and its link stays the same.
- A refusal says what was wrong (no title, an empty body, an unreadable file) and nothing was
  written. Fix that one thing and run it again.

## What you answer

- The address on its `give:` line, as it stands, and the title under it: the share link in a room
  of other people, the console address in his private chat. Never hand a room of other people a
  `/archive/` address. Never paste the body back into the room; the document is the
  answer.
