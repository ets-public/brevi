Simulate a bank with several tellers. Lines of stdin are numbered from 1; blank lines are ignored. The first non-blank line is `TELLERS K` (K at least 1; otherwise print `line N: bad line` and stop). Every later line is a customer: `NAME ARRIVAL SERVICE` with optional words after it, each `vip` or `patience=P`. ARRIVAL is a whole number of minutes from 0, SERVICE and P whole numbers from 1. A line with a different shape or a bad value prints `line N: bad line`; a NAME already used prints `line N: duplicate`; both are ignored. Customers may be listed in any order of arrival.

Tellers are numbered 1 to K. Time moves in whole minutes. At each minute T, in this order:
1. Tellers whose customer is done at T become free.
2. Customers arriving at T join the queue.
3. While a teller is free and the queue is not empty, the lowest-numbered free teller takes the first customer in the queue and prints `T NAME starts at teller X, waited W`. The queue puts vip customers before the others; within each group, earlier arrival first, then the order of the input lines.
4. A customer with patience P who is still waiting at minute ARRIVAL + P leaves and prints `T NAME leaves` (in queue order if several leave at once).

A customer served from minute S is done at S + SERVICE. When everyone has been served or has left, print `served S, left L, average wait A` (A = the average wait of served customers, 2 decimals, or `-` if none), then `teller X: busy B` for each teller (B = total minutes serving), then `done at T`, the last minute a customer was done (0 if none).
