import tkinter as tk

KEYS = [
    ["7", "8", "9", "/"],
    ["4", "5", "6", "*"],
    ["1", "2", "3", "-"],
    ["C", "0", "=", "+"],
]


class Calc:
    def __init__(self, root):
        root.title("Calc")
        self.display = tk.Label(root, text="0")
        self.display.grid(row=0, column=0, columnspan=4)
        for r, row in enumerate(KEYS):
            for c, k in enumerate(row):
                tk.Button(root, text=k, command=lambda k=k: self.press(k)).grid(row=r + 1, column=c)
        self.history = tk.Listbox(root)
        self.history.grid(row=5, column=0, columnspan=4)
        root.bind("<Key>", self.key)
        root.bind("<Return>", lambda e: self.press("="))
        root.bind("<Escape>", lambda e: self.press("C"))
        self.clear()

    def clear(self):
        self.acc = None
        self.op = None
        self.cur = "0"
        self.fresh = True
        self.error = False
        self.expr = ""
        self.show()

    def show(self):
        self.display.config(text="Error" if self.error else self.cur)

    def key(self, e):
        if e.char and e.char in "0123456789+-*/=":
            self.press(e.char)

    def compute(self, a, op, b):
        if op == "+":
            return a + b
        if op == "-":
            return a - b
        if op == "*":
            return a * b
        if b == 0:
            return None
        q = abs(a) // abs(b)
        return q if (a < 0) == (b < 0) else -q

    def press(self, k):
        if k == "C":
            self.clear()
            return
        if self.error:
            return
        if k.isdigit():
            if self.fresh or self.cur == "0":
                self.cur = k
            else:
                self.cur += k
            self.fresh = False
        elif k in "+-*/":
            if self.op is not None and not self.fresh:
                self.finish()
                if self.error:
                    return
            elif self.op is None:
                self.acc = int(self.cur)
                self.expr = self.cur
            self.op = k
            self.fresh = True
        elif k == "=":
            if self.op is not None and not self.fresh:
                self.finish()
                self.op = None
                self.fresh = True
        self.show()

    def finish(self):
        b = int(self.cur)
        r = self.compute(self.acc, self.op, b)
        self.expr += self.op + self.cur
        if r is None:
            self.error = True
            self.history.insert(tk.END, self.expr + "=Error")
            self.show()
            return
        self.history.insert(tk.END, self.expr + "=" + str(r))
        self.acc = r
        self.cur = str(r)
        self.expr = self.cur


def main():
    root = tk.Tk()
    Calc(root)
    root.mainloop()


main()
