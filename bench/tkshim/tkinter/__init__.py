"""A headless stand-in for tkinter, for the GUI benchmark tasks.

It behaves like Brevi's GUI runtime in headless mode (runtime/brevi_tk/src/headless.rs), so the same
tests run a model's tkinter program under CPython (`--lang python`) and built by Brevi (`--lang pyb`):
`mainloop()` reads actions from stdin and `show` prints the window as text. The two must print the
same; tests/gui_shim_test.py checks that against the Rust runtime.

Actions: show, click TEXT, type N TEXT, key N KEY, select N I, slide N X, tap N X Y, wait MS,
answer yes|no|TEXT, close.
"""
import sys

END = "end"; INSERT = "insert"; ANCHOR = "anchor"
LEFT = "left"; RIGHT = "right"; TOP = "top"; BOTTOM = "bottom"
BOTH = "both"; X = "x"; Y = "y"; NONE = "none"
N = "n"; S = "s"; E = "e"; W = "w"; NE = "ne"; NW = "nw"; SE = "se"; SW = "sw"; NSEW = "nsew"; CENTER = "center"
NORMAL = "normal"; DISABLED = "disabled"; ACTIVE = "active"; HIDDEN = "hidden"
HORIZONTAL = "horizontal"; VERTICAL = "vertical"
SINGLE = "single"; BROWSE = "browse"; MULTIPLE = "multiple"; EXTENDED = "extended"
YES = True; NO = False; TRUE = 1; FALSE = 0
RAISED = "raised"; SUNKEN = "sunken"; FLAT = "flat"; RIDGE = "ridge"; GROOVE = "groove"; SOLID = "solid"
WORD = "word"; CHAR = "char"

_widgets = []
_vars = []
_timers = []
_state = {"now": 0, "seq": 0, "item": 0, "quit": False, "answers": []}


class TclError(Exception):
    pass


def _text(v):
    if v is None:
        return ""
    if isinstance(v, Variable):
        return _text(v._val)
    if isinstance(v, bool):
        return "1" if v else "0"
    if isinstance(v, float):
        return repr(v)
    if isinstance(v, (list, tuple)):
        return " ".join(_text(x) for x in v)
    return str(v)


def _num(x):
    x = float(x)
    if x == int(x) and abs(x) < 1e15:
        return str(int(x))
    return repr(x)


def _int(v):
    if isinstance(v, Variable):
        return _int(v._val)
    if isinstance(v, (bool, int)):
        return int(v)
    if isinstance(v, float):
        return int(v)
    try:
        return int(float(str(v).strip()))
    except ValueError:
        return 0


def _float(v):
    if isinstance(v, Variable):
        return _float(v._val)
    try:
        return float(v)
    except (TypeError, ValueError):
        return 0.0


def _truthy(v):
    if isinstance(v, Variable):
        return _truthy(v._val)
    if isinstance(v, str):
        return v != "" and v != "0"
    return bool(v)


# ---------------------------------------------------------------- variables

class Variable:
    _kind = "StringVar"
    _default = ""

    def __init__(self, master=None, value=None, name=None):
        self._val = self._coerce(self._default if value is None else value)
        self._traces = []
        _vars.append(self)

    def _coerce(self, v):
        return _text(v)

    def get(self):
        return self._val

    def set(self, value):
        self._val = self._coerce(value)
        for cb in list(self._traces):
            cb("PY_VAR", "", "write")

    def trace_add(self, mode, callback):
        self._traces.append(callback)
        return str(len(self._traces))

    def trace(self, mode, callback):
        return self.trace_add(mode, callback)

    trace_variable = trace


class StringVar(Variable):
    pass


class IntVar(Variable):
    _kind = "IntVar"
    _default = 0

    def _coerce(self, v):
        return _int(v)


class BooleanVar(Variable):
    _kind = "BooleanVar"
    _default = False

    def _coerce(self, v):
        return _truthy(v)


class DoubleVar(Variable):
    _kind = "DoubleVar"
    _default = 0.0

    def _coerce(self, v):
        return _float(v)


# ---------------------------------------------------------------- widgets

class Event:
    def __init__(self, widget=None, keysym="", char="", x=0, y=0):
        self.widget, self.keysym, self.char, self.x, self.y = widget, keysym, char, x, y
        self.delta = 0
        self.num = 1


