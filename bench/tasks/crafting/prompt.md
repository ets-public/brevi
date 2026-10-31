Simulate a crafting workshop. Read commands from stdin, one per line (ignore blank lines):

- `STOCK ITEM QTY`: add QTY (a whole number, at least 1) of ITEM to the stock.
- `RECIPE ITEM N: ING QTY, ING QTY, ...`: one batch of the recipe makes N (at least 1) of ITEM from the listed ingredients (one or more, each with a whole quantity of at least 1, each listed once, none equal to ITEM). Item names are words.
- `CRAFT ITEM QTY`: make QTY of ITEM.
- `SHOW`: print `stock:` and then, in name order, two spaces, the item, a space and the quantity, for every item with more than 0; or `stock: empty`.

CRAFT always runs ITEM's recipe, enough batches to make at least QTY (batches = QTY divided by N, rounded up). For each ingredient, in the order listed, it needs batches times the listed quantity: it takes as much as it can from stock, and if that is not enough it crafts the rest the same way (using that ingredient's recipe, which first takes its own ingredients from stock), or, for an item with no recipe, records a shortage. Everything made beyond what was needed goes into stock; the QTY of ITEM is added to stock too.

If there is any shortage, nothing changes and it prints `line N: short ITEM Q, ITEM Q` with every short item and its total missing quantity, in name order. Otherwise it prints `crafted QTY ITEM (ITEM B, ITEM B)` listing every recipe that ran and its total batches, in name order.

Errors print one line and change nothing: `line N: bad line` for a line not exactly in one of the forms above (N counts every input line, blank ones included), `line N: duplicate recipe` for a second recipe for the same item, `line N: cycle` when the new recipe would make an item need itself (directly or through other recipes), and `line N: no recipe` when CRAFT names an item without a recipe.
