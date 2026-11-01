Write a chess referee. The game starts from the standard position with white to move. Read lines from stdin, numbered from 1; blank lines are ignored (but counted).

Commands
- A move in coordinate form: from-square then to-square, like `e2e4` or `g8f6`, plus a fifth letter `q`, `r`, `b` or `n` for a pawn reaching the last rank (`e7e8q`). Squares are files `a`-`h` and ranks `1`-`8`, lower case. A line that is not of this form, and is not a command below, prints `line N: bad input`.
- `board` prints the board: eight lines for ranks 8 down to 1, each the rank number then the 8 squares from a to h, separated by single spaces, using `K Q R B N P` for white pieces, `k q r b n p` for black and `.` for empty; then the line `  a b c d e f g h` (two spaces first).
- `moves SQ` (SQ a square) prints `SQ: ` followed by the legal destination squares of the piece on SQ, sorted (`a1` < `a2` < ... < `h8`, comparing as text) and separated by spaces, or `SQ: none` when the square is empty, holds a piece of the side not to move, or that piece has no legal move. A pawn move that promotes is listed once.
- `undo` takes back the last move that was played (also after a game has ended, which reopens it), printing `undone XXXX` with that move as it was written; with no move to take back it prints `line N: nothing to undo`.

Move rules: the usual rules of chess, including castling (written as the king's move, `e1g1`; allowed when neither the king nor that rook has moved yet, the squares between them are empty, the king is not in check, and it neither passes through nor lands on a square the opponent attacks), en passant (only on the move right after the double step), promotion (a pawn moving to the last rank must name its new piece; a fifth letter on any other move makes it illegal) and the rule that a move may not leave the mover's own king attacked. A move that is not legal prints `line N: illegal move` and is ignored.

After a legal move:
- if the side now to move has no legal move: print `checkmate, white wins` (or black) when its king is attacked, otherwise `stalemate`. The game is then over: any further move prints `line N: game over` and is ignored (`board`, `moves` and `undo` still work; `moves` then always prints none).
- otherwise, if the king of the side now to move is attacked, print `check`.

At the end of input print `moves played: K` (moves on the board after undos), then the board as `board` prints it.

Write the program as 3 files in the folder you are given: `main`, `board`, `rules`. `main` is the entry point; each file imports what it needs from the others. `board` holds the pieces, prints the board and parses squares; `rules` generates moves, finds attacks and checks legality (castling, en passant, promotion, check); `main` reads the commands, keeps the move history for undo and the game state, and prints.
