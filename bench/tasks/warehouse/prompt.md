Write a warehouse stock keeper. Read commands from stdin, one per line, numbered from 1. Blank lines are ignored.

- `DAY YYYY-MM-DD` sets today's date. It must be a real date and not before the current one (`line N: date goes back`, ignored). When the date is set, every lot whose expiry is before the new date is thrown away: print `discard SKU QTY EXPIRY` for each, ordered by SKU, then expiry, then the order received.
- `RECV SKU QTY EXPIRY` receives a lot: a positive whole quantity with an expiry date. A lot already expired (expiry before today) prints `line N: expired on arrival` and is not stored. After storing a lot, waiting backorders for that SKU are filled (below).
- `ORDER ID SKU QTY` ships QTY units of SKU. Units come from lots with the earliest expiry first (ties: the lot received first). If there is not enough, ship what there is: print `ORDER ID: shipped S of Q`, and the missing units become a backorder. If everything ships, print `ORDER ID: shipped Q`. An order ID used before prints `line N: duplicate order` and is ignored.
- `STOCK` prints one line per SKU that has units: `SKU QTY in L lots` (L = lots with units left), ordered by SKU; or `stock empty`.
- Before the first DAY line, RECV and ORDER print `line N: no date` and are ignored.
- Anything else, or a command with the wrong number of words, a quantity that is not a positive whole number, or a bad date, prints `line N: bad line`.

Backorders are filled in the order they were created, whenever a RECV stores units of their SKU, using the same lot order as orders: print `BACKORDER ID: shipped X, R left` or, when nothing is left, `BACKORDER ID: done`. A backorder that gets no units prints nothing.

At the end print `shipped T units, backordered B units`, where T counts every unit shipped (orders and backorders) and B the units still waiting.

Write the program as 3 files in the folder you are given: `main`, `stock`, `orders`. `main` is the entry point; each file imports what it needs from the others. `stock` holds the lots and picks units in expiry order; `orders` tracks order IDs and backorders; `main` reads and checks the commands, keeps the date and totals, and prints.
