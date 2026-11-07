# Brevi's markupsafe (after MarkupSafe 3.0): Markup strings and HTML escaping, in plain Python. Used by
# Brevi's jinja2 and importable on its own. What it lacks (Markup.format with format specs on Markup
# values, Markup(bytes, encoding)) sends the program to CPython's markupsafe.

__version__ = "3.0.3"


def _escape_inner(s):
    return s.replace("&", "&amp;").replace(">", "&gt;").replace("<", "&lt;").replace("'", "&#39;").replace('"', "&#34;")


def escape(s):
    if type(s) is str:
        return Markup(_escape_inner(s))
    if hasattr(s, "__html__"):
        return Markup(s.__html__())
    return Markup(_escape_inner(str(s)))


def escape_silent(s):
    if s is None:
        return Markup()
    return escape(s)


def soft_str(s):
    if not isinstance(s, str):
        return str(s)
    return s


def _unescape(s):
    # html.unescape for the named and numeric references markupsafe's striptags meets
    if "&" not in s:
        return s
    out = []
    i = 0
    n = len(s)
    while i < n:
        c = s[i]
        if c != "&":
            out.append(c)
            i += 1
            continue
        j = s.find(";", i)
        if j == -1 or j - i > 32:
            out.append(c)
            i += 1
            continue
        ent = s[i + 1:j]
        rep = None
        if ent.startswith("#x") or ent.startswith("#X"):
            try:
                rep = chr(int(ent[2:], 16))
            except ValueError:
                rep = None
        elif ent.startswith("#"):
            try:
                rep = chr(int(ent[1:]))
            except ValueError:
                rep = None
        else:
            rep = _ENTITIES.get(ent)
        if rep is None:
            out.append(c)
            i += 1
        else:
            out.append(rep)
            i = j + 1
    return "".join(out)


_ENTITIES = {
    "amp": "&", "lt": "<", "gt": ">", "quot": '"', "apos": "'", "nbsp": "\xa0", "copy": "\xa9", "reg": "\xae",
    "hellip": "…", "mdash": "—", "ndash": "–", "lsquo": "‘", "rsquo": "’",
    "ldquo": "“", "rdquo": "”", "laquo": "\xab", "raquo": "\xbb", "middot": "\xb7", "bull": "•",
    "euro": "€", "pound": "\xa3", "yen": "\xa5", "cent": "\xa2", "deg": "\xb0", "times": "\xd7",
    "divide": "\xf7", "trade": "™", "sect": "\xa7", "para": "\xb6", "iexcl": "\xa1", "iquest": "\xbf",
}


class _MarkupEscapeHelper:
    def __init__(self, obj, esc):
        self.obj = obj
        self.escape = esc

    def __getitem__(self, key):
        return _MarkupEscapeHelper(self.obj[key], self.escape)

    def __str__(self):
        return str(self.escape(self.obj))

    def __repr__(self):
        return str(self.escape(repr(self.obj)))

    def __int__(self):
        return int(self.obj)

    def __float__(self):
        return float(self.obj)


