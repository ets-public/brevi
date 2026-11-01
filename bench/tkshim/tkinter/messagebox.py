"""tkinter.messagebox for the headless stand-in: prints `[kind] title: message`."""
from tkinter import _dialog


def _mk(kind):
    def f(title=None, message=None, **kw):
        return _dialog(kind, "" if title is None else str(title), "" if message is None else str(message))
    f.__name__ = kind
    return f


showinfo = _mk("showinfo"); showwarning = _mk("showwarning"); showerror = _mk("showerror")
askyesno = _mk("askyesno"); askokcancel = _mk("askokcancel"); askretrycancel = _mk("askretrycancel")
askyesnocancel = _mk("askyesnocancel"); askquestion = _mk("askquestion")
