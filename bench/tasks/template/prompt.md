Stdin has a JSON object on the first line, then a template (all remaining lines). Render the template with the data and print it.

- `{{name}}` inserts a value. Names can be dotted paths into objects: `{{user.city}}`. Strings insert as-is, numbers as JSON writes them (integers without a decimal point), `true`/`false`, and `null` or a missing value insert nothing. Arrays and objects insert as compact JSON.
- `{{name|upper}}`, `{{name|lower}}`, `{{name|len}}` (length of a string or array), `{{name|default:TEXT}}` (TEXT if the value is missing, null or an empty string). Filters can chain: `{{name|default:anon|upper}}`.
- `{{#each items}}...{{/each}}` repeats its body for each element of the array `items`. Inside, `{{.}}` is the element itself, names are looked up in the element first (if it is an object) and then outside, and `{{@index}}` is the 0-based index. Blocks can nest.
- `{{#if name}}...{{else}}...{{/if}}` renders the first part if the value is truthy (not missing, null, false, 0, empty string or empty array), otherwise the `{{else}}` part, which is optional. Blocks can nest.
- Everything else is copied unchanged, including newlines.
