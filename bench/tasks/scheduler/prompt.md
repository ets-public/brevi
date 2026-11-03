Plan jobs on a pool of workers. Lines of stdin are numbered from 1; blank lines are ignored. Job IDs are letters, digits, `_` and `-`.

- `WORKERS K` sets the number of workers (K at least 1) for later runs. Before any WORKERS line there is 1 worker.
- `JOB ID DURATION [after=ID,ID,...] [prio=P]` defines a job. DURATION is a whole number from 1; P is a whole number that may be negative (default 0). The two options may come in either order, each at most once; `after=` lists one or more IDs this job must wait for (they may be defined later). A job ID already defined prints `line N: duplicate job` and the line is ignored.
- `RUN` plans all jobs defined so far (it may appear several times; later runs include later jobs).
Anything else, or a line with a bad value, prints `line N: bad line`.

A RUN first checks the jobs:
1. If any job waits for an ID that is not defined, print `job X needs unknown Y` for each such pair, by job ID (sorted as text) and then in the order Y is listed, and skip this run.
2. Otherwise, if some jobs wait on each other in a cycle (including a job waiting for itself), print `cycle among: A,B,...` with every job that is on a cycle, sorted, and skip this run.

Otherwise print `plan with K workers`, then simulate. Workers are numbered 1 to K. Time starts at 0. At each moment, first every job ending then finishes; then, while a worker is free and a job is ready (not started, and every job it waits for has finished), the lowest-numbered free worker starts the ready job with the highest priority, then the longest duration, then the smallest ID as text, printing `  T start ID on wX`. Time then moves on to the next moment a job ends. When all jobs are done print `makespan M` (the time the last job ends; 0 if there are no jobs).

Then print `critical path A > B > C (L)`: the chain of jobs with the largest total duration L, where each job in the chain waits for the one before it. On a tie, take the chain whose list of IDs comes first when compared ID by ID as text (a chain that is a prefix of another comes first). With no jobs print `critical path none`.

Write the program as 3 files in the folder you are given: `main`, `jobs`, `plan`. `main` is the entry point; each file imports what it needs from the others. `jobs` reads and checks the WORKERS and JOB lines and holds the job table; `plan` checks for unknown jobs and cycles, simulates the workers and finds the critical path; `main` reads stdin and prints.