def _index(s, idx):
    n = len(s)
    if isinstance(idx, int) and not isinstance(idx, bool):
        return max(0, min(idx, n))
    t = str(idx)
    if t in ("end", "insert") or t.startswith("end"):
        return n
    if "." in t:
        line, _, col = t.partition(".")
        line = int(line) if line.isdigit() else 1
        lines = s.split("\n")
        off = sum(len(l) + 1 for l in lines[:min(max(line - 1, 0), len(lines))])
        this = len(lines[line - 1]) if 0 < line <= len(lines) else 0
        c = this if col == "end" else min(int(col) if col.isdigit() else 0, this)
        return min(off + c, n)
    try:
        return max(0, min(int(t), n))
    except ValueError:
        return 0


class Misc:
    _kind = "Frame"

    def __init__(self, master=None, cnf=None, **kw):
        if master is None and self._kind not in ("Tk",):
            master = _default_root()
        self.master = master
        self._opts = {}
        self._place = None
        self._alive = True
        self._children = []
        self._text = ""
        self._lines = []
        self._sel = []
        self._items = []
        self._binds = []
        self._checked = False
        self._title = ""
        if cnf:
            kw = {**cnf, **kw}
        self._set_opts(kw)
        _widgets.append(self)
        if master is not None:
            master._children.append(self)

    def _set_opts(self, kw):
        for k, v in kw.items():
            k = k.rstrip("_")
            if k == "values":
                self._lines = [_text(x) for x in v]
            self._opts[k] = v

    # options
    def config(self, cnf=None, **kw):
        if cnf:
            kw = {**cnf, **kw}
        self._set_opts(kw)

    configure = config

    def cget(self, key):
        v = self._opts.get(key, "")
        return v

    def __getitem__(self, key):
        return self.cget(key)

    def __setitem__(self, key, value):
        self.config(**{key: value})

    def keys(self):
        return list(self._opts)

    # geometry
    def pack(self, cnf=None, **kw):
        self._place = ("pack", kw)

    def grid(self, cnf=None, **kw):
        self._place = ("grid", kw)

    def place(self, cnf=None, **kw):
        self._place = ("place", kw)

    pack_configure = pack
    grid_configure = grid

    def pack_forget(self):
        self._place = None

    grid_forget = grid_remove = place_forget = pack_forget

    def columnconfigure(self, *a, **k): pass
    def rowconfigure(self, *a, **k): pass
    grid_columnconfigure = columnconfigure
    grid_rowconfigure = rowconfigure
    def focus(self): pass
    focus_set = focus_force = focus
    def update(self): pass
    def update_idletasks(self): pass
    def see(self, *a): pass
    def lift(self, *a): pass
    def tkraise(self, *a): pass
    def grab_set(self): pass
    def wait_window(self, *a): pass
    def option_add(self, *a, **k): pass
    def protocol(self, *a): pass
    def iconbitmap(self, *a, **k): pass
    def attributes(self, *a, **k): pass
    def resizable(self, *a, **k): pass
    def minsize(self, *a, **k): pass
    def maxsize(self, *a, **k): pass
    def transient(self, *a): pass

    def winfo_children(self):
        return [c for c in self._children if c._alive]

    def winfo_exists(self):
        return 1 if self._alive else 0

    def winfo_width(self):
        return _int(self._opts.get("width", 1))

    def winfo_height(self):
        return _int(self._opts.get("height", 1))

    def destroy(self):
        stack = [self]
        while stack:
            w = stack.pop()
            w._alive = False
            stack.extend(w._children)
        if self._kind == "Tk":
            _state["quit"] = True

    def quit(self):
        _state["quit"] = True

    # events and time
    def bind(self, sequence, func=None, add=None):
        self._binds.append((sequence, func))

    def bind_all(self, sequence, func=None, add=None):
        _default_root()._binds.append((sequence, func))

    def unbind(self, sequence, funcid=None):
        self._binds = [b for b in self._binds if b[0] != sequence]

    def after(self, ms, func=None, *args):
        _state["seq"] += 1
        f = (lambda: func(*args)) if args else func
        _timers.append((_state["now"] + max(int(ms), 0), _state["seq"], f))
        return _state["seq"]

    def after_idle(self, func, *args):
        return self.after(0, func, *args)

    def after_cancel(self, id):
        _timers[:] = [t for t in _timers if t[1] != id]

    def mainloop(self, n=0):
        _run()

    # windows
    def title(self, t=None):
        if t is None:
            return self._title
        self._title = t

    wm_title = title

    def geometry(self, g=None):
        if g is not None:
            self._opts["geometry"] = g


