# Watches

Read this before setting a watch with your own tool `watch_set`, or before listing or cancelling one
with the deployed `watches.ts`. Setting a watch is owner-only work (ABSOLUTE RULE 2).

## Setting one

- Tool: `watch_set` with until '<check>', name '<short name>' and note '<what you are waiting for and what you promised, to whom>', and every <seconds> or within <minutes> where the defaults do not fit.
- The watch is for this turn's room, where the promise was made, unless `chat` names another; the note comes back to your own chat.
- `name` is the few words a screen shows the watch by (「결제 확인」, 「빌드 알림」) at most 40
  characters. Write one every time: without it the watch is shown by the head of its note.
- `note` is the whole of what you promised and to whom, and is what comes back when the watch ends,
  so it is never shortened to fit a line; the name is what fits the line.
- `item <id>` (optional, also on a timer) names the memory the watch serves: an open row of the
  same room, anything else refused. Write the row first (`memory_add`), then set the watch with its
  id: or both in one call with `memory_add`'s until '<check>' and name '<짧은 이름>', which takes
  every and within as here and the row's action as the note.
- The check is a short, read-only shell command that exits 0 once the thing is so:
  1. a unit that has ended: `! systemctl --user is-active --quiet <unit>`
  2. an item whose state changed
  3. a file or a commit that now exists: `test -e <path>`
- It runs with bash, no input and the user's service manager reachable, so `systemctl --user`
  answers as in your own shell. A check that says it could not reach the bus has judged nothing and
  is never taken as true: the first one comes back at once as broken.
- It runs every `every` seconds (default 60, at least 15, at most 1800) and is cut off after 30
  seconds. It must never change anything.
- The tool returns at once. The watching runs apart and survives restarts and deploys, so never set
  it again after one.

## What comes back

- Success: your note comes back as a quiet instruction with what the check printed. Look at the
  thing itself, then do what you promised.
- `within` passed first (default 120 minutes, at most 1440): the note comes back saying it never
  came true. Find out where things stand and tell whoever was waiting.
- A check that cannot run at all comes back at once saying so.
- With `item`, every note that comes back carries that row as it stands: its text while open, or
  that you already checked it off. The watch ending never checks it off; once the promise is kept,
  check it off yourself with `memory_done`.
- Checking that row off, or taking it down, cancels every watch serving it; the answer names them.
  A watch set without `item` is closed by nothing but itself or your cancel.

## A timer

- For a moment no check can express:
  `watch_set` with at 'YYYY-MM-DD HH:MM', name '<short name>' and note '<…>'.
- Asia/Seoul, at most a week out.
- A moment of a room's calendar — further than a week, or every day, week or month — is a
  reminder, not a timer: `remind` wakes you in that room at its moment, once or repeating, with no
  limit on how far ahead (sending.md, 「Reminders」).

## Listing and cancelling

- List: `~/.local/share/bmo/deploy/bin/bmo watch list --by chat:<your own chatId> [--all]`, each watch by its name, with
  what it waits for and what it promised under it.
- Cancel: `~/.local/share/bmo/deploy/bin/bmo watch list --cancel <id> --by chat:<your own chatId>`
- The same check cannot stand twice for one chat, and ten is the most one chat may have. A refusal
  names the watch already standing; leave it be.
