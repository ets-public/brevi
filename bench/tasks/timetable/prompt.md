Stdin has two sections separated by a line `---`.

The first section is a train timetable, one trip per line: `TRAIN STATION HH:MM STATION HH:MM ...`, the stations a train calls at with its departure time from each (the last time is its arrival). Times are within one day.

The second section has queries `FROM TO HH:MM`: a traveller at station FROM at that time wants to reach TO as early as possible. Changing trains at a station needs at least 5 minutes (arrival time + 5 <= departure time), except staying on the same train. A traveller can board a train at FROM at or after the query time.

For each query print `FROM TO HH:MM via T1>T2...` with the earliest arrival time and the trains used in order (if several routes arrive equally early, prefer the fewest trains, then the alphabetically smallest train list joined by `>`), or `FROM TO unreachable`. If FROM equals TO, print `FROM TO HH:MM via -` with the query time.
