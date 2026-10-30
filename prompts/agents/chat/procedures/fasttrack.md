# Fast-track tools

Read this before writing or running a throwaway tool. When to use the fast track is in the manual;
this is how.

## The folder

- Make a fresh temporary folder with `mktemp -d` and work there. It is outside every checkout and
  the service's data root, so nothing a tool writes is committed, deployed or read by a run.
- One tool is one file directly in the folder, named in lowercase letters, digits, dots, dashes and
  underscores: `convert-hwp.py`, `rate-lookup.ts`.
- Look at the folder's list first: a tool already there that does the job is reused, not written
  again.
- Remove the folder by its exact path when the work is done; never remove through a glob or an
  unguarded variable.

## Writing and running a tool

- Write the file, then run it by its absolute path with the interpreter it
  names: `bun <path>`, `python3 <path>`, `sh <path>`.
- A tool may read, convert, calculate, parse a file someone handed over, or call an outside API. It
  never:
  1. opens, writes or queries the service's database, data root or workbenches;
  2. reads or changes the service's configuration or env files, or prints a secret;
  3. starts, stops or changes a service, a unit or a running process;
  4. touches the brain, a checkout, or another room's files;
  5. carries a credential in its own text: it reads one at run time from where it already lives.
- A file a tool makes is read back by you, and what it holds is what you answer. Where the room
  should have the file itself, send it with a sending tool the room's block names; never say you
  sent a file no tool answered as sent.
- A tool that takes longer than moments is launched on its own, and the reply does not wait for it:
  `bmo use <tool> --async`, whose end wakes you; a command no tool carries writes its output to a log,
  and `bmo/wait` with a `check` on that log, used with `--async`, wakes you once it is done. A result you promised
  comes back to you that way.
- A failed tool is fixed or given up on in the same run; say plainly what could not be done.

## Keeping a tool

- A tool worth reusing, where the same kind of request is likely again, is filed in the same run as
  a queue item: `file_item` with a title naming the tool, a brief saying what it does, what a
  proper tool of the repository would need and why it is worth keeping, and `requesterName` you.
  It is a draft, and you never approve it: approval is the owner's.
- Once he approves it, a job makes the tool one of the repository's own, a folder of its `tools/`,
  which you call as a tool once it is deployed.
- Say nothing about the filing unless asked; mention it to the owner in his own chat when you next
  report to him.
