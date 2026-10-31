Write a double-entry bookkeeping checker. Read lines from stdin, numbered from 1.

- `ACCOUNT NAME TYPE` opens an account. TYPE is one of `asset`, `liability`, `equity`, `income`, `expense`. A second ACCOUNT line with a name already opened prints `line N: duplicate account` and is ignored.
- `TX DATE DESCRIPTION...` starts a transaction. DATE is `YYYY-MM-DD` and must be a real date; the description is one or more words.
- A line that starts with two spaces is a posting of the transaction above it: `  NAME AMOUNT`, where AMOUNT is a signed decimal with at most 2 decimal places (`120`, `-45.5`, `+3.25`). Positive amounts are debits, negative amounts are credits.
- `REPORT` prints the trial balance (below).
- Blank lines are ignored and end nothing. Any other line, or a line of the kinds above with the wrong number of words or a bad value, prints `line N: bad line` and is ignored. A posting line with no transaction open (before any TX, or after a line that is not a posting) is a bad line too.

A transaction ends at the next line that is not a posting (or at the end of input), and is then checked, in this order:
1. If its TX line was a bad line, the transaction and its postings are ignored silently.
2. A posting line that is a bad line, or names an account not opened yet (`line N: unknown account NAME`), is reported when read; the transaction is then discarded with no further message.
3. If its date is before the date of the last accepted transaction, print `line N: date goes back` (N = the TX line) and discard it.
4. With fewer than two postings, print `tx on line N: needs two postings` and discard it.
5. If the amounts do not add up to zero, print `tx on line N: unbalanced by X`, where X is the sum with 2 decimals and a sign (`+12.50`, `-0.01`), and discard it.
Otherwise the transaction is accepted and its postings are added to the account balances.

A REPORT line (which also ends an open transaction, checked first) prints `trial balance` and then one line per opened account: `NAME TYPE BALANCE`. Accounts are ordered by type (asset, liability, equity, income, expense), then by name. BALANCE has 2 decimals: for asset and expense accounts it is the sum of their postings; for the other types it is minus that sum, so a normal balance is positive either way. Then print `debits D credits C`: the totals of all positive and of all negative accepted amounts (C without its sign), with 2 decimals.

At the end print `K transactions accepted, R rejected`, where R counts transactions discarded with a message or for a bad posting (not those with a bad TX line).
