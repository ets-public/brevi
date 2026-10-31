Run programs in a small calculator language read from stdin, one statement per line (ignore blank lines and lines whose first non-space character is `#`). Numbers are 64-bit integers.

Statements:
- `let NAME = EXPR` sets a global variable.
- `fn NAME(P, P, ...) = EXPR` defines (or redefines) a function with zero or more parameters.
- `print EXPR, EXPR, ...` prints the values separated by single spaces.

Expressions, loosest first: comparisons `==`, `!=`, `<`, `>`, `<=`, `>=` (no chaining: `a < b < c` is a syntax error; they give 1 or 0); `+` and `-`; `*`, `/` and `%`; unary `-`; then numbers, names, `(EXPR)` and calls `NAME(EXPR, ...)`. Binary operators of the same level group left to right. `/` rounds toward zero and `%` has the sign of the left value (as in C). `if(C, A, B)` is built in: it evaluates C and then only A (when C is not 0) or only B. Names are letters, digits and `_`, starting with a letter; `let`, `fn`, `print` and `if` are not names. Spaces between tokens are optional.

Inside a function body a name is a parameter if it is one, otherwise a global variable as it is when the call runs. Functions may call themselves and each other, including functions defined later in the program, as long as they exist when the call runs.

Errors stop the current line only (nothing it would set changes, nothing it would print is printed) and print `error line N: MESSAGE`, N being the line number in the input: `syntax`, `unknown name X`, `unknown function F`, `F takes K arguments` (a call with the wrong count), `division by zero` (for `/` and `%`), `too deep` when a call would make more than 200 calls active at once. When a line has several errors, report the first one met while evaluating left to right; syntax errors are found before anything runs.