class Tk(Misc):
    _kind = "Tk"

    def __init__(self, *a, **kw):
        global _root
        Misc.__init__(self, None)
        self._title = "tk"
        _root = self


_root = None


def _default_root():
    if _root is None:
        Tk()
    return _root


class Toplevel(Misc):
    _kind = "Toplevel"

    def __init__(self, master=None, cnf=None, **kw):
        Misc.__init__(self, master, cnf, **kw)
        self._title = "tk"


class Frame(Misc):
    _kind = "Frame"


class LabelFrame(Misc):
    _kind = "LabelFrame"


Labelframe = LabelFrame


class Label(Misc):
    _kind = "Label"


Message = Label


class Button(Misc):
    _kind = "Button"

    def invoke(self):
        _invoke(self)

    def flash(self):
        pass


class _TextBox(Misc):
    def get(self, first=None, last=None):
        cur = _box_text(self)
        if self._kind != "Text":
            return cur
        i = _index(cur, first if first is not None else 0)
        if last is None:
            return cur[i:i + 1]
        nl = str(last).startswith("end") and str(last) != "end-1c"
        j = len(cur) if nl else _index(cur, last)
        return cur[i:j] + ("\n" if nl else "")

    def insert(self, index, s, *tags):
        cur = _box_text(self)
        i = _index(cur, index)
        _set_box(self, cur[:i] + _text(s) + cur[i:])

    def delete(self, first, last=None):
        cur = _box_text(self)
        i = _index(cur, first)
        j = _index(cur, last) if last is not None else min(i + 1, len(cur))
        if i < j:
            _set_box(self, cur[:i] + cur[j:])

    def icursor(self, *a): pass
    def select_range(self, *a): pass
    def selection_range(self, *a): pass
    def tag_configure(self, *a, **k): pass
    def tag_add(self, *a, **k): pass
    def mark_set(self, *a, **k): pass
    def edit_reset(self): pass


class Entry(_TextBox):
    _kind = "Entry"


class Text(_TextBox):
    _kind = "Text"


class Spinbox(_TextBox):
    _kind = "Spinbox"

    def __init__(self, master=None, cnf=None, **kw):
        _TextBox.__init__(self, master, cnf, **kw)
        if "from_" in kw and "textvariable" not in kw:
            self._text = _text(kw["from_"])


class Checkbutton(Misc):
    _kind = "Checkbutton"

    def invoke(self):
        _invoke(self)

    def select(self):
        v = self._opts.get("variable")
        if v is not None:
            v.set(self._opts.get("onvalue", 1))
        else:
            self._checked = True

    def deselect(self):
        v = self._opts.get("variable")
        if v is not None:
            v.set(self._opts.get("offvalue", 0))
        else:
            self._checked = False

    def toggle(self):
        self.deselect() if _checked(self) else self.select()


class Radiobutton(Misc):
    _kind = "Radiobutton"

    def invoke(self):
        _invoke(self)

    def select(self):
        v = self._opts.get("variable")
        if v is not None:
            v.set(self._opts.get("value"))
        else:
            self._checked = True


class Listbox(Misc):
    _kind = "Listbox"

    def insert(self, index, *items):
        n = len(self._lines)
        i = n if str(index).startswith("end") else max(0, min(_int(index), n))
        for k, s in enumerate(items):
            self._lines.insert(i + k, _text(s))

    def delete(self, first, last=None):
        n = len(self._lines)
        ix = lambda v: n - 1 if str(v).startswith("end") else _int(v)
        i = ix(first)
        j = ix(last) if last is not None else i
        if i < n:
            j = min(j, n - 1)
            if j >= i:
                del self._lines[i:j + 1]
        self._sel = []

    def get(self, first, last=None):
        if last is None:
            i = _int(first)
            return self._lines[i] if 0 <= i < len(self._lines) else ""
        n = len(self._lines)
        j = n - 1 if str(last).startswith("end") else _int(last)
        return tuple(self._lines[_int(first):j + 1])

    def size(self):
        return len(self._lines)

    def curselection(self):
        return tuple(self._sel)

    def selection_set(self, first, last=None):
        i = _int(first)
        if 0 <= i < len(self._lines) and i not in self._sel:
            self._sel.append(i)
            self._sel.sort()

    select_set = selection_set

    def selection_clear(self, first=0, last=None):
        self._sel = []

    select_clear = selection_clear

    def activate(self, i): pass

    def index(self, i):
        return len(self._lines) if str(i).startswith("end") else _int(i)


