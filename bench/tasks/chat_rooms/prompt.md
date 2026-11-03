Simulate chat rooms. Lines of stdin are numbered from 1; blank lines are ignored. User names are letters and digits; room names are `#` followed by letters and digits. A line with the wrong number of words, a bad name or an unknown command prints `line N: bad line`.

- `JOIN U R`: U joins room R. If R does not exist it is created and U is its owner. If U is already in R print `line N: already in room`.
- `LEAVE U R`: U leaves R. If the owner leaves, the member who joined earliest among those left becomes the owner. A room left empty is deleted, with its messages and mutes. Leaving also forgets U's mute in R.
- `SAY U R TEXT...`: U posts TEXT (the rest of the line after R and the spaces that follow it, kept as written; at least one word) in R. A muted user prints `line N: muted`. Every distinct word of TEXT that is exactly `@` followed by the name of another member of R gives that member one mention in R.
- `MUTE O U R` and `UNMUTE O U R`: the owner O mutes or unmutes member U. If O is not R's owner print `line N: not owner`; if U is not in R print `line N: not in room`; if U is O print `line N: not allowed`.
- `READ U R`: shows U the messages posted in R by other users since U joined R or last read it, whichever is later. With none, print `R: nothing new for U`. Otherwise print `R: K new for U`, then `  (E earlier)` if there are more than 3 (E = K - 3), then the last 3 (or fewer) as `  SENDER: TEXT`. Reading also clears U's mentions in R.
- `WHO R`: prints `R: ` and the members sorted by name, separated by `, `, with `*` after the owner and `(muted)` after a muted member (`#a: amy*, bob(muted), cal`).
- `INBOX U`: prints `U: ` followed by, for each room U is in sorted by name, `R new=K at=M` (K messages READ would show now, M mentions), separated by `; `. If U is in no room, print `U: no rooms`.
- `STATS`: prints `rooms R, users U, messages M`: rooms that exist, distinct users in at least one room, and all messages ever posted.

For LEAVE, SAY, MUTE, UNMUTE, READ and WHO, a room that does not exist prints `line N: no such room`; for LEAVE, SAY and READ, a user not in the room prints `line N: not in room`. Check the room first, then the rest in the order given above.
