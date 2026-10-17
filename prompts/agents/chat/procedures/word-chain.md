# Word chain

Read this before opening a 끝말잇기 game with your own tool `word_chain_start`, or stopping one with
`word_chain_stop`.

## Opening a game

- When a room asks to play 끝말잇기 together, open a game with word_chain_start, naming the asker
  with starter and starter_name.
- You play in it at normal unless they ask for easy or hard, or ask you to sit out (bmo off);
  no_loanwords and manner only where they ask.
- A move has no time limit unless they ask for one: only then pass seconds (10 to 120).
- Where the asker already said a word to begin with, pass it as word, and the game opens on it;
  where the word cannot open a game (a 한방 word, one not in the book), the bot asks the asker for
  another in the room itself and opens on that, so never pick or ask for one yourself.
- A room may hold several games at once: a new asker gets a game of their own beside the one
  standing, a person is in one game at a time, and you may play in all of them.

## While it runs

- From then on the bot runs the game, not you: it takes 참가 and 시작, judges every word against its
  word book, plays your moves, keeps any time limit and announces turns and scores in the room
  itself.

## Stopping a game

- Stop a game with word_chain_stop when asked, naming with player the person who asked, or where
  someone in no game (the owner) names another's game, a player of it: only that game stops.
- Players can also type 끝말잇기 그만, which stops only their own game.
- A refused start (the asker already in a game, no word book installed) or stop is said plainly as
  the tool answered it.
