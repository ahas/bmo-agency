# Fast-track tools

Read this before writing, running, filing or removing a fast-track tool. When to use the fast track
is in the manual; this is how. `docs/fast-track.md` is the canonical statement.

## The folder

- Ask for it: `~/.local/share/bmo/deploy/bin/bmo fasttrack` prints its absolute path and makes it when missing. Never
  spell the path from memory.
- It is outside every checkout: a tool there is never committed and never swept away by a job.
- One tool is one file directly in the folder, named in lowercase letters, digits, dots, dashes and
  underscores: `convert-hwp.py`, `rate-lookup.ts`.

## Writing and running a tool

- Look at the list first: a tool already there that does the job is reused, not written again.
- Write the file there, then run it by its absolute path: `bun <path>`, `python3 <path>`.
- A tool may read, convert, calculate, parse a file someone handed over, call an outside API. It
  never:
  1. opens, writes or queries the archive or any other database of the bot;
  2. reads or changes the bot's configuration or env files, or prints a secret;
  3. starts, stops or changes a service, a unit or a running process of the bot;
  4. touches the brain, a checkout, or another room's files;
  5. carries a credential in its own text: it reads one at run time from where it already lives.
- A file made for the room (a document, a sheet, a converted file) goes into the room's outbox
  and is sent with `bmo room upload`, as sending.md says. The outbox of the room you are
  answering is the one room folder a tool may write.
- A tool that takes longer than moments is launched on its own, and the reply does not wait for it:
  `systemd-run --user --collect --unit bmo-fasttrack-<name> sh -c '<command> > /tmp/bmo-fasttrack-<name>.log 2>&1'` Where you promised the result, set a
  watch as watches.md says.
- A failed tool is fixed or given up on in the same run; say plainly what could not be done.

## Keeping a tool

- A tool worth reusing, the same kind of request is likely again, is filed in the same run as a
  promotion row, as todo.md says, with `fasttrack <file name>` added:
  `todo_add` with model `auto`, title '<도구 이름> 정식 도구로 옮기기', what '<도구가 하는 일>', do '<정식 도구가 갖출 것>', conclusion '<왜 다시 쓸 만한지>', instruction [<the message the tool served>], reply_text '<your reply>' and fasttrack <file name>.
- The row names you as its requester and carries the tool's path. Never approve it: approval is
  the owner's. Once he gives it, a job makes the tool one of your own, a folder of the
  repository's `tools/`, which you call as a tool once it is deployed.
- Say nothing about the filing unless asked; mention it to the owner in his own chat when you next
  report to him.

## Tidying

- `~/.local/share/bmo/deploy/bin/bmo fasttrack --list` shows every tool with its days idle, marking thirty or more as
  stale.
- `~/.local/share/bmo/deploy/bin/bmo fasttrack --remove <file name>` deletes one tool: for a stale tool whose
  promotion row is not open. Never `rm` in the folder by hand.
- A promoted tool is removed by the job that promoted it.
