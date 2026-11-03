Score games of a five-letter word game. Each non-blank line of stdin is one game: the answer, then up to 6 guesses, separated by spaces, optionally with the word `HARD` before the answer. Lines are numbered from 1; blank lines are ignored.

A guess is scored letter by letter: `G` if the letter is in the same place in the answer; otherwise `Y` if the letter appears in the answer at a place not already marked `G` and not already used for an earlier `Y` of the same letter (looking at the guess from left to right); otherwise `.`.

Check each line in this order and print the first problem found, as `line N: ...`, instead of scoring that game:
1. `bad word W`: the first word (answer or guess) that is not exactly 5 lowercase letters a to z.
2. `too many guesses` if there are more than 6 guesses.
3. `guess after solve` if there is a guess after one that matched the answer.
4. In HARD mode, `guess K breaks hard mode` (K counts guesses from 1) if a guess does not keep every `G` letter from the earlier guesses in its place, or does not contain each letter at least as many times as it was marked `G` or `Y` in any single earlier guess.

Otherwise print `ANSWER: P1 P2 ... solved in K` (the patterns of all guesses) or `ANSWER: P1 P2 ... not solved` (also when there are no guesses: `ANSWER: not solved`). Lines with a problem are not counted below.

At the end print `played P, solved S, average A`, where A is the average number of guesses over solved games with 2 decimals, or `-` if none was solved, then `1:C1 2:C2 3:C3 4:C4 5:C5 6:C6` counting solved games by number of guesses.
