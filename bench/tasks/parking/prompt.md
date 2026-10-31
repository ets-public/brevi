Run a car park. The first line of stdin is `spaces car C moto M van V` (whole numbers, 0 or more). Every later line is `DATE TIME COMMAND` with DATE as `YYYY-MM-DD` and TIME as `HH:MM` (24-hour), or blank (ignore it). Commands:

- `IN PLATE TYPE`: a vehicle arrives. PLATE is a word, TYPE is `car`, `moto` or `van`.
- `OUT PLATE`: it leaves and pays.
- `REPORT`: print the state.

Spaces are named by type letter and number: `C1`..`CC`, `M1`..`MM`, `V1`..`VV`. A car takes the lowest-numbered free car space; a van the lowest free van space; a moto the lowest free moto space, or if none is free, the lowest free car space. On success print `IN PLATE SPACE`; if there is no space for it print `FULL PLATE` (it does not enter).

On OUT, the fee depends on TYPE (not the space used): car 2.50 per started hour with a cap of 20.00 per day, moto 1.20 per hour with a cap of 8.00, van 4.00 per hour with a cap of 30.00. For a stay of D minutes: a stay of 15 minutes or less is free; otherwise split the stay into whole days of 24 hours and a rest; the fee is days times the cap, plus the rest's started hours times the rate but at most the cap. Print `OUT PLATE HhMMm fee F` with the stay as hours and two-digit minutes (`26h05m`) and the fee with 2 decimals.

`REPORT` prints `occupied car X/C moto Y/M van Z/V revenue R`, counting the vehicles in each kind of space (a moto in a car space counts as car), and the fees taken so far.

Errors print one line and change nothing: `line N: bad line` for a line not exactly in one of the forms above, with a real date and time (N counts every input line, blank ones included), `line N: time goes back` when the time is before the previous accepted line's time, `line N: already inside` for IN of a plate that is inside, and `line N: not inside` for OUT of a plate that is not.

After the last line print `revenue R from K exits`.

Write the program as 3 files in the folder you are given: `main`, `garage`, `billing`. `main` is the entry point; each file imports what it needs from the others. `garage` tracks the spaces and the vehicles inside; `billing` works out stay lengths and fees; `main` reads and checks the lines, keeps the clock and revenue, and prints.
