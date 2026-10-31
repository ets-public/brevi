Implement an in-memory key-value store with nested transactions. Each stdin line is one command; print output only where stated.

- `SET k v`: set key k to string v.
- `GET k`: print the value, or `NULL` if k is not set.
- `DEL k`: remove k. Print `NOT FOUND` if it was not set.
- `INCR k n`: add the integer n to k's value (a missing key counts as 0) and print the new value. If the current value is not an integer, print `ERR not an integer` and change nothing.
- `COUNT v`: print how many keys currently have the value v.
- `KEYS p`: print all keys starting with the prefix p, sorted, separated by single spaces, or `(empty)` if there are none.
- `BEGIN`: open a transaction. Transactions can be nested.
- `ROLLBACK`: undo every change made since the most recent open BEGIN and close that transaction. Print `NO TRANSACTION` if none is open.
- `COMMIT`: close all open transactions, keeping their changes. Print `NO TRANSACTION` if none is open.
- Anything else: print `ERR unknown command`.

Commands are uppercase and arguments have no spaces. Ignore blank lines.
