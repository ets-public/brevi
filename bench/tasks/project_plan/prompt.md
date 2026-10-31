Plan a project. Read stdin: first a line `workers K` (K at least 1), then one task per line (ignore blank lines): `NAME DAYS` or `NAME DAYS after A, B, ...`. NAME is letters, digits and `_`; DAYS is a whole number of at least 1; the names after `after` are tasks that must finish before this one starts.

First check the input and stop after printing the first problem found, if any: `line N: bad line` (a line not in these forms, including the first line; N counts blank lines), `line N: duplicate NAME` (a task named twice; the later line is reported), then, after reading everything, `unknown NAME` for the first missing dependency (in input order of the tasks that name it, then the order they name it), then `cycle` if the dependencies form a loop.

Otherwise print two plans.

1. `unlimited` and then, with no limit on workers, each task in input order as `  NAME START-END` where START is the earliest day it can start (the latest END of its dependencies, or 0) and END = START + DAYS. Then `length L` (the largest END) and `critical A > B > C`: a longest chain of tasks, from a task starting at 0 to one ending at L, each depending on the one before it with no gap (END of one equals START of the next). Where several tasks could come next in the chain, or start it, pick the one that comes first in the input.

2. `workers K` and then the same lines for a schedule with K workers, made like this: at each day T from 0, first finish the tasks that end at T; then, while a worker is free, start the ready task (all its dependencies finished) with the largest `tail` (its DAYS plus the largest tail of the tasks that depend on it, 0 if none), ties to the first in the input. Print tasks in input order as `  NAME START-END`, then `length L`.

Write the program as 3 files in the folder you are given: `main`, `graph`, `schedule`. `main` is the entry point; each file imports what it needs from the others. `graph` parses and checks the tasks and works out the unlimited plan and the critical chain; `schedule` makes the plan for K workers; `main` reads stdin and prints.
