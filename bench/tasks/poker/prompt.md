Write a poker hand ranker. Each line of stdin (numbered from 1) is `NAME C1 C2 C3 C4 C5`: a player and five cards. A card is a rank (`2`-`9`, `T`, `J`, `Q`, `K`, `A`) followed by a suit (`S`, `H`, `D`, `C`). A blank line ends a round; the input's end ends the last round (a round with no lines at all after a blank line is not a round).

Checks, in order, for each line: anything but a name and five well-formed cards, or the same card twice in the hand, prints `line N: bad hand`; a name already playing in this round prints `line N: duplicate player`; a card already dealt to an earlier player in this round prints `line N: card XX already dealt` (the first such card in the hand). Such lines are ignored.

Hand categories, best first: straight flush, four of a kind, full house, flush, straight, three of a kind, two pair, pair, high card. A straight is five consecutive ranks; A-2-3-4-5 is a straight whose high card is 5 (and also a straight flush when suited). Hands of the same category compare by their ranks in this order: four of a kind by the quad then the kicker; full house by the triple then the pair; three of a kind by the triple then the remaining cards high to low; two pair by the higher pair, the lower pair, then the kicker; pair by the pair then the rest high to low; flush and high card by all five cards high to low; straights by their high card. Suits never break ties.

For each round print `round R` (counting rounds from 1) and then each player best first as `P. NAME CATEGORY`. Tied hands share the position number and are listed by name; the next position skips (1, 1, 3). A round with no valid players prints `round R` and `no players`.

At the end print `R rounds`.
