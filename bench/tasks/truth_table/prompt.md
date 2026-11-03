Write a tool for boolean formulas. Each non-blank line of stdin is a command (lines numbered from 1):

- `table F` prints the truth table of formula F.
- `taut F` prints `tautology` if F is true for every assignment, else `not a tautology: ` followed by the first falsifying assignment.
- `sat F` prints `satisfiable: ` followed by the first satisfying assignment, or `unsatisfiable`.
- `equiv F ; G` prints `equivalent` if F and G agree on every assignment of their variables together, else `differ at ` followed by the first assignment where they differ.

Formulas: variables are a lowercase letter followed by lowercase letters or digits; `0` and `1` are constants; parentheses group. Operators, from lowest to highest precedence: `<->` (equivalence), `->` (implies, right-associative: `a -> b -> c` is `a -> (b -> c)`), `|` (or), `^` (exclusive or), `&` (and), `!` (not, prefix). `<->`, `|`, `^` and `&` are left-associative. Spaces between tokens are optional.

Assignments run over the formula's variables (for equiv, both formulas' variables) sorted as text, counting in binary from all 0s to all 1s with the first variable as the highest bit. An assignment is written `a=0 b=1`. With no variables, the only assignment is the empty one: write `not a tautology`, `satisfiable` and `differ` with nothing after them (no trailing space or colon).

A table prints a header line: each variable, then `|`, then F exactly as written after `table ` with spaces trimmed from both ends, all separated by single spaces. Then one line per assignment: each variable's value padded on the right to the width of its name, then `|`, then the result, separated by single spaces (with no variables a line is just `| 0`).

A formula that does not parse prints `line N: syntax error`; so does an equiv line without exactly one `;`, or a line starting with another word.
