Keep stock for a warehouse using first-in first-out costing. Read lines from stdin (ignore blank lines), each `DATE OP SKU QTY [PRICE]` with DATE as `YYYY-MM-DD`, QTY a whole number of at least 1 and PRICE a decimal with at most 2 decimals:

- `RECEIVE SKU QTY COST`: a new lot of QTY units, each costing COST.
- `SHIP SKU QTY PRICE`: sell QTY units at PRICE each. Units leave from the oldest lots first; the cost of goods sold is the sum of the costs of the units that left, and the profit is the revenue (QTY times PRICE) minus that cost.
- `LOSS SKU QTY`: QTY units are lost, taken from the oldest lots first; their cost counts as a loss.

All amounts are exact. Errors print one line, change nothing, and processing goes on: `line N: bad line` for a line not in one of these forms, `line N: date out of order` for a date earlier than the latest date accepted so far, `line N: short SKU need Q have H` when a SHIP or LOSS asks for more units than are in stock. N is the line number in the input, counting blank lines.

After the last line print, for each SKU that ever had a lot, in name order: `SKU on_hand Q value V cogs C revenue R profit P loss L` (V is the cost of the units still in stock; amounts with 2 decimals). Then `months` and, for every month with a SHIP or LOSS, in order, `  YYYY-MM profit P loss L`, where profit and loss add up over all SKUs. Then `total profit P loss L net N` with N = profit minus loss.