class Scale(Misc):
    _kind = "Scale"

    def get(self):
        v = _scale_value(self)
        return int(v) if v == int(v) and not isinstance(self._opts.get("resolution"), float) else v

    def set(self, value):
        var = self._opts.get("variable")
        if var is not None:
            var.set(value)
        else:
            self._opts["value"] = float(value)


class Scrollbar(Misc):
    _kind = "Scrollbar"

    def set(self, *a): pass
    def yview(self, *a): pass
    def xview(self, *a): pass


class Canvas(Misc):
    _kind = "Canvas"

    def _create(self, kind, args, kw):
        coords = []
        for a in args:
            if isinstance(a, (list, tuple)):
                for b in a:
                    if isinstance(b, (list, tuple)):
                        coords.extend(float(c) for c in b)
                    else:
                        coords.append(float(b))
            else:
                coords.append(float(a))
        _state["item"] += 1
        self._items.append({"id": _state["item"], "kind": kind, "coords": coords, "opts": {k.rstrip("_"): v for k, v in kw.items()}})
        return _state["item"]

    def create_rectangle(self, *a, **kw): return self._create("rectangle", a, kw)
    def create_oval(self, *a, **kw): return self._create("oval", a, kw)
    def create_line(self, *a, **kw): return self._create("line", a, kw)
    def create_polygon(self, *a, **kw): return self._create("polygon", a, kw)
    def create_text(self, *a, **kw): return self._create("text", a, kw)
    def create_arc(self, *a, **kw): return self._create("arc", a, kw)

    def _find(self, tag):
        return [it for it in self._items if it["id"] == tag or str(it["opts"].get("tags", "")) == str(tag) or (isinstance(it["opts"].get("tags"), (list, tuple)) and tag in it["opts"]["tags"])]

    def coords(self, tag, *xs):
        its = self._find(tag)
        if not its:
            return []
        if xs:
            flat = []
            for x in xs:
                flat.extend(x if isinstance(x, (list, tuple)) else [x])
            its[0]["coords"] = [float(x) for x in flat]
        return list(its[0]["coords"])

    def move(self, tag, dx, dy):
        for it in self._find(tag):
            it["coords"] = [c + (dx if k % 2 == 0 else dy) for k, c in enumerate(it["coords"])]

    def itemconfig(self, tag, cnf=None, **kw):
        for it in self._find(tag):
            it["opts"].update({k.rstrip("_"): v for k, v in kw.items()})

    itemconfigure = itemconfig

    def itemcget(self, tag, opt):
        its = self._find(tag)
        return its[0]["opts"].get(opt, "") if its else ""

    def delete(self, *tags):
        for tag in tags:
            if tag == "all":
                self._items = []
            else:
                gone = {id(it) for it in self._find(tag)}
                self._items = [it for it in self._items if id(it) not in gone]

    def find_all(self):
        return tuple(it["id"] for it in self._items)

    def find_overlapping(self, x1, y1, x2, y2):
        out = []
        for it in self._items:
            c = it["coords"]
            if len(c) >= 4:
                ax1, ax2 = sorted((c[0], c[2]))
                ay1, ay2 = sorted((c[1], c[3]))
                if ax1 <= x2 and x1 <= ax2 and ay1 <= y2 and y1 <= ay2:
                    out.append(it["id"])
        return tuple(out)

    def find_closest(self, x, y):
        hits = self.find_overlapping(x, y, x, y)
        return hits[-1:] if hits else ()

    def bbox(self, tag):
        its = self._find(tag)
        if not its:
            return None
        xs = its[0]["coords"][0::2]
        ys = its[0]["coords"][1::2]
        return (min(xs), min(ys), max(xs), max(ys))


# ---------------------------------------------------------------- shared behaviour

