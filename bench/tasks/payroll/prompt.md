Run a small monthly payroll. Read commands from stdin, one per line (ignore blank lines):

- `EMP ID NAME hourly RATE` or `EMP ID NAME salary ANNUAL`: add an employee. ID and NAME are words; RATE (per hour) and ANNUAL (per year) are positive decimals with at most 2 decimals.
- `HOURS ID DATE H`: hours worked by an hourly employee on DATE (`YYYY-MM-DD`, a real date). H is a decimal with at most 2 decimals, more than 0. Several HOURS lines for the same day add up.
- `BONUS ID YYYY-MM AMOUNT`: a one-off bonus paid in that month, for any employee. AMOUNT is a positive decimal with at most 2 decimals.
- `RUN YYYY-MM`: print the payroll for that month.

Pay for a month:
- salaried: ANNUAL / 12.
- hourly, per day worked in that month: on Saturday and Sunday every hour is paid at 2 times RATE; on other days the first 8 hours of the day are paid at RATE and the hours above 8 at 1.5 times RATE.
- gross = that pay plus the month's bonuses, rounded half up to cents.
- tax on gross: 0% on the first 1000, 20% on the part from 1000 to 4000, 40% on the part above 4000, rounded half up to cents. net = gross - tax.

`RUN` prints `payroll YYYY-MM`, then one line per employee in ID order (string order): two spaces, `ID NAME gross G tax T net N`, then `  total gross G tax T net N` summing the lines above. All money with 2 decimals. Employees with nothing to pay are listed with 0.00. With no employees print just the header and the total line.

Errors print one line and change nothing: `line N: bad line` for a line not exactly in one of the forms above (N counts every input line, blank ones included), `line N: duplicate ID`, `line N: unknown ID`, `line N: not hourly` for HOURS on a salaried employee, and `line N: too many hours` when the day's total for that employee would go above 24.
