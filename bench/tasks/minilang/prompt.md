Write an interpreter for a small programming language. The program to run is read from stdin. It is first checked in full; only if it has no syntax error does it run.

Lines and tokens
- One statement per line. Blank lines are ignored, `#` starts a comment that runs to the end of the line (outside strings), and indentation means nothing.
- Tokens: integers (`0`, `42`; no sign, a minus is an operator), strings in double quotes (no escapes, cannot span lines), names (`[A-Za-z_][A-Za-z0-9_]*`), and the symbols `== != <= >= < > = + - * / % ( ) [ ] ,`. Spaces separate tokens and are otherwise ignored. Any other character, or a string with no closing quote, is a syntax error.
- Keywords (not usable as names): `print if elif else end while for in func return break continue and or not true false none`.

Statements
- `print E1, E2, ...` (at least one expression) prints the values separated by single spaces.
- `NAME = E` sets a variable. `NAME[I] = E` sets an item of the list in NAME.
- `if E` ... optional `elif E` blocks ... optional `else` ... `end`.
- `while E` ... `end`. `for NAME in E` ... `end`. `break` and `continue` work on the innermost loop.
- `func NAME(P1, P2, ...)` ... `end` defines a function (zero or more parameter names). `return` or `return E` leaves it; a function that ends without returning a value returns `none`.
- A call on its own, `NAME(ARGS)`, is a statement too (its value is dropped).

Syntax errors (print `syntax error line N` and stop; nothing runs). N is the first line, from the top, on which one of these is found, except for a missing `end`, which is reported at the line that opened the innermost block left open at the end of the program:
- a line that is not one of the statements above, has extra tokens after one, or has a bad expression;
- `elif`, `else` or `end` with no block to belong to, `elif` or `else` after an `else`, or `elif`/`else` in a `while`, `for` or `func` block;
- `func` anywhere but at the top level (inside any block), a function name used twice, or a function named like a built-in;
- `return` outside a function; `break` or `continue` outside a loop (a loop outside the function does not count).

Expressions, from lowest to highest precedence:
1. `or`, then `and` (both left to right, and both stop early: `false and X` never evaluates X)
2. `not E`
3. one comparison at most: `== != < <= > >=`
4. `+ -` (left to right)
5. `* / %` (left to right)
6. unary minus
7. indexing `E[I]` and calls `NAME(ARGS)`; primaries are integers, strings, `true`, `false`, `none`, names, `( E )` and list literals `[E1, E2, ...]` (possibly empty).

Values and rules
- Values are integers, strings, booleans, `none`, and lists. Lists are shared, not copied: after `b = a`, a change made through `b` is seen through `a`.
- `if`, `while`, `and`, `or` and `not` need booleans, otherwise it is a type error.
- `+` adds integers and joins two strings or two lists (into a new list). `- * / %` need integers; `/` and `%` round toward minus infinity (`-7 / 2` is `-4`, `-7 % 2` is `1`); dividing by zero is an error `division by zero`.
- `==` and `!=` compare any two values: values of different types are never equal (`1 == true` is false); lists are equal when their items are. `< <= > >=` compare two integers or two strings (by character codes); anything else is a type error.
- Indexing works on a list or a string (giving a one-character string), with an integer index; a negative index counts from the end (`-1` is the last). Out of range is the error `index out of range`; anything else is a type error.
- Variables: at the top level, a name is a global variable. Inside a function, parameters and names assigned in it are local to that call; reading a name looks at the call's locals first, then the globals. Reading a name that is not set is the error `unknown name NAME`.
- `for NAME in E`: E must be a list (its items as they are when the loop starts) or a string (its characters); anything else is a type error. NAME is set like a variable on each round.
- Functions can be called before the line that defines them. Calling a name that is neither a function nor a built-in is the error `unknown function NAME`; the wrong number of arguments is the error `wrong number of arguments`.
- Built-ins: `len(X)` (list or string), `str(X)` (the value as `print` shows it), `int(X)` (an integer is returned as it is; a string of digits with an optional leading `-` becomes that integer; any other string is the error `bad number`; other values are a type error), `push(L, X)` (adds X to the end of list L, returns `none`), `pop(L)` (removes and returns the last item; an empty list is `index out of range`), `range(N)` (the list `[0, 1, ..., N-1]`, empty when N <= 0). Wrong argument types are type errors.
- Printing: integers in decimal; strings as they are; `true`, `false`, `none`; lists as `[A, B, C]` with `, ` between items, where strings inside lists are shown in double quotes: `[1, "a", [true]]`.

Runtime errors print `error line N: MESSAGE` (MESSAGE is one of `unknown name X`, `unknown function X`, `wrong number of arguments`, `type error`, `division by zero`, `index out of range`, `bad number`) and stop the program; what was printed before stays. N is the line of the statement that was running (inside a function, the statement in the function body, not the line of the call).

Write the program as 4 files in the folder you are given: `main`, `lexer`, `parser`, `interp`. `main` is the entry point; each file imports what it needs from the others. `lexer` turns a line into tokens; `parser` checks the whole program and builds its statements and expressions; `interp` runs them (values, scopes, built-ins, errors); `main` reads stdin, calls the others and prints.
