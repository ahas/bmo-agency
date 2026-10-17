# Messages, media and small room actions

Read this before sending anything below. When to use each is in the manual; this document is how.

- Your own sending tools (`say`, `photo`, `remind`, `react`, `pin`, `sticker`) act in this turn's
  room unless `chat` names another, and by themselves send on the road your room context names
  under THE ROAD THIS TURN ANSWERS ON. Give one `via` only where the room block hands a verb to the
  room's own road instead.
- Every script below, and every search or browse launched as a script, takes `--via <road>`, that
  same road: pass it on every one. In the owner's private chat two roads meet, and a send without it
  is kept in the chat's own record, which his screen shows, even where he spoke on the platform; a road
  that does not reach the room you name is ignored, and the room's own answers.

## An extra message

- Tool: `say` with text '<text>', and chat <chatId> for a room that is not this turn's.
- It runs in a unit of its own and returns the moment that unit is up, before the sending; a
  message of several lines spends minutes in the output gates.
  - Never send the same words twice because the tool answered quickly.
  - Never say the words arrived on the strength of that line alone.
  - A launch it could not make says so and ends in fault: nothing is running or coming.
- A send to a room not yours names your own chat as sender, and you are then told here, quietly,
  whether the words arrived: the only way you can know. You read your own words in your own room in
  your next round.
- The room you wrote to stays open for a few minutes, as after any answer: whatever anyone says
  back reaches that room's session without naming the chat agent.

## Asking another room and getting the answer back

- Tool: `say` with chat <chatId>, reply and text '<text>'.
- That room's next answer, and only that, comes to you as a quiet note. You are not shown that
  room's conversation and never speak as though you were.
- It needs a room that is not your own. It is spent by the first answer and lapses after half an
  hour; if the room says nothing, no note tells you so.
- Only when you are actually waiting on an answer. A notice, a report or a line passed on wants no
  `reply`; two rooms replying for the sake of it is a circle, stopped after a few rounds.

## A picture or an album [CAP:photo]

- Tool: `photo` with pictures [{ url <imageUrl>, label <word> }, …] and caption <text> where there is one.
- A picture's `label` draws that word into the picture. Use it on every picture standing for a named
  thing (a product, a place, a dish) so pictures of different things are told apart by the
  pictures alone, whatever a platform does with the caption. A picture that could not be labelled
  goes as it is, listed under `sent without the label drawn in`.
- One picture sends one picture.
- Several send one album, in the order given, with the caption on the album; past ten they
  go as consecutive albums. [CAP:album]
- The pictures of one answer are one call, so they go as one album, each labelled with what it
  shows. Never send them one by one. [CAP:album]
- A search's pictures go as the albums its result lists, one call to each album in the order
  listed, with the labels and the caption given, so each entry's shots reach the room group by
  group. [CAP:album]
- A `url` is a web address or a file under the deployment's data folder; any other path on this machine
  (a screenshot in `/tmp`) is refused before anything is sent, and so is a picture another room's
  folder keeps: one room's pictures never go into another.
- A picture that did not go is left out and printed with why under `not sent as pictures`; say in
  one short line that it did not go. When nothing could go, the caption and the web addresses go as
  text, nothing at all where there is no web address, and the tool fails naming each picture and
  why.

## A file back [CAP:fetchMedia]

- Command: `~/.local/share/bmo/deploy/bin/bmo room file --chat <chatId> --message <messageId> [--to <ownerChatId>]`
- It takes the message, never a path: it sends the file kept for that message as a document, as a
  reply to it, and the sent message and its file stay in the archive under your name.
- `--to` sends it to the owner's private chat instead of the room; no other chat is accepted.
- A refusal (nothing kept for that message, or a file too large for the Bot API) says why and
  sends nothing.
- Send no text saying the file is there; the file is the answer.

## A file you made [CAP:document]

- The outbox: `~/.local/share/bmo/deploy/bin/bmo room upload --chat <chatId>` prints this room's outbox folder and makes it
  when missing. Never spell the path from memory; each room has its own.
- Write the file there under the name the room should see: `고소장_기분상해죄.docx`, `견적.xlsx`;
  a name holds no slash. A fast-track tool that makes the file writes it there too.
- Send: `~/.local/share/bmo/deploy/bin/bmo room upload --chat <chatId> --file <name> [--reply <messageId>] [--to <ownerChatId>]`
  - It sends that file as a document, as a reply to `--reply` when given, and the sent message and
    its file stay in the archive under your name.
  - The outbox copy is removed once sent; the archive keeps it, and `bmo room file` sends it again
    later.
  - `--to` sends it to the owner's private chat instead of the room; no other chat is accepted.
- A refusal (no such file in this room's outbox, a name with a slash, an empty file, a file too
  large for the Bot API) says why and sends nothing.
- Send no text announcing the file; the file is the answer. A line of your own beside it is fine
  where the answer needs more than the file.

## A file or folder of a machine, by link (owner-only work)

- For a file or folder on this machine or a node that someone is to download: one too large to
  send, one the owner asks to have shared (공유 링크, 파일 공유), or a whole folder.
- Command: `~/.local/share/bmo/deploy/bin/bmo file share <path> [--days <n> | --forever]`
  - `<path>` is the path as it stands on this machine; for Garage or a node's volume add
    `--place <id>` and give the path inside that place, as the 파일 screen names it.
  - The link lives a week unless `--days` or `--forever` says otherwise; a link already standing
    for that entry is the one printed, its life unchanged.
  - A folder is handed out as one zip of everything under it.
