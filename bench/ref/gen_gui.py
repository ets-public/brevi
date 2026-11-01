# Held-out GUI tasks (written 2026-11-01 for Adam's "programs with a graphical interface" ask).
# The model writes an ordinary tkinter program. Tests drive it headlessly: under CPython through the stand-in
# in bench/tkshim (PYTHONPATH), built by Brevi through runtime/brevi_tk's headless mode. Both read the same
# actions from stdin and print the window as text, so the expected outputs come from running the reference
# programs in ref/gui/ under the stand-in.
# Writes tasks/<name>/{prompt.md,gui,tests/N.in,N.out}.
import os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
SHIM = os.path.join(HERE, "..", "tkshim")

HEADLESS = r"""
How it is tested: the tests run your program without a screen. `mainloop()` then reads test actions from stdin (you do not handle them) and `show` prints every placed widget, one per line, indented under its parent: `window "TITLE"`, `Label "TEXT"`, `Button "TEXT"`, `Entry "TEXT"`, `Checkbutton "TEXT" [x]`, `Listbox [a | b*]` (`*` marks the selection), `Canvas WxH` followed by its items (`line 0 100 300 100 width=2`, `text 50 50 fill=blue text=X`: the kind, the coordinates, then the options you passed, sorted by name), `Frame`. Children of a container are shown in the order they were created, except grid cells, which are shown by row, then column. Dialogs (`messagebox`) print `[showinfo] TITLE: MESSAGE` (the function name, the title, the message); the answer to a question dialog is supplied by the test. `after` timers run on a test clock. Use only `tkinter` (with `messagebox`) and the standard library; widgets show exactly the texts and options given here.
"""

T = {}
def task(name, prompt, inputs):
    T[name] = (prompt.strip() + "\n" + HEADLESS.rstrip(), inputs)

task("todo_app", r"""
Write a to-do list app with tkinter. The window title is `Todo`. The window holds, packed in this order (all with plain `.pack()` unless said otherwise):
1. an Entry for a new task;
2. a Button `Add`;
3. a Checkbutton `Hide done` (off at the start);
4. a Listbox of the tasks;
5. a Button `Done` and a Button `Clear done`, both packed with `side="left"`;
6. a Label with the status line.

Each task has a text and is open or done. The list shows the tasks in the order they were added, each as `[ ] TEXT` when open or `[x] TEXT` when done; while `Hide done` is on, done tasks are left out of the list. The status line is `K open, M done` (counting all tasks, also hidden ones) and is kept up to date.

- `Add` (and the Return key in the Entry) adds the Entry's text, with spaces at both ends removed, as an open task, then empties the Entry. If the text is empty, show `messagebox.showwarning("Todo", "Nothing to add")`; if an open task with the same text exists, show `messagebox.showwarning("Todo", "Already on the list")`; in both cases nothing changes (the Entry keeps its text).
- `Done` switches the selected task between open and done (the selection is a line of the list as shown). With no line selected, show `messagebox.showinfo("Todo", "Select a task first")`.
- `Clear done` asks `messagebox.askyesno("Todo", "Remove K done tasks?")` (K the number of done tasks) and, on yes, removes every done task. With no done task, show `messagebox.showinfo("Todo", "No done tasks")` instead.
- Toggling `Hide done` updates the list at once.
- The list is rebuilt after every change, so no line stays selected.
""", [
"""show
click Add
type 1 buy milk
click Add
type 1   walk dog
key 1 Return
type 1 buy milk
key 1 Return
click Done
select 1 0
click Done
show
click Hide done
show
select 1 0
click Done
click Hide done
answer no
click Clear done
show
click Clear done
show
""",
"""type 1 a
click Add
type 1 b
click Add
type 1 c
click Add
select 1 1
click Done
click Clear done
show
type 1 b
click Add
select 1 1
click Done
show
click Hide done
select 1 0
click Done
show
click Hide done
show
""",
"""click Clear done
click Done
type 1 x
key 1 Return
type 1 x
key 1 Return
select 1 0
click Done
type 1 x
key 1 Return
show
click Hide done
answer yes
click Clear done
show
click Hide done
show
""",
"""type 1 one
click Add
type 1 two
click Add
click Hide done
select 1 0
click Done
show
select 1 0
click Done
show
click Hide done
show
""",
])

