"""tkinter.simpledialog for the headless stand-in: the answer is the next `answer` line."""
from tkinter import _dialog


def askstring(title, prompt, **kw):
    return _dialog("askstring", str(title), str(prompt))


def askinteger(title, prompt, **kw):
    return _dialog("askinteger", str(title), str(prompt))


def askfloat(title, prompt, **kw):
    return _dialog("askfloat", str(title), str(prompt))
