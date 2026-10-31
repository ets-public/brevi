Read an INI file, then answer queries. Stdin has the INI text, then a line `---`, then one query per line.

INI rules: a line `[name]` starts a section (trim spaces inside the brackets). `key = value` sets a key (trim spaces around key and value; split at the first `=`). Lines starting with `;` or `#` (after leading spaces) and blank lines are ignored. Keys before any section belong to section `default`. A key set again in the same section replaces the earlier value. Section and key names are case-sensitive.

Values may contain references `${key}` (looked up in the same section, then in `default`) or `${section.key}`. References are replaced recursively. A reference to a missing key becomes the empty string. If resolving a value would need a key that is already being resolved (a cycle), the whole queried value is `<cycle>`.

Queries (ignore blank lines):
- `section.key`: print the resolved value, or `<missing>` if the key does not exist.
- `section.*`: print `key=value` (resolved) for every key of the section, sorted by key, or `<missing>` if there is no such section.
- `sections`: print all section names that have at least one key, sorted, separated by single spaces.
