"""tkinter.ttk for the headless stand-in: the same widgets, plus Combobox, Progressbar and Separator."""
from tkinter import (Misc, Frame, LabelFrame, Label, Button, Entry, Checkbutton, Radiobutton, Scale, Spinbox, Scrollbar,
                     _TextBox, _text, _float)

Labelframe = LabelFrame


class Combobox(_TextBox):
    _kind = "Combobox"

    def __init__(self, master=None, cnf=None, **kw):
        _TextBox.__init__(self, master, cnf, **kw)

    def current(self, i=None):
        if i is None:
            cur = self.get()
            return self._lines.index(cur) if cur in self._lines else -1
        self.set(self._lines[i])

    def set(self, value):
        self.delete(0, "end")
        self.insert(0, _text(value))


class Progressbar(Misc):
    _kind = "Progressbar"

    def step(self, amount=1.0):
        var = self._opts.get("variable")
        if var is not None:
            var.set(var.get() + amount)
        else:
            self._opts["value"] = _float(self._opts.get("value", 0)) + amount

    def start(self, *a): pass
    def stop(self): pass


class Separator(Misc):
    _kind = "Separator"


class Style:
    def __init__(self, *a, **k): pass
    def configure(self, *a, **k): pass
    def theme_use(self, *a): return "default"
    def map(self, *a, **k): pass
