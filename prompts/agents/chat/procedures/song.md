# Songs

Read this before launching a song. Making one is owner-only work (ABSOLUTE RULE 2); your own tool
is `song`, and the deployed `song.ts` does the same from a shell.

## A song in the owner's words [CAP:document]

- Tool: `song` with title '<곡 제목>', and only what he said besides:
  - artist '<가수>' where he named the singer;
  - remix where he asked for a remix;
  - style '<tags>' with remix, only where he named a style for it: his words turned into one line
    of comma-separated English tags, 300 characters at most, a tempo as `<n> BPM`, e.g.
    `lo-fi hip hop, chill, mellow piano, 80 BPM`;
  - transpose <n> where he asked for it higher (+) or lower (-), in whole semitones, never with
    remix;
  - voice 'natural' where he asked for it without the chat agent's voice or timbre, cover or remix alike. A
    style never takes the chat agent's voice out; only this option does.
- Command: `~/.local/share/bmo/deploy/bin/bmo song --chat <chatId> --by chat:<your own chatId> --title '<곡 제목>' [--artist '<가수>'] [--remix [--style '<tags>'] | --transpose <n>] [--voice natural]`
- The title is all the tool needs. Never look up, choose or write the lyrics, the recording or
  backing video, the manner of singing beyond the voice he asked for, the mix or the key, never put any of them in a call, a row
  or a job's prompt, and never put a line of the lyrics in an answer.
- His song is never filed as an item or handed to a job: the tool does the whole of it.
- Say in one short line that the song is being made; the videos arrive in the room by themselves,
  one per take, over tens of minutes. Do not wait for it or poll it.

## When it comes back

- Done: say which posted video is which take, and which takes were dropped, from what the
  instruction lists. Never judge or recommend a take and never cite a number as a reason one is
  good; the owner listens and chooses.
- Lyrics to be formed (a cover or a remix alike): file an item with `todo_add` whose `do` asks for
  the lyrics put into YuE2's form under the `song-cover` skill and the command the instruction
  names run again, carrying that command and nothing of the lyrics, and approve it. It stays on the
  mother worker, where every item goes.
- Failed: say so plainly, in the tool's own reason, and launch it again only once the cause is
  known.

## A member asking for a song

- Launch nothing. File their request with `todo_add` in their room, unapproved: the title and only
  the options they said, and that a job runs the song tool's `make` for this room. Say plainly that
  it waits for the owner's approval.
