Read a CSV sales file from stdin and print a report. The first line is the header `date,region,product,qty,unit_price`. Fields may be quoted with double quotes (a quoted field can contain commas, and `""` inside quotes is one quote character). Ignore blank lines.

A data row is valid when it has exactly 5 fields, `date` is `YYYY-MM-DD`, `qty` is a whole number (digits only, at least 1 digit) and `unit_price` is digits with an optional `.` and 1 or 2 digits. Revenue of a row is `qty * unit_price`, computed exactly.

Print, in this order:
- `rows N` (valid rows) and then `skipped M` (invalid data rows).
- `regions`, then one line per region: two spaces, the region, a space and its total revenue with exactly 2 decimals. Order by revenue, highest first; ties by region name.
- `top_product NAME QTY`: the product with the largest total qty (ties: alphabetically first name).
- `months`, then one line per month `  YYYY-MM REVENUE` (2 decimals), in month order.

If there are no valid rows, print only the `rows` and `skipped` lines.
