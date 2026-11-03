Score ten-pin bowling games. Each non-blank line of stdin is one game: a player name, then the number of pins knocked down by each roll, separated by spaces. Lines are numbered from 1; blank lines are ignored.

Rules: a game has 10 frames. In frames 1 to 9, a roll of 10 is a strike and ends the frame; otherwise the frame has two rolls whose sum is at most 10 (a spare if it is exactly 10). A strike scores 10 plus the next two rolls; a spare scores 10 plus the next roll; any other frame scores its pins. In frame 10, a strike earns two bonus rolls and a spare earns one, all thrown in frame 10; frame 10 scores the sum of its rolls. After a strike in frame 10, if the first bonus roll is not a strike, the two bonus rolls add up to at most 10.

For each game print one line:
- `NAME: SCORE` for a complete game.
- `NAME: invalid` if a roll is not a whole number from 0 to 10, a frame has too many pins, or there are rolls after the game is over.
- `NAME: incomplete after frame K, score S` if the rolls run out first. K is the number of frames fully rolled. S adds up the completed frames in order and stops at the first one whose bonus rolls have not all been thrown yet.

Check the rolls in order: the first problem found decides between invalid and incomplete. A line with a name and no rolls is `incomplete after frame 0, score 0`.

At the end print `best: NAME SCORE` for the highest complete game (the first one in the input on a tie), or `best: none` if no game is complete.
