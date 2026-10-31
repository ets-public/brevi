Write a quiz grader. Stdin (lines numbered from 1) has a line `KEY`, then answer-key lines, then a line `ANSWERS`, then student lines. Blank lines are ignored everywhere. Lines before `KEY`, a second `KEY` or `ANSWERS`, or a missing section are not tested.

Key lines are `ID TYPE POINTS SPEC`, where ID is `Q` followed by a whole number, POINTS a whole number from 1 to 100, and:
- `choice POINTS X`: one letter A-E. Full points for exactly that letter.
- `multi POINTS LETTERS`: two or more different letters A-E, written together (`ACD`). A student's answer is a set of letters (repeats count once). Full points for exactly the set; otherwise points * (right letters chosen - wrong letters chosen) / (letters in the key), rounded down, and never below 0.
- `number POINTS VALUE TOL`: VALUE and TOL decimals (TOL not negative). Full points if the answer is a decimal within TOL of VALUE (inclusive).
- `text POINTS WORD|WORD|...`: one or more words separated by `|`. Full points if the answer equals one of them ignoring case.
A key line with any other shape, or an ID used before, prints `key line N: bad question` and that line is ignored.

Student lines are `NAME ID=ANSWER ID=ANSWER ...` (zero or more answers; an answer has no spaces). A NAME used before prints `line N: duplicate student` and the line is ignored. Within an accepted line: a token without `=` or with an empty answer prints `line N: bad answer TOKEN`; an ID not in the key prints `line N: unknown question ID`; a second answer to the same question prints `line N: answered twice ID`. These answers are skipped; the rest of the line still counts. Unanswered questions score 0, and so does an answer of the wrong form (a `number` answer that is not a decimal, `choice` or `multi` answers with letters outside A-E).

Output, in this order:
1. Messages, in line order, as they occur.
2. For each student in input order: `NAME SCORE/MAX PCT%`, where MAX is the sum of all key points and PCT is SCORE * 100 / MAX rounded down.
3. For each key question in key order: `ID avg X` with X the mean score of the students, 2 decimals (half up).
4. `top: NAMES (SCORE)`: the highest score and every student with it, names in input order separated by `, `.
If there are no students, print the messages and then only `no students`. If the key has no valid question, print the messages and then only `empty key`.

Write the program as 3 files in the folder you are given: `main`, `key`, `grade`. `main` is the entry point; each file imports what it needs from the others. `key` reads and checks the answer key and grades one answer against a question; `grade` keeps the students' scores and works out totals, averages and the top score; `main` splits stdin into its sections, calls them and prints.
