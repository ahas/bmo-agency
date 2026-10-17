# The morning news page

Read this before running `bmo news-brief`. Every command here is owner-only work (ABSOLUTE
RULE 2): a room gets a page every morning because the owner said so, and nobody else's word adds,
changes or switches one.

## What it is

- On working days (Monday to Friday, not on a day the holiday calendar holds) each switched-on
  room is sent a news page at its own time: the title, a few summary lines and a share link living
  the room's share hours.
- The machine writes and sends it on its own; nothing comes to you when it goes out, only a morning
  that failed, as a `news-brief-failed` fact.

## Commands

- The rooms and the calendar: `~/.local/share/bmo/deploy/bin/bmo news-brief`
- Add a room, off: `~/.local/share/bmo/deploy/bin/bmo news-brief --add --chat <chatId> [--at HH:MM] [--lead <분>] [--share-hours <n>]`,
  defaults 08:30, writing forty minutes before, a link living twenty-four hours.
- A trial of a room's page, sent to the owner instead: `~/.local/share/bmo/deploy/bin/bmo news-brief --run <id> --send-to <owner's chatId>`,
  it takes a few minutes and arrives by itself; say it is on its way and stop.
- Switch a room on once the owner has seen a trial and said to: `--on <id>`; off again: `--off <id>`.
- Change a room's time, lead or link life: `--set <id> [--at HH:MM] [--lead <분>] [--share-hours <n>]`.
- Take a room away: `--remove <id>`.
- The holiday calendar: `--holidays [<year>]`; a temporary holiday the government announces, or a
  year not yet in it: `--holiday-add YYYY-MM-DD '<이름>'`; `--holiday-remove YYYY-MM-DD`.

## When the calendar is empty

- A `news-brief-calendar-empty` fact means the calendar holds no day of this year, so pages go out
  every weekday, holidays included. Tell the owner, and add the year's holidays once he has
  confirmed them.
