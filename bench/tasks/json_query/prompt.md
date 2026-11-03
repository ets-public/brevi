Answer queries about a JSON document. Stdin holds the document (over one or more lines) up to a line that is exactly `%%`; each non-blank line after it is a query. Queries are numbered from 1.

A query is a chain of steps, optionally followed by `|` and a function name. Spaces between tokens are allowed anywhere (but not inside a key, number or string). A query that is only `.` means the whole document. Steps:
- `.key` where key is a letter or `_` followed by letters, digits or `_`; or `["any key"]` with a JSON string. Picks that member of an object.
- `[N]` with N a whole number, maybe negative (counting from the end, `-1` is the last). Picks that element of an array.
- `[*]` picks every element of an array, or every member value of an object in document order.
- `[?key OP VALUE]` keeps the elements of an array that are objects having member `key` whose value compares true with VALUE. OP is `==`, `!=`, `<`, `<=`, `>` or `>=`; VALUE is a JSON number, string, `true`, `false` or `null`. `==` and `!=` compare as JSON (numbers by value; `true` is not `1`); the other four are true only when both sides are numbers (not booleans) or both are strings (compared as text).

The query starts with a list holding the document, and each step replaces every value in the list with what it picks from it, in order. A value a step cannot pick from (a missing key, an index out of range, or the wrong type) gives nothing.

Functions, applied after all steps: `count` gives one value, how many values there are; `sum` gives one value, the sum of the values that are numbers (not booleans), an integer if they all are, else a float (0 if none); `first` keeps only the first value, if any; `keys` replaces each object with the sorted list of its keys and drops other values; `length` replaces each string, array or object with its length and drops other values.

Print each resulting value on its own line as Python's `json.dumps(value)` prints it (default settings; objects keep their key order). If there are no values, print `(none)`. A query that does not fit this grammar prints `query K: syntax error`.

Write the program as 3 files in the folder you are given: `main`, `lexer`, `query`. `main` is the entry point; each file imports what it needs from the others. `lexer` turns a query into tokens; `query` parses the tokens into steps and runs them on the document; `main` reads stdin, splits off the document and prints the results.
