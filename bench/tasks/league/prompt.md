Keep a football league table. Each line of stdin is a match result `HOME AWAY HG-AG`, for example `Lions Tigers 2-1` (team names are words; goals are whole numbers). Ignore blank lines. Teams join the league the first time they appear.

A win is 3 points, a draw 1, a loss 0. Errors print one line and the result is ignored: `line N: bad line` for a line not in that form (N counts every input line, blank ones included), `line N: same team` when HOME and AWAY are the same, and `line N: already played` when the same HOME and AWAY (in that order) have already played.

After the last line print the table. Teams are ordered by points, then goal difference (goals for minus against), then goals for, all highest first. Teams still level on all three are ordered by the points they won in the matches between themselves only (highest first), and then by name (string order). Print a header line `Pos Team P W D L GF GA GD Pts` and one line per team: position (1, 2, 3, ... with no sharing), name, played, won, drawn, lost, goals for, goals against, goal difference (with a `+` when above 0) and points, separated by single spaces.

Then print `most goals: TEAM (N)` for the team with the most goals for and `best defence: TEAM (N)` for the team with the fewest goals against, taking the first in table order when tied. With no valid results print only `no matches`.
