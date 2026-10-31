Stdin is one JSON object: `{"products": {...}, "customers": {...}, "orders": [...]}`.
- `products` maps a SKU to `{"name": str, "price": "12.50", "category": str}` (price is a decimal string).
- `customers` maps a customer id to `{"name": str, "tier": "gold" | "silver" | "basic", "country": str}`.
- `orders` is a list of `{"id": str, "customer": id, "lines": [{"sku": str, "qty": int}]}`.

For each order, in input order, compute an invoice:
- Each line's amount is price times qty. Unknown SKUs make the whole order invalid.
- Discount: gold customers get 10% off the subtotal, silver 5%, basic nothing. Additionally, any category with 3 or more units in the order (qty summed over its lines) gets an extra 5% off that category's lines, applied before the tier discount. Round the discount amounts (each one) to cents, half to even.
- Tax: 20% of the discounted total if the country is "UK", 8.25% if "US", otherwise 0. Round tax to cents, half to even.
- An unknown customer also makes the order invalid.

Print a JSON array (compact, no spaces) of invoices, each `{"id":...,"customer":NAME,"subtotal":"...","discount":"...","tax":"...","total":"..."}` with money as strings with exactly 2 decimals, where discount is the sum of all discounts and total = subtotal - discount + tax. An invalid order prints `{"id":...,"error":"unknown sku SKU"}` or `{"id":...,"error":"unknown customer ID"}` (customer is checked first; for skus name the first unknown one).
Then print a second line: the total revenue (sum of valid totals) as a string with 2 decimals, in the form `revenue 123.45`.
