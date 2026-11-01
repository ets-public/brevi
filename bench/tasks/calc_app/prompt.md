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

How it is tested: the tests run your program without a screen. `mainloop()` then reads test actions from stdin (you do not handle them) and `show` prints every placed widget, one per line, indented under its parent: `window "TITLE"`, `Label "TEXT"`, `Button "TEXT"`, `Entry "TEXT"`, `Checkbutton "TEXT" [x]`, `Listbox [a | b*]` (`*` marks the selection), `Canvas WxH` followed by its items (`line 0 100 300 100 width=2`, `text 50 50 fill=blue text=X`: the kind, the coordinates, then the options you passed, sorted by name), `Frame`. Children of a container are shown in the order they were created, except grid cells, which are shown by row, then column. Dialogs (`messagebox`) print `[showinfo] TITLE: MESSAGE` (the function name, the title, the message); the answer to a question dialog is supplied by the test. `after` timers run on a test clock. Use only `tkinter` (with `messagebox`) and the standard library; widgets show exactly the texts and options given here.