def _box_text(w):
    v = w._opts.get("textvariable")
    return _text(v.get()) if v is not None else w._text


def _set_box(w, s):
    v = w._opts.get("textvariable")
    if v is not None:
        v.set(s)
    else:
        w._text = s


def _label_text(w):
    v = w._opts.get("textvariable")
    if v is not None:
        return _text(v)
    return _text(w._opts.get("text"))


def _checked(w):
    var = w._opts.get("variable")
    if var is None:
        return w._checked
    if w._kind == "Radiobutton":
        return _text(var.get()) == _text(w._opts.get("value"))
    if "onvalue" in w._opts:
        return _text(var.get()) == _text(w._opts["onvalue"])
    return _truthy(var.get())


def _scale_value(w):
    for k in ("variable", "value", "from"):
        if k in w._opts:
            return _float(w._opts[k])
    return 0.0


def _invoke(w):
    if _text(w._opts.get("state")) == "disabled":
        return
    var = w._opts.get("variable")
    if w._kind == "Checkbutton":
        now = not _checked(w)
        if var is not None:
            var.set(w._opts.get("onvalue", 1) if now else w._opts.get("offvalue", 0))
        else:
            w._checked = now
    elif w._kind == "Radiobutton":
        if var is not None:
            var.set(w._opts.get("value"))
        else:
            w._checked = True
    cmd = w._opts.get("command")
    if callable(cmd):
        cmd()


def _fire(w, name, ev):
    for seq, f in list(w._binds):
        if seq == name and f is not None:
            f(ev)


def _shown(w):
    cs = [c for c in w._children if c._alive and c._place is not None and c._kind != "Toplevel"]

    def key(c):
        if c._place[0] == "grid":
            kw = c._place[1]
            return (_int(kw.get("row", 0)), _int(kw.get("column", 0)))
        return (0, 0)
    return sorted(cs, key=key)


