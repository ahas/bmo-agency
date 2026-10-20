# Recording into the 아카이브

Read this before filing a document with `artifact_file`. When to record is in the manual; this is how.

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
  study material laid out as a book, its headings `#` 부, `##` 장, `###` 절. An HTML page is a
  `page`, with its HTML and no body. An `import` is a web page brought in whole, which
  `artifact_import` makes from its address; it is never filed by hand.
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
- His: an HTML file he hands over is filed as a `page` with its `html`, never attached to a record.
- Yours: the 아카이브 is your canvas too. When something is better seen than told (options side by
  side, a layout, a chart, a table too wide for a message, a small tool) write one HTML page in
  your scratch folder and file it as a `page`, unasked.
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
  pictures), read it with `~/.local/share/bmo/deploy/bin/bmo html-read <file>
  --pictures <a folder of your own>`: its words, a block to a line; a long one ends by naming the
  `--from` to carry on with.

## The tools

- Write the body in markdown, then file it with `artifact_file`, which takes `body`, the document:
  `kind` `record` or `learning`, the Korean `title`, `actorName` the name you file under, `labels`
  `{}`, `scope` the room's chat id, and the markdown in the document's `body`. It answers the
  document, whose `id` is the address below.
- A page: `artifact_file` with `kind` `page`, the same title, actor, labels and scope, and the HTML
  in the document's `html`, with no body. Leave `title` empty where the page's own `<title>` is the
  one to use.
- Each file he sent that belongs with it, a picture included, is one `artifact_attach` with the
  document's `documentId`, its `name`, the `mediaType` it is read as, and `content`, the file in
  base64. His message keeps its own. File the body first where it draws a kept picture by name
  (`![a caption](name.png)`).
- `scope` is always the room you are answering, its chat id, so the room's documents stand together.
- In any room but the owner's private chat, call `artifact_share` with the `documentId`: the link
  opens the document without a login, and the console address is behind his login and nobody else
  there can open it. Give `expiresAt` a week from now in epoch milliseconds; where he names another
  length, that moment; where he wants it never to end (영구, 무기한), give no `expiresAt`. His
  private chat gets no link: the console address is the one to give.
- A document already in the archive (yours, a search's, a page) handed to a room of other people:
  `artifact_share` with its `documentId` gives the link that stands or makes one. A link that stands
  keeps the end it was made with; end one with `artifact_unshare` and the `token` `artifact_shares`
  gives, where it must be replaced. Never write a share row by hand.
- A spoken briefing of a markdown document is its recording attached with `role` `head`: a sound
  file stands at the document's head as its 듣기 band. Never write a page only to carry a player.
- A document meant to be listened along to, as a deep dive is, takes a narration: a script for the
  ear by the rules of a deep dive's (deep-dives.md, 「The narration」), spoken in your voice by the
  voice engine and attached with `artifact_attach` at `role` `head`. A refused narration leaves the
  document written; the tool says why.
- `artifact_rewrite` rewrites one of your own documents under its number where he asks for it
  redone: give `documentId`, the `labels` (they replace the labels whole), and the `title`, `body`
  or `html` that changed. Its attachments and share links stay, and a file of a name it already
  keeps is not attached again.
- A refusal comes back as a tool error saying what was wrong (no title, an empty body, an
  unreadable file, a wrong number) and nothing was written. Fix that one thing and call it again.

## What you answer

- The document's address and the title under it: the share link `artifact_share` answered in a room
  of other people, `/archive/<id>` in his private chat. Never hand a room of other people a
  `/archive/` address. Never paste the body back into the room; the document is the answer.
