Write a journey planner for a timetable read from stdin (lines numbered from 1). Times are `HH:MM` on one day (00:00 to 23:59).

- `TRAIN ID STOP TIME STOP TIME ...` defines a train with at least two stops: it is at each stop at the given time, in order. Times must strictly increase and a stop may not appear twice; otherwise print `line N: bad train`. A second train with an ID already used prints `line N: duplicate train`. Bad and duplicate trains are ignored.
- `TRIP FROM TO TIME` asks for the earliest arrival at TO for a traveller standing at FROM at TIME. It only uses trains defined above it.
- Blank lines are ignored; anything else, or a TRAIN or TRIP line with the wrong shape or a bad time, prints `line N: bad line`.

For a trip: a traveller can board a train at any of its stops at or after the moment they are there, ride it, and get off at any later stop of that train. Changing to a different train at a stop needs at least 2 minutes between arriving and the next train's time at that stop; at the starting stop they can board any train whose time there is at or after TIME. Staying on the same train is not a change.

Print `TRIP FROM->TO: arrive HH:MM via A,B,...` where the list is the trains used, in order, one entry per ride. Choose the plan with the earliest arrival; among those, the fewest rides; among those, the one whose list of train IDs is smallest comparing ID by ID as text. If FROM or TO is not a stop of any train defined so far, print `TRIP FROM->TO: unknown stop NAME` (FROM is checked first). If FROM equals TO, print `TRIP FROM->TO: already there`. If no plan reaches TO, print `TRIP FROM->TO: no route`.

At the end print `T trains, Q trips`, counting accepted trains and all well-formed TRIP lines.
