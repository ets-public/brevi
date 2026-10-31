Find shortest train routes. Stdin first has edge lines `A B MINUTES` (an undirected track between stations A and B; names have no spaces; MINUTES is a positive integer; a repeated pair keeps the smaller time). A blank line ends the edges. Every later non-blank line is a query `FROM TO`.

For each query print one line:
- If FROM or TO is not a station in any edge: `unknown station NAME` (check FROM first).
- If FROM equals TO: `FROM (0 min)`.
- If TO cannot be reached: `no route FROM TO`.
- Otherwise the stations of the fastest route joined by ` -> `, then ` (N min)`. If several routes are equally fast, take the one with the fewest stations; if still tied, the one whose list of station names is smallest when compared name by name.
