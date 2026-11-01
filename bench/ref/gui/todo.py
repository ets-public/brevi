import tkinter as tk
from tkinter import messagebox

TITLE = "Todo"


class App:
    def __init__(self, root):
        self.root = root
        root.title(TITLE)
        self.tasks = []  # [text, done]
        self.entry = tk.Entry(root)
        self.entry.pack()
        self.entry.bind("<Return>", lambda e: self.add())
        tk.Button(root, text="Add", command=self.add).pack()
        self.hide = tk.BooleanVar(value=False)
        tk.Checkbutton(root, text="Hide done", variable=self.hide, command=self.refresh).pack()
        self.box = tk.Listbox(root)
        self.box.pack()
        tk.Button(root, text="Done", command=self.done).pack(side="left")
        tk.Button(root, text="Clear done", command=self.clear).pack(side="left")
        self.status = tk.Label(root, text="")
        self.status.pack()
        self.refresh()

    def shown(self):
        return [t for t in self.tasks if not (self.hide.get() and t[1])]

    def refresh(self):
        self.box.delete(0, tk.END)
        for t in self.shown():
            self.box.insert(tk.END, ("[x] " if t[1] else "[ ] ") + t[0])
        n_done = sum(1 for t in self.tasks if t[1])
        self.status.config(text=f"{len(self.tasks) - n_done} open, {n_done} done")

    def add(self):
        text = self.entry.get().strip()
        if not text:
            messagebox.showwarning(TITLE, "Nothing to add")
            return
        if any(t[0] == text and not t[1] for t in self.tasks):
            messagebox.showwarning(TITLE, "Already on the list")
            return
        self.tasks.append([text, False])
        self.entry.delete(0, tk.END)
        self.refresh()

    def done(self):
        sel = self.box.curselection()
        if not sel:
            messagebox.showinfo(TITLE, "Select a task first")
            return
        t = self.shown()[sel[0]]
        t[1] = not t[1]
        self.refresh()

    def clear(self):
        k = sum(1 for t in self.tasks if t[1])
        if k == 0:
            messagebox.showinfo(TITLE, "No done tasks")
            return
        if messagebox.askyesno(TITLE, f"Remove {k} done tasks?"):
            self.tasks = [t for t in self.tasks if not t[1]]
            self.refresh()


def main():
    root = tk.Tk()
    App(root)
    root.mainloop()


main()
