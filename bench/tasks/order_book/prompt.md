Run a stock exchange order book. Read commands from stdin, one per line (ignore blank lines):

- `BUY ID QTY PRICE` and `SELL ID QTY PRICE`: a limit order. ID is a word, QTY a whole number of at least 1, PRICE a positive decimal with at most 2 decimals (`12`, `12.5`, `12.50`).
- `MBUY ID QTY` and `MSELL ID QTY`: a market order.
- `CANCEL ID`: remove the resting order ID from the book; print `CANCELLED ID QTY` with the quantity that was still resting.
- `BOOK`: print the book.

Matching: an incoming buy trades with resting sells, cheapest price first and, at the same price, the oldest order first; a limit buy only trades with sells priced at or below its own price, a market buy with any sell. Selling is the mirror image (highest bid first, a limit sell only trades at or above its own price). Each fill prints `TRADE BUYID SELLID QTY PRICE`, at the resting order's price with 2 decimals. What is left of a limit order then rests in the book (keeping its arrival time); what is left of a market order is dropped and prints `DROPPED ID QTY`.

`BOOK` prints `asks` and then one line per sell price, lowest first: two spaces, the price (2 decimals), a space and the total resting quantity at that price; then `bids` and the buy prices the same way, highest first. A side with no orders prints two spaces and `-`.

Errors print one line and change nothing: `ERR line N: bad command` for a line that is not exactly one of the forms above (N is the line number in the input, counting blank lines), `ERR line N: duplicate ID` when a new order uses an ID that any earlier accepted order used, `ERR line N: unknown ID` when CANCEL names an order that is not resting.

After the last line print `volume Q value V`: the total traded quantity and the sum of quantity times price over all trades (2 decimals).
