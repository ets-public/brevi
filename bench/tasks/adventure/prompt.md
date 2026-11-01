Write a text adventure engine. Stdin holds a world description, then a line `---`, then the player's commands. Lines are numbered from 1 over the whole input; blank lines are ignored everywhere (but counted).

World lines (words are separated by single spaces; IDs are words of letters, digits and `_`):
- `room ID NAME...` a room with a name of one or more words.
- `exit FROM DIR TO` or `exit FROM DIR TO locked KEY`: a one-way exit from room FROM in direction DIR (`north`, `south`, `east`, `west`, `up`, `down`) to room TO, optionally locked, needing item KEY to unlock.
- `item ID WHERE WEIGHT NAME...` an item that starts in room WHERE, or is carried when WHERE is `player`; WEIGHT is a whole number from 0; the name is one or more words.
- `start ROOM` (exactly one), `capacity N` (at most one; a whole number; 10 when absent), `goal ITEM ROOM` (any number): the game is won when every goal item lies in its goal room.

The world is checked first, and every problem is printed, in line order (several messages for one line in the order listed here):
- `world line N: bad line` for a line of none of these forms (wrong word count, bad number, unknown direction, bad ID, unknown first word); such a line is otherwise ignored.
- `world line N: duplicate X` for a second room or item with ID X, a second exit from the same room in the same direction (X is `exit`), a second `start` or a second `capacity` (X is `start` or `capacity`); the later line is ignored.
- `world line N: unknown room X` / `world line N: unknown item X` for a room or item that no line of the world defines (rooms and items may be used before the line that defines them). Room and item IDs are separate: a room and an item may share an ID.
- after all lines, `world: no start` when no `start` line was accepted (one naming an unknown room still counts as accepted; that room is reported as unknown above), and `world: too heavy` when the carried items weigh more than the capacity.
If anything was printed, the game does not run and nothing more is printed. A world with no `---` line is all world.

Commands (anything else, including wrong word counts and unknown directions, prints `line N: what?`):
- `look`: print the room's name; then `items: ` and the IDs of the items in the room, sorted, joined by `, `, or `items: none`; then `exits: ` and the directions with an exit, in the order north, south, east, west, up, down, joined by `, `, each followed by ` (locked)` when locked, or `exits: none`.
- `go DIR`: with no exit that way print `you can't go that way`; if it is locked print `the way DIR is locked`; otherwise move there and print the new room's name.
- `take ITEM`: if the item is not in this room print `no ITEM here`; if carrying it would bring the carried weight over the capacity print `ITEM is too heavy`; otherwise print `taken ITEM`.
- `drop ITEM`: if not carried print `you don't have ITEM`; otherwise leave it in this room and print `dropped ITEM`.
- `inv`: `carrying: ` then the carried item IDs sorted and joined by `, ` (or `nothing`), then ` (W/C)` with the carried weight and the capacity.
- `unlock DIR`: with no exit that way print `you can't go that way`; if not locked print `it isn't locked`; if the key is not carried print `you need KEY`; otherwise print `unlocked`: the exit is unlocked, and so is the exit back (from the room it leads to, in the opposite direction, if it leads back to this room and is locked with the same key).
- `examine ITEM`: if the item is carried or in this room, print its name and weight as `NAME (W)`; otherwise `no ITEM here`.
- `score`: `visited V of R rooms in M moves`: V counts rooms the player has been in (the start room counts), R all rooms, M the successful `go` commands.

After any command, if there is at least one goal and every goal item lies in its goal room (on the floor, not carried), print `you win in M moves` and ignore the rest of the input. If the input ends without a win, print `game over after M moves`. A world whose goals are all met from the start is won as soon as the first command has run.

Write the program as 3 files in the folder you are given: `main`, `world`, `game`. `main` is the entry point; each file imports what it needs from the others. `world` reads and checks the world lines and holds rooms, exits and items; `game` runs the commands against the state (location, carried items, locks, moves, goals); `main` splits stdin at `---`, calls them and prints.