def _quote(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def _describe(w, depth, out):
    pad = "  " * depth
    k = w._kind
    dis = " (disabled)" if _text(w._opts.get("state")) == "disabled" else ""
    if k in ("Tk", "Toplevel"):
        line = "window " + _quote(w._title)
    elif k == "Frame":
        line = "Frame"
    elif k in ("LabelFrame", "Label", "Button"):
        line = f"{k} {_quote(_label_text(w))}"
    elif k in ("Entry", "Spinbox", "Text"):
        line = f"{k} {_quote(_box_text(w))}"
    elif k == "Combobox":
        line = f"Combobox {_quote(_box_text(w))} of [{', '.join(w._lines)}]"
    elif k == "Checkbutton":
        line = f"Checkbutton {_quote(_label_text(w))} [{'x' if _checked(w) else ' '}]"
    elif k == "Radiobutton":
        line = f"Radiobutton {_quote(_label_text(w))} ({'o' if _checked(w) else ' '})"
    elif k == "Listbox":
        line = "Listbox [" + " | ".join(l + ("*" if i in w._sel else "") for i, l in enumerate(w._lines)) + "]"
    elif k == "Scale":
        line = "Scale " + _num(_scale_value(w))
    elif k == "Progressbar":
        v = w._opts.get("variable", w._opts.get("value", 0))
        line = "Progressbar " + _num(_float(v))
    elif k == "Canvas":
        line = f"Canvas {_int(w._opts.get('width', 0))}x{_int(w._opts.get('height', 0))}"
    elif k == "Scrollbar":
        return
    else:
        line = k
    out.append(pad + line + dis)
    if k == "Canvas":
        for it in w._items:
            cs = " ".join(_num(c) for c in it["coords"])
            os_ = " ".join(f"{key}={_text(v)}" for key, v in sorted(it["opts"].items()) if key != "tags")
            out.append(f"{pad}  {it['kind']} {cs}{' ' if os_ else ''}{os_}")
    for c in _shown(w):
        _describe(c, depth + 1, out)


def _show():
    out = []
    roots = [_default_root()] + [w for w in _widgets if w._kind == "Toplevel" and w._alive]
    for r in roots:
        if r._alive:
            _describe(r, 0, out)
    return "\n".join(out)


def _order():
    out = []

    def walk(w):
        out.append(w)
        for c in _shown(w):
            walk(c)
    for r in [_default_root()] + [w for w in _widgets if w._kind == "Toplevel" and w._alive]:
        walk(r)
    return out


def _nth(kinds, n):
    ws = [w for w in _order() if w._kind in kinds]
    return ws[n - 1] if 1 <= n <= len(ws) else None


def _run_timers(until):
    while True:
        due = [t for t in _timers if t[0] <= until]
        if not due:
            break
        t = min(due, key=lambda t: (t[0], t[1]))
        _timers.remove(t)
        _state["now"] = max(_state["now"], t[0])
        t[2]()
    _state["now"] = max(_state["now"], until)


_TEXTS = ("Entry", "Text", "Spinbox", "Combobox")


def _key(w, key):
    ev = Event(w, key, key if len(key) == 1 else "")
    root = _default_root()
    for n in (f"<{key}>", f"<KeyPress-{key}>", f"<Key-{key}>", key if len(key) == 1 else ""):
        if n:
            _fire(w, n, ev)
            if w is not root:
                _fire(root, n, ev)
    _fire(w, "<Key>", ev)


def _run():
    _default_root()
    _run_timers(_state["now"])
    for line in sys.stdin:
        if _state["quit"]:
            break
        l = line.strip()
        if not l or l.startswith("#"):
            continue
        cmd, _, rest = l.partition(" ")
        a1, _, a2 = rest.partition(" ")
        ok = True
        if cmd == "show":
            print(_show())
        elif cmd == "click":
            ws = [w for w in _widgets if w._alive and w._kind in ("Button", "Checkbutton", "Radiobutton") and w._place is not None and _label_text(w) == rest]
            if ws:
                _invoke(ws[0])
            else:
                ok = False
        elif cmd == "type":
            w = _nth(_TEXTS, int(a1)) if a1.isdigit() else None
            if w is None:
                ok = False
            else:
                _set_box(w, a2)
        elif cmd == "key":
            w = _default_root() if a1 == "0" else (_nth(_TEXTS, int(a1)) if a1.isdigit() else None)
            if w is None:
                ok = False
            else:
                _key(w, a2)
        elif cmd == "select":
            w = _nth(("Listbox",), int(a1)) if a1.isdigit() else None
            if w is None or not a2.isdigit() or int(a2) >= len(w._lines):
                ok = False
            else:
                w._sel = [int(a2)]
                _fire(w, "<<ListboxSelect>>", Event(w))
        elif cmd == "slide":
            w = _nth(("Scale",), int(a1)) if a1.isdigit() else None
            try:
                x = float(a2)
            except ValueError:
                w = None
            if w is None:
                ok = False
            else:
                Scale.set(w, x)
                cmd_ = w._opts.get("command")
                if callable(cmd_):
                    cmd_(_num(x))
        elif cmd == "tap":
            xy = a2.split(" ")
            w = _nth(("Canvas",), int(a1)) if a1.isdigit() else None
            if w is None or len(xy) != 2 or not all(p.lstrip("-").isdigit() for p in xy):
                ok = False
            else:
                ev = Event(w, x=int(xy[0]), y=int(xy[1]))
                for n in ("<Button-1>", "<Button>", "<1>", "<ButtonPress-1>"):
                    _fire(w, n, ev)
        elif cmd == "wait":
            if a1.isdigit():
                _run_timers(_state["now"] + int(a1))
            else:
                ok = False
        elif cmd == "answer":
            _state["answers"].append(rest)
        elif cmd == "close":
            _default_root().destroy()
        else:
            ok = False
        if not ok:
            print(f"? {l}")
        sys.stdout.flush()
        _run_timers(_state["now"])


def _dialog(kind, title, msg):
    ans = _state["answers"].pop(0) if _state["answers"] else None
    print(f"[{kind}] {title}: {msg}")
    if kind in ("askyesno", "askokcancel", "askretrycancel"):
        return True if ans is None else ans in ("yes", "ok", "retry")
    if kind == "askyesnocancel":
        return {None: True, "no": False, "cancel": None}.get(ans, True)
    if kind == "askquestion":
        return ans or "yes"
    if kind == "askstring":
        return ans
    if kind == "askinteger":
        try:
            return int(ans)
        except (TypeError, ValueError):
            return None
    if kind == "askfloat":
        try:
            return float(ans)
        except (TypeError, ValueError):
            return None
    return "ok"