class Markup(str):
    def __html__(self):
        return self

    def __add__(self, value):
        if isinstance(value, str) or hasattr(value, "__html__"):
            return Markup(str.__add__(self, Markup.escape(value)))
        return NotImplemented

    def __radd__(self, value):
        if isinstance(value, str) or hasattr(value, "__html__"):
            return Markup(str.__add__(Markup.escape(value), self))
        return NotImplemented

    def __mul__(self, value):
        return Markup(str.__mul__(self, value))

    def __rmul__(self, value):
        return Markup(str.__mul__(self, value))

    def __mod__(self, value):
        if isinstance(value, tuple):
            value = tuple([_MarkupEscapeHelper(x, Markup.escape) for x in value])
        elif isinstance(value, dict):
            value = {k: _MarkupEscapeHelper(v, Markup.escape) for k, v in value.items()}
        else:
            value = (_MarkupEscapeHelper(value, Markup.escape),)
        return Markup(str.__mod__(str(self), value))

    def __repr__(self):
        return "Markup(" + str.__repr__(self) + ")"

    def join(self, iterable):
        return Markup(str.join(self, [Markup.escape(x) for x in iterable]))

    def split(self, sep=None, maxsplit=-1):
        return [Markup(v) for v in str.split(self, sep, maxsplit)]

    def rsplit(self, sep=None, maxsplit=-1):
        return [Markup(v) for v in str.rsplit(self, sep, maxsplit)]

    def splitlines(self, keepends=False):
        return [Markup(v) for v in str.splitlines(self, keepends)]

    def unescape(self):
        return _unescape(str(self))

    def striptags(self):
        value = str(self)
        while True:
            start = value.find("<!--")
            if start == -1:
                break
            end = value.find("-->", start)
            if end == -1:
                break
            value = value[:start] + value[end + 3:]
        while True:
            start = value.find("<")
            if start == -1:
                break
            end = value.find(">", start)
            if end == -1:
                break
            value = value[:start] + value[end + 1:]
        value = " ".join(value.split())
        return Markup(value).unescape()

    @staticmethod
    def escape(s):
        rv = escape(s)
        if type(rv) is not Markup:
            return Markup(rv)
        return rv

    def __getitem__(self, key):
        return Markup(str.__getitem__(self, key))

    def capitalize(self):
        return Markup(str.capitalize(self))

    def title(self):
        return Markup(str.title(self))

    def lower(self):
        return Markup(str.lower(self))

    def upper(self):
        return Markup(str.upper(self))

    def replace(self, old, new, count=-1):
        return Markup(str.replace(self, old, Markup.escape(new), count))

    def ljust(self, width, fillchar=" "):
        return Markup(str.ljust(self, width, Markup.escape(fillchar)))

    def rjust(self, width, fillchar=" "):
        return Markup(str.rjust(self, width, Markup.escape(fillchar)))

    def lstrip(self, chars=None):
        return Markup(str.lstrip(self, chars))

    def rstrip(self, chars=None):
        return Markup(str.rstrip(self, chars))

    def center(self, width, fillchar=" "):
        return Markup(str.center(self, width, Markup.escape(fillchar)))

    def strip(self, chars=None):
        return Markup(str.strip(self, chars))

    def expandtabs(self, tabsize=8):
        return Markup(str.expandtabs(self, tabsize))

    def swapcase(self):
        return Markup(str.swapcase(self))

    def zfill(self, width):
        return Markup(str.zfill(self, width))

    def casefold(self):
        return Markup(str.casefold(self))

    def removeprefix(self, prefix):
        return Markup(str.removeprefix(self, prefix))

    def removesuffix(self, suffix):
        return Markup(str.removesuffix(self, suffix))

    def partition(self, sep):
        left, s, right = str.partition(self, sep)
        return (Markup(left), Markup(s), Markup(right))

    def rpartition(self, sep):
        left, s, right = str.rpartition(self, sep)
        return (Markup(left), Markup(s), Markup(right))

    def format(self, *args, **kwargs):
        args = [_markup_fmt_arg(a) for a in args]
        kwargs = {k: _markup_fmt_arg(v) for k, v in kwargs.items()}
        return Markup(str.format(str(self), *args, **kwargs))

    def format_map(self, mapping):
        return Markup(str.format(str(self), **{k: _markup_fmt_arg(v) for k, v in mapping.items()}))

    def __html_format__(self, format_spec):
        if format_spec:
            raise ValueError("Unsupported format specification for Markup.")
        return self


def _markup_fmt_arg(v):
    if hasattr(v, "__html__"):
        return str(v.__html__())
    if isinstance(v, str):
        return str(escape(v))
    if isinstance(v, (int, float, bool)) or v is None:
        return v
    return str(escape(str(v)))
