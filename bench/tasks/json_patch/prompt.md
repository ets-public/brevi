Apply edits to a JSON document. The first line of stdin is a JSON document on one line. Every later line (numbered from 2, blank lines ignored) is an operation:

- `add PATH VALUE`, `replace PATH VALUE`, `test PATH VALUE`: VALUE is the rest of the line after PATH, a JSON text (it may contain spaces).
- `remove PATH`, `print PATH`
- `move FROM PATH`, `copy FROM PATH`

A PATH starts with `/`. The path `/` alone means the whole document. Otherwise it is split on `/` into keys after the first `/`; in a key, `~1` stands for `/` and `~0` for `~`. In an object a key names a member; in an array it must be an index written as a whole number without leading zeros (`0`, `7`), or `-` in the cases below.

What each operation does:
- `add`: the container the last key points into must exist. In an object, set the member (adding or overwriting it). In an array, the key is an index from 0 to the array's length, and the value is inserted there; `-` appends. Adding at `/` replaces the whole document.
- `remove`: the value must exist; remove it (later array items move down). Removing `/` is an error.
- `replace`: the value must exist; replace it (at `/`, the whole document).
- `move`: the value at FROM must exist, and PATH may not be inside FROM (PATH starts with FROM followed by `/`). Remove it from FROM, then `add` it at PATH. Moving to the same path changes nothing.
- `copy`: like move, but FROM keeps its value and PATH may be inside FROM; PATH gets an independent copy.
- `test`: the value must exist and equal VALUE, or the program prints `line N: test failed`. Equal means the same JSON type with equal contents, except that numbers compare by value (`1` equals `1.0`); `true` is not equal to `1`. Object member order does not matter.
- `print`: print the value at PATH as compact JSON (below), or `line N: error` if it does not exist.

A line that is not one of these forms, with a bad PATH (not starting with `/`) or a VALUE that is not valid JSON, prints `line N: bad line`. An operation whose paths do not work as described prints `line N: error` and changes nothing.

At the end print the document. Compact JSON is what Python's `json.dumps(value, sort_keys=True, separators=(",", ":"))` prints.