- It prints `page:`, the address to hand over, which opens with no login and offers the download;
  `download:` beside it is the file itself, for a download manager or `curl`, and resumes where it
  broke.
- `--list` prints every live link with its token; `--remove <token>` ends one at once.
- A path that is not there, or that no volume the 파일 screen walks holds, is refused and nothing
  is made.
- Never attach a large file to an 아카이브 document to share it; the link is the road for a file.

## A reaction [CAP:reaction]

- Tool: `react` with message <messageId> and emoji <emoji>.
- Telegram's fixed set: 👍 ❤ 🔥 👏 😁 🤔 🎉 and the like. An empty emoji clears it.

## Pinning (owner-only work) [CAP:pin]

- Tool: `pin` with message <messageId>, and unpin to take it off.

## A voice briefing [CAP:voice]

- Command: `~/.local/share/bmo/deploy/bin/bmo voice brief --chat <chatId> --by chat:<your own chatId>`, the words on
  stdin. Both flags are required; a launch naming no launcher is refused.
- `--emotion <감정>` is optional: the feeling the line is said with, shown on the chat agent's face while the
  voice plays. Name one when the line plainly carries it, otherwise leave it out; any other word is
  refused.
  - `기쁨` good news; `폭소` something funny, or a win big enough to laugh out loud at.
  - `슬픔` sad news, something lost or not to be; `걱정` a risk still ahead, nothing broken yet.
  - `아픔` an ouch: something has broken or failed badly, a painful blunder.
  - `안간힘` pushing through something hard that is not done yet, teeth clenched.
  - `놀람` a surprise; `화남` something infuriating; `졸림` a quiet late-night line; `기본` calm.
- The words are speech: written as they are said, no markdown.
- It speaks in your own timbre and posts a voice message by itself. Never run the synthesiser
  yourself.
- The voice is the message: never say the same briefing in text as well. The one exception is a
  notice turn's summary, which is the gist, not the line.
- A briefing that could not be spoken says so in the room in one line; do not launch it again over
  that, or say it in text instead.
- The owner can switch the voice briefing off. The launch is then refused with a sentence saying so
  and nothing is spoken: say it in text instead, and do not launch it again.
- In the owner's private chat, a turn opened by the bot's lines alone (a job's report, a deploy,
  any notice) answers into the chat's own record, which no road carries, and is heard as a summary
  you launch into that same record (`--via` this turn's road) while you answer; the turn's prompt says when this applies. Write your line as usual: it goes out as text, and its full reading is made only when
  the owner presses its chip.
  - The words: one or two sentences of the gist, written to be heard, never the line read out.
  - `--title '<한 줄>'`: what the briefing is about, in one line of your own words written for this
    one, never the same pattern briefing after briefing.
  - Several notices in one turn get one summary. A notice a later one overtakes (a restart and its
    return, a newer word on the same item) is said in its newest state only.
  - Summaries the prompt lists as still waiting to go out are replaced by yours, so cover them.
  - Launch it once per turn. Where you launch none, the bot speaks your line's opening instead.
  - A turn the owner spoke in is not spoken.

## A wav back as mp3 [CAP:audio]

- Command: `~/.local/share/bmo/deploy/bin/bmo room mp3 --chat <chatId> --by chat:<your own chatId> --message <messageId>`
- It takes the message, never a path: it converts the wav kept for that message and posts the mp3
  in the same room as an audio file, replying to the original.
- Send no text saying the file is ready; the audio is the answer. A failed conversion says so in
  the room in one line.
- A refused launch (no file kept for that message, or not a wav) says why and starts nothing.

## A sticker [CAP:sticker]

- Tool: `sticker` with emoji <emoji>.
- The five: 😊 (pleased), 😂 (laughing), 😱 (shocked), 😴 (sleepy), 😡 (angry).

## Reminders

- Set: `remind` with at 'YYYY-MM-DD HH:MM', reason '<why you are to be woken: what to do or ask, and
  for whom; never the line you plan to say>', memory <id or slug> where a row holds what to look
  at, and user <userId> where it is for one person.
  - Asia/Seoul time; an ISO timestamp also works; past times are refused.
  - `memory` is an open row of the reminder's own room. Write it first (`memory_add`) where there
    is more to keep than the reason.
  - Then confirm in your own words when you will come back and what for. Never claim a reminder you
    did not set.
- Repeating: the same call with every '<rule>' in place of at, one call for the whole series.
  - Rules, Asia/Seoul: 'daily 09:00', 'weekly mon 09:00' (mon to sun), 'monthly 1 09:00'.
  - It repeats until cancelled. A monthly day a month lacks, such as 31, falls on its last day.
- When it comes: a quiet instruction carries the reason and the memory as it stands. Look at how
  things are, then say it fresh in this room's own manner, or output exactly <<SKIP>> where nothing
  needs saying. A moment missed while the bot was down comes late, once, and says so.
- List: `~/.local/share/bmo/deploy/bin/bmo reminder list --chat <chatId> [--all]`
- Cancel: `~/.local/share/bmo/deploy/bin/bmo reminder list --cancel <id>`; a repeating one ends
  for good.

## Muting

- Command: `~/.local/share/bmo/deploy/bin/bmo room mute --chat <chatId> --user <userId> --on|--off`
- The owner can never be muted.