task("calc_app", r"""
Write a pocket calculator for whole numbers with tkinter. The window title is `Calc`. Everything is placed with `.grid()`:
- row 0, column 0 (columnspan 4): a Label, the display;
- rows 1 to 4: Buttons, laid out as
  ```
  7 8 9 /
  4 5 6 *
  1 2 3 -
  C 0 = +
  ```
  (row 1 is `7 8 9 /`, columns 0 to 3);
- row 5, column 0 (columnspan 4): a Listbox, the history.

Keys work too: bind `<Key>` on the root window so that typing a digit or one of `+ - * / =` presses that button (use the event's `char`), and bind `<Return>` to `=` and `<Escape>` to `C`.

Behaviour (as a simple calculator that works left to right, with no precedence):
- The display starts as `0`. It shows the number being typed, or the last result.
- A digit starts a new number when the last key was an operator or `=` (or at the start, or after `C`), otherwise it is added to the end of the number being typed; a number never starts with an extra `0` (typing `0` then `7` gives `7`).
- An operator after a number: if an operator is waiting, first work out `previous op number` as if `=` had been pressed (see below, but the history line continues from the result); then remember the new operator. An operator right after another operator replaces it. An operator right after `=` uses the result as its first number.
- `=` works out the waiting operation, shows the result, and adds a history line `A op B=R` (for a chain such as `1 + 2 * 3 =` the lines are `1+2=3` and `3*3=9`). `=` with no waiting operation, or right after an operator, does nothing.
- `/` divides and rounds toward zero (`-7 / 2` is `-3`; results can be negative through `-`). Dividing by zero shows `Error` and adds the history line `A/0=Error`; after that every key except `C` is ignored.
- `C` clears everything except the history.
""", [
"""show
click 1
click 2
click +
click 3
click =
show
click *
click 4
click -
click 6
click =
key 0 5
key 0 /
key 0 0
key 0 Return
show
click 7
key 0 Escape
key 0 9
key 0 0
key 0 /
key 0 7
key 0 =
click 0
click 0
click 3
show
""",
"""click 3
click -
click 1
click 0
click =
click /
click 2
click =
click *
click *
click -
click 4
click =
click =
show
""",
"""click 0
click 0
click 7
click +
click =
click 1
click =
key 0 +
key 0 2
key 0 *
key 0 3
key 0 =
show
key 0 x
key 0 Escape
show
""",
"""click 9
click /
click 0
click =
click 5
click +
click =
show
click C
click 8
click /
click 3
click =
show
""",
])

task("tictactoe_app", r"""
Write tic-tac-toe for two players with tkinter. The window title is `Tic-tac-toe`. It holds, packed in this order: a Label (the status line); a Canvas of width 300 and height 300; a Frame holding a Button `New game` and a Label (the score), both packed with `side="left"`.

The board is 3 by 3 cells of 100 by 100 pixels; cell 0 is top left, cell 8 bottom right (`cell = (y // 100) * 3 + x // 100`).

- A new game clears the canvas (`delete("all")`) and draws the grid as four lines, in this order: `create_line(100, 0, 100, 300, width=2)`, `create_line(0, 100, 300, 100, width=2)`, `create_line(200, 0, 200, 300, width=2)`, `create_line(0, 200, 300, 200, width=2)`. The first game starts with X; each new game starts with the player who did not start the game before. The status line shows `X to move` (or O).
- A click (`<Button-1>` on the canvas) on an empty cell puts the mark of the player to move there, as `create_text(CX, CY, text="X", fill="blue")` (O is `fill="red"`), where CX, CY is the cell's centre (50, 150 or 250). Then:
  - if that player has three in a row (a row, a column or a diagonal), draw `create_line(AX, AY, BX, BY, fill="green", width=4)` from the centre of the first to the centre of the last cell of that line, where the lines are checked in the order rows (top to bottom), columns (left to right), then the diagonal from cell 0 to 8, then from cell 2 to 6, and the first complete one is used; the game is won;
  - else, if all cells are full, the game is a draw;
  - else the other player is to move and the status line says so.
- A click on a taken cell sets the status line to `Taken, X to move` (the player to move) and changes nothing else. Clicks after a game has ended, and clicks outside the 300 by 300 board, are ignored.
- When a game ends, the score Label `X 1 - O 0 - draws 0` is updated (it starts at all zeros and is always shown), and the status line counts down: `X wins! New game in 3` (or `Draw! New game in 3`), then after 1000 ms `... New game in 2`, then `... in 1`, and after another 1000 ms a new game starts by itself. Use `after` for this.
- `New game` starts a new game at once (stopping any countdown, with `after_cancel`). It does not change the score.
""", [
"""show
tap 1 50 50
tap 1 60 60
tap 1 150 50
tap 1 50 150
tap 1 250 50
tap 1 50 250
show
tap 1 150 150
wait 1000
show
wait 1999
show
wait 1
show
""",
"""tap 1 150 150
tap 1 50 50
tap 1 250 50
tap 1 50 250
tap 1 50 150
tap 1 250 150
tap 1 150 50
tap 1 150 250
tap 1 250 250
show
click New game
show
wait 5000
show
""",
"""tap 1 250 50
tap 1 50 50
tap 1 150 150
tap 1 50 150
tap 1 50 250
show
tap 1 250 250
wait 3000
tap 1 150 150
tap 1 50 50
tap 1 150 50
tap 1 50 150
tap 1 150 250
show
click New game
tap 1 299 0
tap 1 300 10
show
""",
])

for name, (prompt, inputs) in T.items():
    d = os.path.join(HERE, "..", "tasks", name)
    os.makedirs(f"{d}/tests", exist_ok=True)
    open(f"{d}/prompt.md", "w").write(prompt + "\n")
    open(f"{d}/gui", "w").write("tkinter\n")
    ref = os.path.join(HERE, "gui", name.replace("_app", "") + ".py")
    for i, inp in enumerate(inputs):
        open(f"{d}/tests/{i}.in", "w").write(inp)
        r = subprocess.run([sys.executable, ref], input=inp, capture_output=True, text=True, env={**os.environ, "PYTHONPATH": SHIM}, timeout=30)
        if r.returncode:
            raise SystemExit(f"{name} test {i}: {r.stderr}")
        open(f"{d}/tests/{i}.out", "w").write(r.stdout)
print(len(T), "tasks")
