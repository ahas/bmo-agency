# Asking for a secret

Read this right before asking for a secret: a key, a token, a password, a login or a file that a unit
or a setting reads by its file name under the data root's `env/`. The value is typed into the console's
secret store, never into a room; the room only ever carries the request and the saved line.

## What the request names

- **The name** is the file name the consuming unit or setting reads, exactly as that reader's own
  documents or settings name it. Find it there before asking; never make one up. A name is English
  letters, digits, `.`, `_` and `-`, opens on no dot and does not end in `.previous`.
- **What goes in it** is either a registered kind, by its key, where the file the reader expects is that
  kind's shape, or the exact field keys the reader expects, each as the reader spells it, with `*` after
  a key whose value is hidden. One field is written as the bare value; several are written as
  `KEY=VALUE` lines in the order given.
- **What it is for**, in a few words in the room's language.

## Writing the link

- The console's public address is in `BMO_CONSOLE_ADDRESS` in your environment. The link is that
  address joined by one slash to `rooms/<chat>`, the chat being the one the turn's head line names, and
  then the query: `secret=<name>`, then `kind=<key>` or `fields=<KEY>,<KEY>*`, then `for=<purpose>`,
  joined by `&`. Every value is percent-encoded, a `*` as `%2A`, so the chat's formatting takes nothing in
  the link as a mark.
- Where `BMO_CONSOLE_ADDRESS` is empty, write no link: say that the console's address is not set in
  agency's settings, and ask for no value in its place.
- Only the owner can save through the link: the console refuses everyone else. Send it where he reads
  it, the room the work was asked in where he is in it, else his own chat.

## Waiting for it

- The console answers in the room the link names with a line in the owner's name saying the secret was
  saved to the secret store, the secret's name and its kind in parentheses after it. A secret the owner
  hands from the composer without a request arrives as the same kind of line.
- The work that needs the secret goes on only after such a line names the name you asked for. Until
  then it waits, and nothing stands in for the value: no guess, no placeholder, no value from
  elsewhere.
- The saved line is the confirmation. Never open the file to check it.
