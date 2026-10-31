Write a small regular-expression search. The first line of stdin is a pattern; every later line is a text line to search.

Pattern syntax:
- a plain character matches itself; `\` followed by any character matches that character literally (`\.`, `\*`, `\\`).
- `.` matches any one character.
- `[abc]` matches one of the listed characters, `[a-z]` a range (ranges and single characters can be mixed: `[a-cx_0-9]`), and `[^...]` any character not in the set. Inside brackets every character is literal except `^` right after `[`, `-` between two characters, and `]`, which closes the set. A `-` first or last in the set is literal.
- `*` (0 or more), `+` (1 or more), `?` (0 or 1), `{n}` (exactly n) and `{m,n}` (m to n) repeat the item just before them. n and m are whole numbers.
- `^` as the very first character anchors the match to the start of the line, and `$` as the very last character anchors it to the end. Anywhere else they are plain characters.

For each text line (numbered from 1, blank lines included), find the match that starts leftmost; among the matches starting there, take the longest. If the line matches, print `N: S-E [TEXT]`: the line number, the start and end position (0-based, end exclusive) and the matched text. An empty match counts as a match.

After the last line print `K of T lines match`.

If the pattern is invalid, print only `bad pattern` and nothing else. A pattern is invalid if it has a `[` with no closing `]`, an empty set `[]` or `[^]`, a range whose start is after its end (`[z-a]`), a repeat with nothing before it (at the start, or right after `^`), a repeat right after another repeat (`a**`, `a+?`), a `{` that is not `{n}` or `{m,n}`, `{m,n}` with m greater than n, or a `\` at the very end.
