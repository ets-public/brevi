Write a shop checkout with promotions. Lines of stdin are numbered from 1; blank lines are ignored. Money is decimal with 2 places and is always printed with exactly 2 decimals. SKUs, categories and IDs are words without spaces.

Setup lines (they may appear anywhere):
- `ITEM SKU CATEGORY PRICE`: PRICE is a positive amount with at most 2 decimals. A SKU already defined prints `line N: duplicate item`.
- `PROMO ID percent CATEGORY P`: P% off items of CATEGORY, P a whole number from 1 to 90.
- `PROMO ID bogo SKU`: buy one, get one free on SKU.
- `PROMO ID bundle SKU1 SKU2 PRICE`: one SKU1 plus one SKU2 together cost PRICE (SKU1 and SKU2 differ).
- `COUPON CODE AMOUNT MIN`: AMOUNT off a cart whose subtotal is at least MIN (both amounts with at most 2 decimals). Each code can be redeemed once in total.
A promo naming an undefined SKU prints `line N: unknown item`; a promo ID or coupon code already used prints `line N: duplicate promo` or `line N: duplicate coupon`. These lines are then ignored.

Cart lines:
- `CART ID` opens a cart. If a cart is already open, print `cart OLD abandoned` first.
- `ADD SKU QTY` and `REMOVE SKU QTY` (QTY a whole number from 1) change the open cart. ADD of an undefined SKU prints `line N: unknown item`; REMOVE of more than the cart holds prints `line N: not in cart`.
- `USE CODE` attaches a coupon to the open cart, replacing any earlier one. An unknown code prints `line N: unknown coupon`; a code already redeemed prints `line N: coupon used`.
- `CHECKOUT` prints the receipt (below) and closes the cart.
A cart line with no open cart prints `line N: no cart`. Any other line, or a line with the wrong shape or a bad value, prints `line N: bad line`.

Receipt: if the cart holds nothing, print `cart ID is empty`. Otherwise print `receipt ID`, then for each SKU in the cart, sorted by SKU, `  SKU QTY x PRICE = AMOUNT`. Then the promotions, each unit getting at most one, in this order:
1. Bundles, in the order the promos were defined: the number of pairs is the smaller of the units of SKU1 and SKU2 not yet used by a promo. If there is at least one pair and the bundle saves money, print `  bundle ID xPAIRS -SAVING` (SAVING = PAIRS times (price of SKU1 + price of SKU2 - PRICE)), and those units are used.
2. Bogo promos, in definition order: of the unused units of the SKU, every two units make one free unit. If at least one unit is free, print `  bogo ID SKU -SAVING` (free units times price); the paired units are used (an odd unit left over stays unused).
3. Percent promos, in definition order, but only the first percent promo defined for each category counts: for each SKU of that category in the cart (sorted by SKU) with unused units, print `  percent ID SKU -SAVING`, where SAVING is unused units times price times P / 100, rounded half up to 2 decimals.
Then `subtotal S` (amounts minus savings). If a coupon is attached: when S is at least MIN, print `  coupon CODE -D` with D the smaller of AMOUNT and S, and the code is redeemed; otherwise print `  coupon CODE needs MIN` and the code stays unredeemed. Then `total T`.

At the end of input, an open cart prints `cart ID abandoned`. Finally print `sales X in K carts`: the sum of the totals of the receipts printed, and how many there were.

Write the program as 3 files in the folder you are given: `main`, `catalog`, `pricing`. `main` is the entry point; each file imports what it needs from the others. `catalog` reads and checks the ITEM, PROMO and COUPON lines and holds them; `pricing` works out a receipt from a cart and the catalog; `main` reads stdin, keeps the open cart and the sales total, and prints.
