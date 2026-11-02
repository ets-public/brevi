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

How it is tested: the tests run your program without a screen. `mainloop()` then reads test actions from stdin (you do not handle them) and `show` prints every placed widget, one per line, indented under its parent: `window "TITLE"`, `Label "TEXT"`, `Button "TEXT"`, `Entry "TEXT"`, `Checkbutton "TEXT" [x]`, `Listbox [a | b*]` (`*` marks the selection), `Canvas WxH` followed by its items (`line 0 100 300 100 width=2`, `text 50 50 fill=blue text=X`: the kind, the coordinates, then the options you passed, sorted by name), `Frame`. Children of a container are shown in the order they were created, except grid cells, which are shown by row, then column. Dialogs (`messagebox`) print `[showinfo] TITLE: MESSAGE` (the function name, the title, the message); the answer to a question dialog is supplied by the test. `after` timers run on a test clock. Use only `tkinter` (with `messagebox`) and the standard library; widgets show exactly the texts and options given here.
