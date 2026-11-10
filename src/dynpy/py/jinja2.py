# Brevi's jinja2 (after Jinja 3.1.6), in plain Python, so templates render natively in a Brevi program.
# Jinja compiles a template to Python source and runs it with exec; Brevi programs have no exec, so this
# copy keeps Jinja's lexer and parser and walks the parsed tree instead, following what the generated code
# does (scoping of loops, macros, blocks, includes and imports; filters, tests, undefined values,
# autoescaping, whitespace control). What it lacks (async rendering, the sandbox, i18n, custom extensions,
# loaders that read files) sends the program to CPython's jinja2 (src/caps.rs JINJA2_LACKS) or, inside a
# template Brevi only sees at run time, raises NotImplementedError.

import re
import json
import math
from markupsafe import Markup, escape, soft_str

__version__ = "3.1.6"


# -- exceptions


class TemplateError(Exception):
    def __init__(self, message=None):
        Exception.__init__(self, message)

    @property
    def message(self):
        return self.args[0] if self.args else None


class TemplateNotFound(IOError, LookupError, TemplateError):
    def __init__(self, name, message=None):
        IOError.__init__(self, name)
        if message is None:
            if isinstance(name, Undefined):
                name._fail_with_undefined_error()
            message = name
        self._message = message
        self.name = name
        self.templates = [name]

    @property
    def message(self):
        return self._message

    def __str__(self):
        return str(self._message)


class TemplatesNotFound(TemplateNotFound):
    def __init__(self, names=(), message=None):
        if message is None:
            parts = []
            for name in names:
                if isinstance(name, Undefined):
                    parts.append(name._undefined_message)
                else:
                    parts.append(name)
            message = "none of the templates given were found: " + ", ".join([str(p) for p in parts])
        TemplateNotFound.__init__(self, names[-1] if names else None, message)
        self.templates = list(names)


class TemplateSyntaxError(TemplateError):
    def __init__(self, message, lineno, name=None, filename=None):
        TemplateError.__init__(self, message)
        self.lineno = lineno
        self.name = name
        self.filename = filename
        self.source = None
        self.translated = False

    def __str__(self):
        if self.translated:
            return self.message
        location = "line " + str(self.lineno)
        name = self.filename or self.name
        if name:
            location = 'File "' + name + '", ' + location
        lines = [self.message, "  " + location]
        if self.source is not None:
            src_lines = self.source.splitlines()
            if 0 <= self.lineno - 1 < len(src_lines):
                lines.append("    " + src_lines[self.lineno - 1].strip())
        return "\n".join(lines)


class TemplateAssertionError(TemplateSyntaxError):
    pass


class TemplateRuntimeError(TemplateError):
    pass


class UndefinedError(TemplateRuntimeError):
    pass


class SecurityError(TemplateRuntimeError):
    pass


class FilterArgumentError(TemplateRuntimeError):
    pass


def _lacks(what):
    raise NotImplementedError("Brevi's jinja2 does not have " + what + " yet (CPython's jinja2 does)")


# -- utils


_DICT_ATTRS = frozenset(("clear", "copy", "fromkeys", "get", "items", "keys", "pop", "popitem", "setdefault", "update", "values"))


class _Missing:
    def __repr__(self):
        return "missing"

    def __reduce__(self):
        return "missing"


missing = _Missing()

_PASS_CONTEXT = 1
_PASS_EVAL_CONTEXT = 2
_PASS_ENVIRONMENT = 3


def pass_context(f):
    f.jinja_pass_arg = _PASS_CONTEXT
    return f


def pass_eval_context(f):
    f.jinja_pass_arg = _PASS_EVAL_CONTEXT
    return f


def pass_environment(f):
    f.jinja_pass_arg = _PASS_ENVIRONMENT
    return f


def _pass_arg(obj):
    return getattr(obj, "jinja_pass_arg", None)


def _builtin_type(v):
    return (v is None or v is True or v is False or isinstance(v, (int, float, str, list, dict, tuple, set, frozenset, bytes, range)))


def _type_name(obj):
    if isinstance(obj, bool):
        return "bool"
    if isinstance(obj, Markup):
        return "Markup"
    for t, n in ((int, "int"), (float, "float"), (str, "str"), (list, "list"), (dict, "dict"), (tuple, "tuple"),
                 (set, "set"), (frozenset, "frozenset"), (bytes, "bytes"), (range, "range")):
        if type(obj) is t:
            return n
    return type(obj).__name__


def object_type_repr(obj):
    if obj is None:
        return "None"
    elif obj is Ellipsis:
        return "Ellipsis"
    if _builtin_type(obj) and type(obj).__module__ == "builtins":
        return _type_name(obj) + " object"
    cls = type(obj)
    if isinstance(obj, Markup):
        return "markupsafe.Markup object"
    own = _own_class_name(obj)
    if own is not None:
        return own + " object"
    if callable(obj) and type(obj).__name__ in ("function", "builtin_function_or_method", "method"):
        return type(obj).__name__ + " object"
    mod = cls.__module__
    if mod == "builtins":
        return cls.__name__ + " object"
    return mod + "." + cls.__name__ + " object"


def _own_class_name(obj):
    # Jinja's module path for the classes of this copy
    for cls, path in _OWN_CLASSES:
        if type(obj) is cls:
            return path
    return None


def select_autoescape(enabled_extensions=("html", "htm", "xml"), disabled_extensions=(), default_for_string=True, default=False):
    enabled_patterns = tuple(["." + x.lstrip(".").lower() for x in enabled_extensions])
    disabled_patterns = tuple(["." + x.lstrip(".").lower() for x in disabled_extensions])

    def autoescape(template_name):
        if template_name is None:
            return default_for_string
        template_name = template_name.lower()
        if template_name.endswith(enabled_patterns):
            return True
        if template_name.endswith(disabled_patterns):
            return False
        return default

    return autoescape


def htmlsafe_json_dumps(obj, dumps=None, **kwargs):
    if dumps is None:
        dumps = json.dumps
    return Markup(dumps(obj, **kwargs).replace("<", "\\u003c").replace(">", "\\u003e").replace("&", "\\u0026").replace("'", "\\u0027"))


_URL_SAFE = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_.-~"


def url_quote(obj, charset="utf-8", for_qs=False):
    if not isinstance(obj, bytes):
        if not isinstance(obj, str):
            obj = str(obj)
        obj = obj.encode(charset)
    safe = _URL_SAFE if for_qs else _URL_SAFE + "/"
    out = []
    for b in obj:
        c = chr(b)
        if b < 128 and c in safe:
            out.append(c)
        else:
            out.append("%" + "0123456789ABCDEF"[b >> 4] + "0123456789ABCDEF"[b & 15])
    rv = "".join(out)
    if for_qs:
        rv = rv.replace("%20", "+")
    return rv


class Cycler:
    def __init__(self, *items):
        if not items:
            raise RuntimeError("at least one item has to be provided")
        self.items = items
        self.pos = 0

    def reset(self):
        self.pos = 0

    @property
    def current(self):
        return self.items[self.pos]

    def next(self):
        rv = self.current
        self.pos = (self.pos + 1) % len(self.items)
        return rv

    def __next__(self):
        return self.next()


class Joiner:
    def __init__(self, sep=", "):
        self.sep = sep
        self.used = False

    def __call__(self):
        if not self.used:
            self.used = True
            return ""
        return self.sep


class Namespace:
    def __init__(self, *args, **kwargs):
        self._ns_attrs = dict(*args, **kwargs)

    def __getattr__(self, name):
        if name == "_ns_attrs":
            raise AttributeError(name)
        try:
            return self._ns_attrs[name]
        except KeyError:
            raise AttributeError(name) from None

    def __getitem__(self, name):
        try:
            return self._ns_attrs[name]
        except KeyError:
            raise AttributeError(name) from None

    def __setitem__(self, name, value):
        self._ns_attrs[name] = value

    def __repr__(self):
        return "<Namespace " + repr(self._ns_attrs) + ">"


_LOREM_WORDS = None


def generate_lorem_ipsum(n=5, html=True, min=20, max=100):
    _lacks("the lipsum() global")


def _pformat(obj):
    rv = _prepr(obj)
    if len(rv) > 80:
        _lacks("pprint of values longer than 80 characters")
    return rv


def _prepr(obj):
    if isinstance(obj, dict) and type(obj) is dict:
        try:
            items = sorted(obj.items(), key=lambda kv: kv[0])
        except TypeError:
            items = list(obj.items())
        return "{" + ", ".join([_prepr(k) + ": " + _prepr(v) for k, v in items]) + "}"
    if type(obj) is list:
        return "[" + ", ".join([_prepr(v) for v in obj]) + "]"
    if type(obj) is tuple:
        if len(obj) == 1:
            return "(" + _prepr(obj[0]) + ",)"
        return "(" + ", ".join([_prepr(v) for v in obj]) + ")"
    return repr(obj)


# -- undefined values


class Undefined:
    def __init__(self, hint=None, obj=missing, name=None, exc=UndefinedError):
        self._undefined_hint = hint
        self._undefined_obj = obj
        self._undefined_name = name
        self._undefined_exception = exc

    @property
    def _undefined_message(self):
        if self._undefined_hint:
            return self._undefined_hint
        if self._undefined_obj is missing:
            return repr(self._undefined_name) + " is undefined"
        if not isinstance(self._undefined_name, str):
            return object_type_repr(self._undefined_obj) + " has no element " + repr(self._undefined_name)
        return repr(object_type_repr(self._undefined_obj)) + " has no attribute " + repr(self._undefined_name)

    def _fail_with_undefined_error(self, *args, **kwargs):
        raise self._undefined_exception(self._undefined_message)

    def __getattr__(self, name):
        if name[:2] == "__" and name[-2:] == "__":
            raise AttributeError(name)
        if name[:11] == "_undefined_":
            raise AttributeError(name)
        return self._fail_with_undefined_error()

    __add__ = __radd__ = __sub__ = __rsub__ = _fail_with_undefined_error
    __mul__ = __rmul__ = __div__ = __rdiv__ = _fail_with_undefined_error
    __truediv__ = __rtruediv__ = _fail_with_undefined_error
    __floordiv__ = __rfloordiv__ = _fail_with_undefined_error
    __mod__ = __rmod__ = _fail_with_undefined_error
    __pos__ = __neg__ = _fail_with_undefined_error
    __call__ = __getitem__ = _fail_with_undefined_error
    __lt__ = __le__ = __gt__ = __ge__ = _fail_with_undefined_error
    __int__ = __float__ = __complex__ = _fail_with_undefined_error
    __pow__ = __rpow__ = _fail_with_undefined_error

    def __eq__(self, other):
        return type(self) is type(other)

    def __ne__(self, other):
        return not self.__eq__(other)

    def __hash__(self):
        return id(type(self))

    def __str__(self):
        return ""

    def __len__(self):
        return 0

    def __iter__(self):
        return iter(())

    def __bool__(self):
        return False

    def __repr__(self):
        return "Undefined"


class ChainableUndefined(Undefined):
    def __html__(self):
        return str(self)

    def __getattr__(self, name):
        if name[:2] == "__" and name[-2:] == "__":
            raise AttributeError(name)
        if name[:11] == "_undefined_":
            raise AttributeError(name)
        return self

    def __getitem__(self, _name):
        return self


class DebugUndefined(Undefined):
    def __str__(self):
        if self._undefined_hint:
            message = "undefined value printed: " + self._undefined_hint
        elif self._undefined_obj is missing:
            message = self._undefined_name
        else:
            message = "no such element: " + object_type_repr(self._undefined_obj) + "[" + repr(self._undefined_name) + "]"
        return "{{ " + str(message) + " }}"


class StrictUndefined(Undefined):
    __iter__ = __str__ = __len__ = Undefined._fail_with_undefined_error
    __eq__ = __ne__ = __bool__ = __hash__ = Undefined._fail_with_undefined_error
    __contains__ = Undefined._fail_with_undefined_error


def make_logging_undefined(logger=None, base=Undefined):
    _lacks("make_logging_undefined")


def _is_strict(v):
    return isinstance(v, StrictUndefined)


# -- lexer

_whitespace_re = re.compile(r"\s+")
_newline_re = re.compile(r"(\r\n|\r|\n)")
_string_re = re.compile(r"('([^'\\]*(?:\\.[^'\\]*)*)'" r'|"([^"\\]*(?:\\.[^"\\]*)*)")', re.S)
_integer_re = re.compile(r"(0[bB](_?[0-1])+|0[oO](_?[0-7])+|0[xX](_?[0-9a-fA-F])+|[1-9](_?[0-9])*|0(_?0)*)")
_float_re = re.compile(r"(?<!\.)([0-9]+_)*[0-9]+((\.([0-9]+_)*[0-9]+)?[eE][+\-]?([0-9]+_)*[0-9]+|\.([0-9]+_)*[0-9]+)")
_name_re = re.compile(r"[\w]+")
_operators = {
    "+": "add", "-": "sub", "/": "div", "//": "floordiv", "*": "mul", "%": "mod", "**": "pow", "~": "tilde",
    "[": "lbracket", "]": "rbracket", "(": "lparen", ")": "rparen", "{": "lbrace", "}": "rbrace", "==": "eq",
    "!=": "ne", ">": "gt", ">=": "gteq", "<": "lt", "<=": "lteq", "=": "assign", ".": "dot", ":": "colon",
    "|": "pipe", ",": "comma", ";": "semicolon",
}
_reverse_operators = {v: k for k, v in _operators.items()}
_operator_re = re.compile(r"(//|\*\*|==|!=|>=|<=|\+|\-|/|\*|%|\~|\[|\]|\(|\)|\{|\}|>|<|=|\.|:|\||,|;)")
_ignored_tokens = ("comment_begin", "comment", "comment_end", "whitespace", "linecomment_begin", "linecomment_end", "linecomment")
_ignore_if_empty = ("whitespace", "data", "comment", "linecomment")
_TOKEN_DESC = {
    "comment_begin": "begin of comment", "comment_end": "end of comment", "comment": "comment",
    "linecomment": "comment", "block_begin": "begin of statement block", "block_end": "end of statement block",
    "variable_begin": "begin of print statement", "variable_end": "end of print statement",
    "linestatement_begin": "begin of line statement", "linestatement_end": "end of line statement",
    "data": "template data / text", "eof": "end of template",
}


def _describe_token_type(token_type):
    if token_type in _reverse_operators:
        return _reverse_operators[token_type]
    return _TOKEN_DESC.get(token_type, token_type)


def _describe_token(token):
    if token.type == "name":
        return token.value
    return _describe_token_type(token.type)


def _describe_token_expr(expr):
    if ":" in expr:
        type_, value = expr.split(":", 1)
        if type_ == "name":
            return value
    else:
        type_ = expr
    return _describe_token_type(type_)


class Token:
    def __init__(self, lineno, type, value):
        self.lineno = lineno
        self.type = type
        self.value = value

    def __str__(self):
        return _describe_token(self)

    def __repr__(self):
        return "Token(lineno=" + repr(self.lineno) + ", type=" + repr(self.type) + ", value=" + repr(self.value) + ")"

    def test(self, expr):
        if self.type == expr:
            return True
        if ":" in expr:
            return expr.split(":", 1) == [self.type, self.value]
        return False

    def test_any(self, *iterable):
        for expr in iterable:
            if self.test(expr):
                return True
        return False


class _Failure:
    def __init__(self, message):
        self.message = message


class TokenStream:
    def __init__(self, tokens, name, filename):
        self._tokens = tokens
        self._pos = 0
        self._pushed = []
        self.name = name
        self.filename = filename
        self.closed = False
        self.current = Token(1, "initial", "")
        self._advance()

    def __bool__(self):
        return bool(self._pushed) or self.current.type != "eof"

    @property
    def eos(self):
        return not self.__bool__()

    def push(self, token):
        self._pushed.append(token)

    def look(self):
        old_token = self._advance()
        result = self.current
        self.push(result)
        self.current = old_token
        return result

    def skip(self, n=1):
        for _ in range(n):
            self._advance()

    def next_if(self, expr):
        if self.current.test(expr):
            return self._advance()
        return None

    def skip_if(self, expr):
        return self.next_if(expr) is not None

    def __next__(self):
        return self._advance()

    def _advance(self):
        rv = self.current
        if self._pushed:
            self.current = self._pushed.pop(0)
        elif self.current.type != "eof":
            if self._pos < len(self._tokens):
                self.current = self._tokens[self._pos]
                self._pos += 1
            else:
                self.close()
        return rv

    def close(self):
        self.current = Token(self.current.lineno, "eof", "")
        self._tokens = []
        self._pos = 0
        self.closed = True

    def expect(self, expr):
        if not self.current.test(expr):
            expr = _describe_token_expr(expr)
            if self.current.type == "eof":
                raise TemplateSyntaxError("unexpected end of template, expected " + repr(expr) + ".", self.current.lineno, self.name, self.filename)
            raise TemplateSyntaxError("expected token " + repr(expr) + ", got " + repr(_describe_token(self.current)), self.current.lineno, self.name, self.filename)
        return self._advance()


def _c(x):
    return re.compile(x, re.M | re.S)


_ESCAPES = {"n": "\n", "t": "\t", "r": "\r", "\\": "\\", "'": "'", '"': '"', "a": "\x07", "b": "\x08", "f": "\x0c", "v": "\x0b", "0": "\x00", "\n": ""}


def _unescape_string(s):
    # str.encode("ascii", "backslashreplace").decode("unicode-escape"), as Jinja reads string literals
    if "\\" not in s:
        return s
    out = []
    i = 0
    n = len(s)
    while i < n:
        c = s[i]
        if c != "\\":
            out.append(c)
            i += 1
            continue
        if i + 1 >= n:
            raise ValueError("\\ at end of string")
        d = s[i + 1]
        if d in "01234567":
            j = i + 1
            while j < n and j < i + 4 and s[j] in "01234567":
                j += 1
            out.append(chr(int(s[i + 1:j], 8)))
            i = j
        elif d in _ESCAPES:
            out.append(_ESCAPES[d])
            i += 2
        elif d == "x" or d == "u" or d == "U":
            k = {"x": 2, "u": 4, "U": 8}[d]
            hexs = s[i + 2:i + 2 + k]
            if len(hexs) < k or any([h not in "0123456789abcdefABCDEF" for h in hexs]):
                raise ValueError("truncated \\" + ("xXX" if d == "x" else ("uXXXX" if d == "u" else "UXXXXXXXX")) + " escape")
            out.append(chr(int(hexs, 16)))
            i += 2 + k
        elif d == "N":
            _lacks("\\N{...} escapes in template strings")
        else:
            out.append("\\")
            out.append(d)
            i += 2
    return "".join(out)


class Lexer:
    def __init__(self, environment):
        e = re.escape
        tag_rules = [
            (_whitespace_re, "whitespace", None),
            (_float_re, "float", None),
            (_integer_re, "integer", None),
            (_name_re, "name", None),
            (_string_re, "string", None),
            (_operator_re, "operator", None),
        ]
        root_rules = [
            (len(environment.comment_start_string), "comment_begin", e(environment.comment_start_string)),
            (len(environment.block_start_string), "block_begin", e(environment.block_start_string)),
            (len(environment.variable_start_string), "variable_begin", e(environment.variable_start_string)),
        ]
        if environment.line_statement_prefix is not None:
            root_rules.append((len(environment.line_statement_prefix), "linestatement_begin", r"^[ \t\v]*" + e(environment.line_statement_prefix)))
        if environment.line_comment_prefix is not None:
            root_rules.append((len(environment.line_comment_prefix), "linecomment_begin", r"(?:^|(?<=\S))[^\S\r\n]*" + e(environment.line_comment_prefix)))
        root_rules = [x[1:] for x in sorted(root_rules, reverse=True)]
        block_start_re = e(environment.block_start_string)
        block_end_re = e(environment.block_end_string)
        comment_end_re = e(environment.comment_end_string)
        variable_end_re = e(environment.variable_end_string)
        block_suffix_re = "\\n?" if environment.trim_blocks else ""
        self.lstrip_blocks = environment.lstrip_blocks
        self.newline_sequence = environment.newline_sequence
        self.keep_trailing_newline = environment.keep_trailing_newline
        root_raw_re = "(?P<raw_begin>" + block_start_re + r"(\-|\+|)\s*raw\s*(?:\-" + block_end_re + r"\s*|" + block_end_re + "))"
        root_parts_re = "|".join([root_raw_re] + ["(?P<" + n + ">" + r + r"(\-|\+|))" for n, r in root_rules])
        # The names of the groups of the root rule, in order (Python's groupdict order)
        self.root_names = ["raw_begin"] + [n for n, r in root_rules]
        self.rules = {
            "root": [
                (_c("(.*?)(?:" + root_parts_re + ")"), ("#lstrip", "data", "#bygroup"), "#bygroup"),
                (_c(".+"), "data", None),
            ],
            "comment_begin": [
                (_c(r"(.*?)((?:\+" + comment_end_re + r"|\-" + comment_end_re + r"\s*|" + comment_end_re + block_suffix_re + "))"), ("comment", "comment_end"), "#pop"),
                (_c(r"(.)"), (_Failure("Missing end of comment tag"),), None),
            ],
            "block_begin": [
                (_c(r"(?:\+" + block_end_re + r"|\-" + block_end_re + r"\s*|" + block_end_re + block_suffix_re + ")"), "block_end", "#pop"),
            ] + tag_rules,
            "variable_begin": [
                (_c(r"\-" + variable_end_re + r"\s*|" + variable_end_re), "variable_end", "#pop"),
            ] + tag_rules,
            "raw_begin": [
                (_c(r"(.*?)((?:" + block_start_re + r"(\-|\+|))\s*endraw\s*(?:\+" + block_end_re + r"|\-" + block_end_re + r"\s*|" + block_end_re + block_suffix_re + "))"), ("#lstrip", "data", "raw_end"), "#pop"),
                (_c(r"(.)"), (_Failure("Missing end of raw directive"),), None),
            ],
            "linestatement_begin": [
                (_c(r"\s*(\n|$)"), "linestatement_end", "#pop"),
            ] + tag_rules,
            "linecomment_begin": [
                (_c(r"(.*?)()(?=\n|$)"), ("linecomment", "linecomment_end"), "#pop"),
            ],
        }

    def _normalize_newlines(self, value):
        return _newline_re.sub(self.newline_sequence, value)

    def tokenize(self, source, name=None, filename=None, state=None):
        stream = self.tokeniter(source, name, filename, state)
        return TokenStream(self.wrap(stream, name, filename), name, filename)

    def wrap(self, stream, name=None, filename=None):
        out = []
        for lineno, token, value_str in stream:
            if token in _ignored_tokens:
                continue
            value = value_str
            if token == "linestatement_begin":
                token = "block_begin"
            elif token == "linestatement_end":
                token = "block_end"
            elif token == "raw_begin" or token == "raw_end":
                continue
            elif token == "data":
                value = self._normalize_newlines(value_str)
            elif token == "keyword":
                token = value_str
            elif token == "name":
                value = value_str
                if not value.isidentifier():
                    raise TemplateSyntaxError("Invalid character in identifier", lineno, name, filename)
            elif token == "string":
                try:
                    value = _unescape_string(self._normalize_newlines(value_str[1:-1]))
                except ValueError as e:
                    msg = str(e).split(":")[-1].strip()
                    raise TemplateSyntaxError(msg, lineno, name, filename) from None
            elif token == "integer":
                value = int(value_str.replace("_", ""), 0)
            elif token == "float":
                value = float(value_str.replace("_", ""))
            elif token == "operator":
                token = _operators[value_str]
            out.append(Token(lineno, token, value))
        return out

    def tokeniter(self, source, name, filename=None, state=None):
        out = []
        lines = _newline_re.split(source)[::2]
        if not self.keep_trailing_newline and lines[-1] == "":
            del lines[-1]
        source = "\n".join(lines)
        pos = 0
        lineno = 1
        stack = ["root"]
        if state is not None and state != "root":
            stack.append(state + "_begin")
        statetokens = self.rules[stack[-1]]
        source_length = len(source)
        balancing_stack = []
        newlines_stripped = 0
        line_starting = True
        while True:
            matched = False
            for regex, tokens, new_state in statetokens:
                m = regex.match(source, pos)
                if m is None:
                    continue
                if balancing_stack and tokens in ("variable_end", "block_end", "linestatement_end"):
                    continue
                if isinstance(tokens, tuple):
                    groups = list(m.groups())
                    toks = tokens
                    if tokens[0] == "#lstrip":
                        toks = tokens[1:]
                        text = groups[0]
                        strip_sign = None
                        for g in groups[2::2]:
                            if g is not None:
                                strip_sign = g
                                break
                        if strip_sign == "-":
                            stripped = text.rstrip()
                            newlines_stripped = text[len(stripped):].count("\n")
                            groups = [stripped] + groups[1:]
                        elif strip_sign != "+" and self.lstrip_blocks and not m.groupdict().get("variable_begin"):
                            l_pos = text.rfind("\n") + 1
                            if l_pos > 0 or line_starting:
                                if _whitespace_re.fullmatch(text, l_pos):
                                    groups = [text[:l_pos]] + groups[1:]
                    for idx in range(len(toks)):
                        token = toks[idx]
                        if isinstance(token, _Failure):
                            raise TemplateSyntaxError(token.message, lineno, name, filename)
                        elif token == "#bygroup":
                            gd = m.groupdict()
                            found = False
                            for key in self.root_names:
                                value = gd.get(key)
                                if value is not None:
                                    out.append((lineno, key, value))
                                    lineno += value.count("\n")
                                    found = True
                                    break
                            if not found:
                                raise RuntimeError("wanted to resolve the token dynamically but no group matched")
                        else:
                            data = groups[idx]
                            if data or token not in _ignore_if_empty:
                                out.append((lineno, token, data))
                            lineno += data.count("\n") + newlines_stripped
                            newlines_stripped = 0
                else:
                    data = m.group()
                    if tokens == "operator":
                        if data == "{":
                            balancing_stack.append("}")
                        elif data == "(":
                            balancing_stack.append(")")
                        elif data == "[":
                            balancing_stack.append("]")
                        elif data in ("}", ")", "]"):
                            if not balancing_stack:
                                raise TemplateSyntaxError("unexpected '" + data + "'", lineno, name, filename)
                            expected_op = balancing_stack.pop()
                            if expected_op != data:
                                raise TemplateSyntaxError("unexpected '" + data + "', expected '" + expected_op + "'", lineno, name, filename)
                    if data or tokens not in _ignore_if_empty:
                        out.append((lineno, tokens, data))
                    lineno += data.count("\n")
                line_starting = m.group()[-1:] == "\n"
                pos2 = m.end()
                if new_state is not None:
                    if new_state == "#pop":
                        stack.pop()
                    elif new_state == "#bygroup":
                        gd = m.groupdict()
                        found = False
                        for key in self.root_names:
                            if gd.get(key) is not None:
                                stack.append(key)
                                found = True
                                break
                        if not found:
                            raise RuntimeError("wanted to resolve the new state dynamically but no group matched")
                    else:
                        stack.append(new_state)
                    statetokens = self.rules[stack[-1]]
                elif pos2 == pos:
                    raise RuntimeError("yielded empty string without stack change")
                pos = pos2
                matched = True
                break
            if not matched:
                if pos >= source_length:
                    return out
                raise TemplateSyntaxError("unexpected char " + repr(source[pos]) + " at " + str(pos), lineno, name, filename)


_lexer_cache = {}


def get_lexer(environment):
    key = (environment.block_start_string, environment.block_end_string, environment.variable_start_string,
           environment.variable_end_string, environment.comment_start_string, environment.comment_end_string,
           environment.line_statement_prefix, environment.line_comment_prefix, environment.trim_blocks,
           environment.lstrip_blocks, environment.newline_sequence, environment.keep_trailing_newline)
    lexer = _lexer_cache.get(key)
    if lexer is None:
        lexer = Lexer(environment)
        _lexer_cache[key] = lexer
    return lexer


# -- nodes


class Node:
    fields = ()
    # the node compiled to a closure (see _compile_expr / _compile_stmt), made on its first render
    _x = None
    _s = None

    def __init__(self, *args, **kw):
        self.lineno = kw.get("lineno")
        fs = self.fields
        for i in range(len(fs)):
            if i < len(args):
                setattr(self, fs[i], args[i])
            elif fs[i] in kw:
                setattr(self, fs[i], kw[fs[i]])

    def can_assign(self):
        return False

    def set_ctx(self, ctx):
        for n in self._walk():
            if hasattr(n, "ctx"):
                n.ctx = ctx

    def iter_child_nodes(self):
        out = []
        for f in self.fields:
            v = getattr(self, f, None)
            if isinstance(v, Node):
                out.append(v)
            elif isinstance(v, list):
                for x in v:
                    if isinstance(x, Node):
                        out.append(x)
        return out

    def _walk(self):
        out = [self]
        todo = [self]
        while todo:
            n = todo.pop()
            for c in n.iter_child_nodes():
                out.append(c)
                todo.append(c)
        return out

    def find_all(self, cls):
        out = []
        self._find(cls, out)
        return out

    def _find(self, cls, out):
        for c in self.iter_child_nodes():
            if isinstance(c, cls):
                out.append(c)
            c._find(cls, out)

    def find(self, cls):
        r = self.find_all(cls)
        return r[0] if r else None


class Stmt(Node):
    pass


class Expr(Node):
    pass


class Helper(Node):
    pass


class TemplateNode(Node):
    fields = ("body",)


class Output(Stmt):
    fields = ("nodes",)


class Extends(Stmt):
    fields = ("template",)


class For(Stmt):
    fields = ("target", "iter", "body", "else_", "test", "recursive")


class If(Stmt):
    fields = ("test", "body", "elif_", "else_")


class MacroNode(Stmt):
    fields = ("name", "args", "defaults", "body")


class CallBlock(Stmt):
    fields = ("call", "args", "defaults", "body")


class FilterBlock(Stmt):
    fields = ("body", "filter")


class With(Stmt):
    fields = ("targets", "values", "body")


class Block(Stmt):
    fields = ("name", "body", "scoped", "required")


class Include(Stmt):
    fields = ("template", "with_context", "ignore_missing")


class Import(Stmt):
    fields = ("template", "target", "with_context")


class FromImport(Stmt):
    fields = ("template", "names", "with_context")


class ExprStmt(Stmt):
    fields = ("node",)


class Assign(Stmt):
    fields = ("target", "node")


class AssignBlock(Stmt):
    fields = ("target", "filter", "body")


class Name(Expr):
    fields = ("name", "ctx")

    def can_assign(self):
        return self.name not in ("true", "false", "none", "True", "False", "None")


class NSRef(Expr):
    fields = ("name", "attr")

    def can_assign(self):
        return True


class Literal(Expr):
    pass


class Const(Literal):
    fields = ("value",)


class TemplateData(Literal):
    fields = ("data",)


class Tuple(Literal):
    fields = ("items", "ctx")

    def can_assign(self):
        for item in self.items:
            if not item.can_assign():
                return False
        return True


class List(Literal):
    fields = ("items",)


class Dict(Literal):
    fields = ("items",)


class Pair(Helper):
    fields = ("key", "value")


class Keyword(Helper):
    fields = ("key", "value")


class CondExpr(Expr):
    fields = ("test", "expr1", "expr2")


class _FilterTestCommon(Expr):
    fields = ("node", "name", "args", "kwargs", "dyn_args", "dyn_kwargs")


class Filter(_FilterTestCommon):
    pass


class Test(_FilterTestCommon):
    pass


class Call(Expr):
    fields = ("node", "args", "kwargs", "dyn_args", "dyn_kwargs")


class Getitem(Expr):
    fields = ("node", "arg", "ctx")


class Getattr(Expr):
    fields = ("node", "attr", "ctx")


class Slice(Expr):
    fields = ("start", "stop", "step")


class Concat(Expr):
    fields = ("nodes",)


class Compare(Expr):
    fields = ("expr", "ops")


class Operand(Helper):
    fields = ("op", "expr")


class BinExpr(Expr):
    fields = ("left", "right")
    operator = ""


class Mul(BinExpr):
    operator = "*"


class Div(BinExpr):
    operator = "/"


class FloorDiv(BinExpr):
    operator = "//"


class Add(BinExpr):
    operator = "+"


class Sub(BinExpr):
    operator = "-"


class Mod(BinExpr):
    operator = "%"


class Pow(BinExpr):
    operator = "**"


class And(BinExpr):
    operator = "and"


class Or(BinExpr):
    operator = "or"


class UnaryExpr(Expr):
    fields = ("node",)


class Not(UnaryExpr):
    pass


class Neg(UnaryExpr):
    pass


class Pos(UnaryExpr):
    pass


class Continue(Stmt):
    pass


class Break(Stmt):
    pass


class Scope(Stmt):
    fields = ("body",)


class ScopedEvalContextModifier(Stmt):
    fields = ("options", "body")


# -- parser

_statement_keywords = ("for", "if", "block", "extends", "print", "macro", "include", "from", "import", "set", "with", "autoescape")
_compare_operators = ("eq", "ne", "lt", "lteq", "gt", "gteq")
_math_nodes = {"add": Add, "sub": Sub, "mul": Mul, "div": Div, "floordiv": FloorDiv, "mod": Mod}


class Parser:
    def __init__(self, environment, source, name=None, filename=None, state=None):
        self.environment = environment
        self.stream = environment._tokenize(source, name, filename, state)
        self.name = name
        self.filename = filename
        self.closed = False
        self.extensions = {}
        for tag in environment._ext_tags:
            self.extensions[tag] = tag
        self._last_identifier = 0
        self._tag_stack = []
        self._end_token_stack = []

    def fail(self, msg, lineno=None, exc=TemplateSyntaxError):
        if lineno is None:
            lineno = self.stream.current.lineno
        raise exc(msg, lineno, self.name, self.filename)

    def _fail_ut_eof(self, name, end_token_stack, lineno):
        expected = set()
        for exprs in end_token_stack:
            for e in exprs:
                expected.add(_describe_token_expr(e))
        if end_token_stack:
            currently_looking = " or ".join([repr(_describe_token_expr(e)) for e in end_token_stack[-1]])
        else:
            currently_looking = None
        if name is None:
            message = ["Unexpected end of template."]
        else:
            message = ["Encountered unknown tag " + repr(name) + "."]
        if currently_looking:
            if name is not None and name in expected:
                message.append("You probably made a nesting mistake. Jinja is expecting this tag, but currently looking for " + currently_looking + ".")
            else:
                message.append("Jinja was looking for the following tags: " + currently_looking + ".")
        if self._tag_stack:
            message.append("The innermost block that needs to be closed is " + repr(self._tag_stack[-1]) + ".")
        self.fail(" ".join(message), lineno)

    def fail_unknown_tag(self, name, lineno=None):
        self._fail_ut_eof(name, self._end_token_stack, lineno)

    def fail_eof(self, end_tokens=None, lineno=None):
        stack = list(self._end_token_stack)
        if end_tokens is not None:
            stack.append(end_tokens)
        self._fail_ut_eof(None, stack, lineno)

    def is_tuple_end(self, extra_end_rules=None):
        if self.stream.current.type in ("variable_end", "block_end", "rparen"):
            return True
        elif extra_end_rules is not None:
            return self.stream.current.test_any(*extra_end_rules)
        return False

    def parse_statement(self):
        token = self.stream.current
        if token.type != "name":
            self.fail("tag name expected", token.lineno)
        self._tag_stack.append(token.value)
        pop_tag = True
        try:
            if token.value in _statement_keywords:
                v = token.value
                if v == "for":
                    return self.parse_for()
                if v == "if":
                    return self.parse_if()
                if v == "block":
                    return self.parse_block()
                if v == "extends":
                    return self.parse_extends()
                if v == "print":
                    return self.parse_print()
                if v == "macro":
                    return self.parse_macro()
                if v == "include":
                    return self.parse_include()
                if v == "from":
                    return self.parse_from()
                if v == "import":
                    return self.parse_import()
                if v == "set":
                    return self.parse_set()
                if v == "with":
                    return self.parse_with()
                return self.parse_autoescape()
            if token.value == "call":
                return self.parse_call_block()
            if token.value == "filter":
                return self.parse_filter_block()
            ext = self.extensions.get(token.value)
            if ext is not None:
                return self.parse_extension(ext)
            self._tag_stack.pop()
            pop_tag = False
            self.fail_unknown_tag(token.value, token.lineno)
        finally:
            if pop_tag:
                self._tag_stack.pop()

    def parse_extension(self, tag):
        token = self.stream._advance()
        if tag == "break":
            return Break(lineno=token.lineno)
        if tag == "continue":
            return Continue(lineno=token.lineno)
        if tag == "do":
            return ExprStmt(self.parse_tuple(), lineno=token.lineno)
        _lacks("the {% " + tag + " %} tag")

    def parse_statements(self, end_tokens, drop_needle=False):
        self.stream.skip_if("colon")
        self.stream.expect("block_end")
        result = self.subparse(end_tokens)
        if self.stream.current.type == "eof":
            self.fail_eof(end_tokens)
        if drop_needle:
            self.stream._advance()
        return result

    def parse_set(self):
        lineno = self.stream._advance().lineno
        target = self.parse_assign_target(with_namespace=True)
        if self.stream.skip_if("assign"):
            expr = self.parse_tuple()
            return Assign(target, expr, lineno=lineno)
        filter_node = self.parse_filter(None)
        body = self.parse_statements(("name:endset",), drop_needle=True)
        return AssignBlock(target, filter_node, body, lineno=lineno)

    def parse_for(self):
        lineno = self.stream.expect("name:for").lineno
        target = self.parse_assign_target(extra_end_rules=("name:in",))
        self.stream.expect("name:in")
        iter = self.parse_tuple(with_condexpr=False, extra_end_rules=("name:recursive",))
        test = None
        if self.stream.skip_if("name:if"):
            test = self.parse_expression()
        recursive = self.stream.skip_if("name:recursive")
        body = self.parse_statements(("name:endfor", "name:else"))
        if self.stream._advance().value == "endfor":
            else_ = []
        else:
            else_ = self.parse_statements(("name:endfor",), drop_needle=True)
        return For(target, iter, body, else_, test, recursive, lineno=lineno)

    def parse_if(self):
        node = result = If(lineno=self.stream.expect("name:if").lineno)
        while True:
            node.test = self.parse_tuple(with_condexpr=False)
            node.body = self.parse_statements(("name:elif", "name:else", "name:endif"))
            node.elif_ = []
            node.else_ = []
            token = self.stream._advance()
            if token.test("name:elif"):
                node = If(lineno=self.stream.current.lineno)
                result.elif_.append(node)
                continue
            elif token.test("name:else"):
                result.else_ = self.parse_statements(("name:endif",), drop_needle=True)
            break
        return result

    def parse_with(self):
        node = With(lineno=self.stream._advance().lineno)
        targets = []
        values = []
        while self.stream.current.type != "block_end":
            if targets:
                self.stream.expect("comma")
            target = self.parse_assign_target()
            target.set_ctx("param")
            targets.append(target)
            self.stream.expect("assign")
            values.append(self.parse_expression())
        node.targets = targets
        node.values = values
        node.body = self.parse_statements(("name:endwith",), drop_needle=True)
        return node

    def parse_autoescape(self):
        node = ScopedEvalContextModifier(lineno=self.stream._advance().lineno)
        node.options = [Keyword("autoescape", self.parse_expression())]
        node.body = self.parse_statements(("name:endautoescape",), drop_needle=True)
        return Scope([node])

    def parse_block(self):
        node = Block(lineno=self.stream._advance().lineno)
        node.name = self.stream.expect("name").value
        node.scoped = self.stream.skip_if("name:scoped")
        node.required = self.stream.skip_if("name:required")
        if self.stream.current.type == "sub":
            self.fail("Block names in Jinja have to be valid Python identifiers and may not contain hyphens, use an underscore instead.")
        node.body = self.parse_statements(("name:endblock",), drop_needle=True)
        if node.required:
            for body_node in node.body:
                bad = not isinstance(body_node, Output)
                if not bad:
                    for output_node in body_node.nodes:
                        if not isinstance(output_node, TemplateData) or not output_node.data.isspace():
                            bad = True
                if bad:
                    self.fail("Required blocks can only contain comments or whitespace")
        self.stream.skip_if("name:" + node.name)
        return node

    def parse_extends(self):
        node = Extends(lineno=self.stream._advance().lineno)
        node.template = self.parse_expression()
        return node

    def parse_import_context(self, node, default):
        if self.stream.current.test_any("name:with", "name:without") and self.stream.look().test("name:context"):
            node.with_context = self.stream._advance().value == "with"
            self.stream.skip()
        else:
            node.with_context = default
        return node

    def parse_include(self):
        node = Include(lineno=self.stream._advance().lineno)
        node.template = self.parse_expression()
        if self.stream.current.test("name:ignore") and self.stream.look().test("name:missing"):
            node.ignore_missing = True
            self.stream.skip(2)
        else:
            node.ignore_missing = False
        return self.parse_import_context(node, True)

    def parse_import(self):
        node = Import(lineno=self.stream._advance().lineno)
        node.template = self.parse_expression()
        self.stream.expect("name:as")
        node.target = self.parse_assign_target(name_only=True).name
        return self.parse_import_context(node, False)

    def _from_context(self, node):
        if self.stream.current.value in ("with", "without") and self.stream.look().test("name:context"):
            node.with_context = self.stream._advance().value == "with"
            self.stream.skip()
            return True
        return False

    def parse_from(self):
        node = FromImport(lineno=self.stream._advance().lineno)
        node.template = self.parse_expression()
        self.stream.expect("name:import")
        node.names = []
        node.with_context = None
        while True:
            if node.names:
                self.stream.expect("comma")
            if self.stream.current.type == "name":
                if self._from_context(node):
                    break
                target = self.parse_assign_target(name_only=True)
                if target.name.startswith("_"):
                    self.fail("names starting with an underline can not be imported", target.lineno, exc=TemplateAssertionError)
                if self.stream.skip_if("name:as"):
                    alias = self.parse_assign_target(name_only=True)
                    node.names.append((target.name, alias.name))
                else:
                    node.names.append(target.name)
                if self._from_context(node) or self.stream.current.type != "comma":
                    break
            else:
                self.stream.expect("name")
        if node.with_context is None:
            node.with_context = False
        return node

    def parse_signature(self, node):
        args = node.args = []
        defaults = node.defaults = []
        self.stream.expect("lparen")
        while self.stream.current.type != "rparen":
            if args:
                self.stream.expect("comma")
            arg = self.parse_assign_target(name_only=True)
            arg.set_ctx("param")
            if self.stream.skip_if("assign"):
                defaults.append(self.parse_expression())
            elif defaults:
                self.fail("non-default argument follows default argument")
            args.append(arg)
        self.stream.expect("rparen")

    def parse_call_block(self):
        node = CallBlock(lineno=self.stream._advance().lineno)
        if self.stream.current.type == "lparen":
            self.parse_signature(node)
        else:
            node.args = []
            node.defaults = []
        call_node = self.parse_expression()
        if not isinstance(call_node, Call):
            self.fail("expected call", node.lineno)
        node.call = call_node
        node.body = self.parse_statements(("name:endcall",), drop_needle=True)
        return node

    def parse_filter_block(self):
        node = FilterBlock(lineno=self.stream._advance().lineno)
        node.filter = self.parse_filter(None, start_inline=True)
        node.body = self.parse_statements(("name:endfilter",), drop_needle=True)
        return node

    def parse_macro(self):
        node = MacroNode(lineno=self.stream._advance().lineno)
        node.name = self.parse_assign_target(name_only=True).name
        self.parse_signature(node)
        node.body = self.parse_statements(("name:endmacro",), drop_needle=True)
        return node

    def parse_print(self):
        node = Output(lineno=self.stream._advance().lineno)
        node.nodes = []
        while self.stream.current.type != "block_end":
            if node.nodes:
                self.stream.expect("comma")
            node.nodes.append(self.parse_expression())
        return node

    def parse_assign_target(self, with_tuple=True, name_only=False, extra_end_rules=None, with_namespace=False):
        if name_only:
            token = self.stream.expect("name")
            target = Name(token.value, "store", lineno=token.lineno)
        else:
            if with_tuple:
                target = self.parse_tuple(simplified=True, extra_end_rules=extra_end_rules, with_namespace=with_namespace)
            else:
                target = self.parse_primary(with_namespace=with_namespace)
            target.set_ctx("store")
        if not target.can_assign():
            self.fail("can't assign to " + repr(type(target).__name__.lower()), target.lineno)
        return target

    def parse_expression(self, with_condexpr=True):
        if with_condexpr:
            return self.parse_condexpr()
        return self.parse_or()

    def parse_condexpr(self):
        lineno = self.stream.current.lineno
        expr1 = self.parse_or()
        while self.stream.skip_if("name:if"):
            expr2 = self.parse_or()
            if self.stream.skip_if("name:else"):
                expr3 = self.parse_condexpr()
            else:
                expr3 = None
            expr1 = CondExpr(expr2, expr1, expr3, lineno=lineno)
            lineno = self.stream.current.lineno
        return expr1

    def parse_or(self):
        lineno = self.stream.current.lineno
        left = self.parse_and()
        while self.stream.skip_if("name:or"):
            right = self.parse_and()
            left = Or(left, right, lineno=lineno)
            lineno = self.stream.current.lineno
        return left

    def parse_and(self):
        lineno = self.stream.current.lineno
        left = self.parse_not()
        while self.stream.skip_if("name:and"):
            right = self.parse_not()
            left = And(left, right, lineno=lineno)
            lineno = self.stream.current.lineno
        return left

    def parse_not(self):
        if self.stream.current.test("name:not"):
            lineno = self.stream._advance().lineno
            return Not(self.parse_not(), lineno=lineno)
        return self.parse_compare()

    def parse_compare(self):
        lineno = self.stream.current.lineno
        expr = self.parse_math1()
        ops = []
        while True:
            token_type = self.stream.current.type
            if token_type in _compare_operators:
                self.stream._advance()
                ops.append(Operand(token_type, self.parse_math1()))
            elif self.stream.skip_if("name:in"):
                ops.append(Operand("in", self.parse_math1()))
            elif self.stream.current.test("name:not") and self.stream.look().test("name:in"):
                self.stream.skip(2)
                ops.append(Operand("notin", self.parse_math1()))
            else:
                break
            lineno = self.stream.current.lineno
        if not ops:
            return expr
        return Compare(expr, ops, lineno=lineno)

    def parse_math1(self):
        lineno = self.stream.current.lineno
        left = self.parse_concat()
        while self.stream.current.type in ("add", "sub"):
            cls = _math_nodes[self.stream.current.type]
            self.stream._advance()
            right = self.parse_concat()
            left = cls(left, right, lineno=lineno)
            lineno = self.stream.current.lineno
        return left

    def parse_concat(self):
        lineno = self.stream.current.lineno
        args = [self.parse_math2()]
        while self.stream.current.type == "tilde":
            self.stream._advance()
            args.append(self.parse_math2())
        if len(args) == 1:
            return args[0]
        return Concat(args, lineno=lineno)

    def parse_math2(self):
        lineno = self.stream.current.lineno
        left = self.parse_pow()
        while self.stream.current.type in ("mul", "div", "floordiv", "mod"):
            cls = _math_nodes[self.stream.current.type]
            self.stream._advance()
            right = self.parse_pow()
            left = cls(left, right, lineno=lineno)
            lineno = self.stream.current.lineno
        return left

    def parse_pow(self):
        lineno = self.stream.current.lineno
        left = self.parse_unary()
        while self.stream.current.type == "pow":
            self.stream._advance()
            right = self.parse_unary()
            left = Pow(left, right, lineno=lineno)
            lineno = self.stream.current.lineno
        return left

    def parse_unary(self, with_filter=True):
        token_type = self.stream.current.type
        lineno = self.stream.current.lineno
        if token_type == "sub":
            self.stream._advance()
            node = Neg(self.parse_unary(False), lineno=lineno)
        elif token_type == "add":
            self.stream._advance()
            node = Pos(self.parse_unary(False), lineno=lineno)
        else:
            node = self.parse_primary()
        node = self.parse_postfix(node)
        if with_filter:
            node = self.parse_filter_expr(node)
        return node

    def parse_primary(self, with_namespace=False):
        token = self.stream.current
        if token.type == "name":
            self.stream._advance()
            if token.value in ("true", "false", "True", "False"):
                node = Const(token.value in ("true", "True"), lineno=token.lineno)
            elif token.value in ("none", "None"):
                node = Const(None, lineno=token.lineno)
            elif with_namespace and self.stream.current.type == "dot":
                self.stream._advance()
                attr = self.stream.expect("name")
                node = NSRef(token.value, attr.value, lineno=token.lineno)
            else:
                node = Name(token.value, "load", lineno=token.lineno)
        elif token.type == "string":
            self.stream._advance()
            buf = [token.value]
            lineno = token.lineno
            while self.stream.current.type == "string":
                buf.append(self.stream.current.value)
                self.stream._advance()
            node = Const("".join(buf), lineno=lineno)
        elif token.type in ("integer", "float"):
            self.stream._advance()
            node = Const(token.value, lineno=token.lineno)
        elif token.type == "lparen":
            self.stream._advance()
            node = self.parse_tuple(explicit_parentheses=True)
            self.stream.expect("rparen")
        elif token.type == "lbracket":
            node = self.parse_list()
        elif token.type == "lbrace":
            node = self.parse_dict()
        else:
            self.fail("unexpected " + repr(_describe_token(token)), token.lineno)
        return node

    def parse_tuple(self, simplified=False, with_condexpr=True, extra_end_rules=None, explicit_parentheses=False, with_namespace=False):
        lineno = self.stream.current.lineno
        args = []
        is_tuple = False
        while True:
            if args:
                self.stream.expect("comma")
            if self.is_tuple_end(extra_end_rules):
                break
            if simplified:
                args.append(self.parse_primary(with_namespace=with_namespace))
            else:
                args.append(self.parse_expression(with_condexpr=with_condexpr))
            if self.stream.current.type == "comma":
                is_tuple = True
            else:
                break
            lineno = self.stream.current.lineno
        if not is_tuple:
            if args:
                return args[0]
            if not explicit_parentheses:
                self.fail("Expected an expression, got " + repr(_describe_token(self.stream.current)))
        return Tuple(args, "load", lineno=lineno)

    def parse_list(self):
        token = self.stream.expect("lbracket")
        items = []
        while self.stream.current.type != "rbracket":
            if items:
                self.stream.expect("comma")
            if self.stream.current.type == "rbracket":
                break
            items.append(self.parse_expression())
        self.stream.expect("rbracket")
        return List(items, lineno=token.lineno)

    def parse_dict(self):
        token = self.stream.expect("lbrace")
        items = []
        while self.stream.current.type != "rbrace":
            if items:
                self.stream.expect("comma")
            if self.stream.current.type == "rbrace":
                break
            key = self.parse_expression()
            self.stream.expect("colon")
            value = self.parse_expression()
            items.append(Pair(key, value, lineno=key.lineno))
        self.stream.expect("rbrace")
        return Dict(items, lineno=token.lineno)

    def parse_postfix(self, node):
        while True:
            token_type = self.stream.current.type
            if token_type == "dot" or token_type == "lbracket":
                node = self.parse_subscript(node)
            elif token_type == "lparen":
                node = self.parse_call(node)
            else:
                break
        return node

    def parse_filter_expr(self, node):
        while True:
            token_type = self.stream.current.type
            if token_type == "pipe":
                node = self.parse_filter(node)
            elif token_type == "name" and self.stream.current.value == "is":
                node = self.parse_test(node)
            elif token_type == "lparen":
                node = self.parse_call(node)
            else:
                break
        return node

    def parse_subscript(self, node):
        token = self.stream._advance()
        if token.type == "dot":
            attr_token = self.stream.current
            self.stream._advance()
            if attr_token.type == "name":
                return Getattr(node, attr_token.value, "load", lineno=token.lineno)
            elif attr_token.type != "integer":
                self.fail("expected name or number", attr_token.lineno)
            arg = Const(attr_token.value, lineno=attr_token.lineno)
            return Getitem(node, arg, "load", lineno=token.lineno)
        if token.type == "lbracket":
            args = []
            while self.stream.current.type != "rbracket":
                if args:
                    self.stream.expect("comma")
                args.append(self.parse_subscribed())
            self.stream.expect("rbracket")
            if len(args) == 1:
                arg = args[0]
            else:
                arg = Tuple(args, "load", lineno=token.lineno)
            return Getitem(node, arg, "load", lineno=token.lineno)
        self.fail("expected subscript expression", token.lineno)

    def parse_subscribed(self):
        lineno = self.stream.current.lineno
        if self.stream.current.type == "colon":
            self.stream._advance()
            args = [None]
        else:
            node = self.parse_expression()
            if self.stream.current.type != "colon":
                return node
            self.stream._advance()
            args = [node]
        if self.stream.current.type == "colon":
            args.append(None)
        elif self.stream.current.type not in ("rbracket", "comma"):
            args.append(self.parse_expression())
        else:
            args.append(None)
        if self.stream.current.type == "colon":
            self.stream._advance()
            if self.stream.current.type not in ("rbracket", "comma"):
                args.append(self.parse_expression())
            else:
                args.append(None)
        else:
            args.append(None)
        return Slice(args[0], args[1], args[2], lineno=lineno)

    def parse_call_args(self):
        token = self.stream.expect("lparen")
        args = []
        kwargs = []
        dyn_args = None
        dyn_kwargs = None
        require_comma = False
        while self.stream.current.type != "rparen":
            if require_comma:
                self.stream.expect("comma")
                if self.stream.current.type == "rparen":
                    break
            if self.stream.current.type == "mul":
                if not (dyn_args is None and dyn_kwargs is None):
                    self.fail("invalid syntax for function call expression", token.lineno)
                self.stream._advance()
                dyn_args = self.parse_expression()
            elif self.stream.current.type == "pow":
                if not (dyn_kwargs is None):
                    self.fail("invalid syntax for function call expression", token.lineno)
                self.stream._advance()
                dyn_kwargs = self.parse_expression()
            else:
                if self.stream.current.type == "name" and self.stream.look().type == "assign":
                    if not (dyn_kwargs is None):
                        self.fail("invalid syntax for function call expression", token.lineno)
                    key = self.stream.current.value
                    self.stream.skip(2)
                    value = self.parse_expression()
                    kwargs.append(Keyword(key, value, lineno=value.lineno))
                else:
                    if not (dyn_args is None and dyn_kwargs is None and not kwargs):
                        self.fail("invalid syntax for function call expression", token.lineno)
                    args.append(self.parse_expression())
            require_comma = True
        self.stream.expect("rparen")
        return args, kwargs, dyn_args, dyn_kwargs

    def parse_call(self, node):
        token = self.stream.current
        args, kwargs, dyn_args, dyn_kwargs = self.parse_call_args()
        return Call(node, args, kwargs, dyn_args, dyn_kwargs, lineno=token.lineno)

    def parse_filter(self, node, start_inline=False):
        while self.stream.current.type == "pipe" or start_inline:
            if not start_inline:
                self.stream._advance()
            token = self.stream.expect("name")
            name = token.value
            while self.stream.current.type == "dot":
                self.stream._advance()
                name += "." + self.stream.expect("name").value
            if self.stream.current.type == "lparen":
                args, kwargs, dyn_args, dyn_kwargs = self.parse_call_args()
            else:
                args = []
                kwargs = []
                dyn_args = dyn_kwargs = None
            node = Filter(node, name, args, kwargs, dyn_args, dyn_kwargs, lineno=token.lineno)
            start_inline = False
        return node

    def parse_test(self, node):
        token = self.stream._advance()
        if self.stream.current.test("name:not"):
            self.stream._advance()
            negated = True
        else:
            negated = False
        name = self.stream.expect("name").value
        while self.stream.current.type == "dot":
            self.stream._advance()
            name += "." + self.stream.expect("name").value
        dyn_args = dyn_kwargs = None
        kwargs = []
        if self.stream.current.type == "lparen":
            args, kwargs, dyn_args, dyn_kwargs = self.parse_call_args()
        elif self.stream.current.type in ("name", "string", "integer", "float", "lparen", "lbracket", "lbrace") and not self.stream.current.test_any("name:else", "name:or", "name:and"):
            if self.stream.current.test("name:is"):
                self.fail("You cannot chain multiple tests with is")
            arg_node = self.parse_primary()
            arg_node = self.parse_postfix(arg_node)
            args = [arg_node]
        else:
            args = []
        node = Test(node, name, args, kwargs, dyn_args, dyn_kwargs, lineno=token.lineno)
        if negated:
            node = Not(node, lineno=token.lineno)
        return node

    def subparse(self, end_tokens=None):
        body = []
        data_buffer = []
        if end_tokens is not None:
            self._end_token_stack.append(end_tokens)
        try:
            while self.stream:
                token = self.stream.current
                if token.type == "data":
                    if token.value:
                        data_buffer.append(TemplateData(token.value, lineno=token.lineno))
                    self.stream._advance()
                elif token.type == "variable_begin":
                    self.stream._advance()
                    data_buffer.append(self.parse_tuple(with_condexpr=True))
                    self.stream.expect("variable_end")
                elif token.type == "block_begin":
                    if data_buffer:
                        body.append(Output(data_buffer[:], lineno=data_buffer[0].lineno))
                        data_buffer = []
                    self.stream._advance()
                    if end_tokens is not None and self.stream.current.test_any(*end_tokens):
                        return body
                    rv = self.parse_statement()
                    if isinstance(rv, list):
                        body.extend(rv)
                    else:
                        body.append(rv)
                    self.stream.expect("block_end")
                else:
                    raise AssertionError("internal parsing error")
            if data_buffer:
                body.append(Output(data_buffer[:], lineno=data_buffer[0].lineno))
                data_buffer = []
        finally:
            if end_tokens is not None:
                self._end_token_stack.pop()
        return body

    def parse(self):
        return TemplateNode(self.subparse(), lineno=1)


# -- runtime


class EvalContext:
    def __init__(self, environment, template_name=None):
        self.environment = environment
        if callable(environment.autoescape):
            self.autoescape = environment.autoescape(template_name)
        else:
            self.autoescape = environment.autoescape
        self.volatile = False

    def save(self):
        return (self.autoescape, self.volatile)

    def revert(self, old):
        self.autoescape = old[0]
        self.volatile = old[1]


def markup_join(seq):
    buf = []
    html = False
    for arg in seq:
        s = soft_str(arg)
        buf.append(s)
        if hasattr(s, "__html__"):
            html = True
    if html:
        return Markup("").join(buf)
    return "".join(buf)


def str_join(seq):
    return "".join([str(x) for x in seq])


def _concat(buf):
    return "".join(buf)


class _ChainMap:
    def __init__(self, *maps):
        self.maps = list(maps) or [{}]

    def __getitem__(self, key):
        for m in self.maps:
            if key in m:
                return m[key]
        raise KeyError(key)

    def get(self, key, default=None):
        for m in self.maps:
            if key in m:
                return m[key]
        return default

    def __contains__(self, key):
        for m in self.maps:
            if key in m:
                return True
        return False

    def __setitem__(self, key, value):
        self.maps[0][key] = value

    def __delitem__(self, key):
        del self.maps[0][key]

    def keys(self):
        seen = {}
        for m in reversed(self.maps):
            for k in m:
                seen[k] = True
        return list(seen.keys())

    def to_dict(self):
        # dict(self): the keys in keys() order, each with the value self[key] gives
        d = {}
        for m in reversed(self.maps):
            if type(m) is dict:
                d.update(m)
            else:
                for k in m.keys():
                    d[k] = m[k]
        return d

    def __iter__(self):
        return iter(self.keys())

    def __len__(self):
        return len(self.keys())

    def items(self):
        return [(k, self[k]) for k in self.keys()]

    def values(self):
        return [self[k] for k in self.keys()]

    def update(self, other=(), **kw):
        if hasattr(other, "keys"):
            for k in other.keys():
                self.maps[0][k] = other[k]
        else:
            for k, v in other:
                self.maps[0][k] = v
        for k, v in kw.items():
            self.maps[0][k] = v

    def copy(self):
        return _ChainMap(dict(self.maps[0]), *self.maps[1:])

    def __repr__(self):
        return "ChainMap(" + ", ".join([repr(m) for m in self.maps]) + ")"


def _todict(m):
    if m is None:
        return {}
    if isinstance(m, dict):
        return dict(m)
    if type(m) is _ChainMap:
        return m.to_dict()
    d = {}
    for k in m.keys():
        d[k] = m[k]
    return d


def new_context(environment, template_name, blocks, vars=None, shared=False, globals=None, locals=None):
    if vars is None:
        vars = {}
    if shared:
        parent = vars
    else:
        parent = _todict(globals)
        for k, v in vars.items():
            parent[k] = v
    if locals:
        if shared:
            parent = dict(parent)
        for key, value in locals.items():
            if value is not missing:
                parent[key] = value
    return Context(environment, parent, template_name, blocks, globals=globals)


class Context:
    _gk = None

    def __init__(self, environment, parent, name, blocks, globals=None):
        self.parent = parent
        self.vars = {}
        self.environment = environment
        self.eval_ctx = EvalContext(self.environment, name)
        self.exported_vars = set()
        self.name = name
        self._globals = globals
        self.blocks = {}
        for k, v in blocks.items():
            self.blocks[k] = [v]

    @property
    def globals_keys(self):
        # made when first asked for (only imports do), not for every render
        gk = self._gk
        if gk is None:
            gk = set() if self._globals is None else set(self._globals.keys())
            self._gk = gk
        return gk

    def super(self, name, current):
        try:
            blocks = self.blocks[name]
            index = _index_of(blocks, current) + 1
            blocks[index]
        except LookupError:
            return self.environment.undefined("there is no parent block called " + repr(name) + ".", name="super")
        return BlockReference(name, self, blocks, index)

    def get(self, key, default=None):
        try:
            return self[key]
        except KeyError:
            return default

    def resolve(self, key):
        rv = self.resolve_or_missing(key)
        if rv is missing:
            return self.environment.undefined(name=key)
        return rv

    def resolve_or_missing(self, key):
        if key in self.vars:
            return self.vars[key]
        if key in self.parent:
            return self.parent[key]
        return missing

    def get_exported(self):
        return {k: self.vars[k] for k in self.exported_vars}

    def get_all(self):
        if not self.vars:
            return self.parent
        if not self.parent:
            return self.vars
        d = dict(self.parent)
        for k, v in self.vars.items():
            d[k] = v
        return d

    def call(self, obj, *args, **kwargs):
        if isinstance(obj, Macro):
            try:
                return obj._call(self.eval_ctx.autoescape, args, kwargs)
            except StopIteration:
                return self.environment.undefined("value was undefined because a callable raised a StopIteration exception")
        pass_arg = _pass_arg(obj)
        if pass_arg is None and not isinstance(obj, Undefined):
            c = None
            try:
                c = getattr(obj, "__call__", None)
            except Exception:
                c = None
            if c is not None and _pass_arg(c) is not None:
                obj = c
                pass_arg = _pass_arg(c)
        if pass_arg == _PASS_CONTEXT:
            args = (self,) + args
        elif pass_arg == _PASS_EVAL_CONTEXT:
            args = (self.eval_ctx,) + args
        elif pass_arg == _PASS_ENVIRONMENT:
            args = (self.environment,) + args
        try:
            return obj(*args, **kwargs)
        except StopIteration:
            return self.environment.undefined("value was undefined because a callable raised a StopIteration exception")

    def derived(self, locals=None):
        context = new_context(self.environment, self.name, {}, self.get_all(), True, None, locals)
        context.eval_ctx = self.eval_ctx
        for k, v in self.blocks.items():
            context.blocks[k] = list(v)
        return context

    def keys(self):
        return self.get_all().keys()

    def values(self):
        return self.get_all().values()

    def items(self):
        return self.get_all().items()

    def __contains__(self, name):
        return name in self.vars or name in self.parent

    def __getitem__(self, key):
        item = self.resolve_or_missing(key)
        if item is missing:
            raise KeyError(key)
        return item

    def __repr__(self):
        return "<Context " + repr(self.get_all()) + " of " + repr(self.name) + ">"


def _index_of(seq, item):
    for i in range(len(seq)):
        if seq[i] is item:
            return i
    raise ValueError("not in list")


class TemplateReference:
    def __init__(self, context):
        self._context = context

    def __getitem__(self, name):
        blocks = self._context.blocks[name]
        return BlockReference(name, self._context, blocks, 0)

    def __repr__(self):
        return "<TemplateReference " + repr(self._context.name) + ">"


class BlockReference:
    def __init__(self, name, context, stack, depth):
        self.name = name
        self._context = context
        self._stack = stack
        self._depth = depth

    @property
    def super(self):
        if self._depth + 1 >= len(self._stack):
            return self._context.environment.undefined("there is no parent block called " + repr(self.name) + ".", name="super")
        return BlockReference(self.name, self._context, self._stack, self._depth + 1)

    def __call__(self):
        rv = _concat(self._stack[self._depth].render(self._context))
        if self._context.eval_ctx.autoescape:
            return Markup(rv)
        return rv


class LoopContext:
    _length = None
    _after = missing
    _current = missing
    _before = missing
    _last_changed_value = missing

    def __init__(self, iterable, undefined, recurse=None, depth0=0):
        self._iterable = iterable
        self._iterator = iter(iterable)
        self._undefined = undefined
        self._recurse = recurse
        self.depth0 = depth0
        self.index0 = -1

    @property
    def length(self):
        if self._length is not None:
            return self._length
        try:
            self._length = len(self._iterable)
        except TypeError:
            iterable = list(self._iterator)
            self._iterator = iter(iterable)
            self._length = len(iterable) + self.index + (self._after is not missing)
        return self._length

    def __len__(self):
        return self.length

    @property
    def depth(self):
        return self.depth0 + 1

    @property
    def index(self):
        return self.index0 + 1

    @property
    def revindex0(self):
        return self.length - self.index

    @property
    def revindex(self):
        return self.length - self.index0

    @property
    def first(self):
        return self.index0 == 0

    def _peek_next(self):
        if self._after is not missing:
            return self._after
        self._after = next(self._iterator, missing)
        return self._after

    @property
    def last(self):
        return self._peek_next() is missing

    @property
    def previtem(self):
        if self.first:
            return self._undefined("there is no previous item")
        return self._before

    @property
    def nextitem(self):
        rv = self._peek_next()
        if rv is missing:
            return self._undefined("there is no next item")
        return rv

    def cycle(self, *args):
        if not args:
            raise TypeError("no items for cycling given")
        return args[self.index0 % len(args)]

    def changed(self, *value):
        if self._last_changed_value != value:
            self._last_changed_value = value
            return True
        return False

    def _next(self):
        if self._after is not missing:
            rv = self._after
            self._after = missing
        else:
            rv = next(self._iterator, missing)
            if rv is missing:
                return missing
        self.index0 += 1
        self._before = self._current
        self._current = rv
        return rv

    def __call__(self, iterable):
        if self._recurse is None:
            raise TypeError("The loop must have the 'recursive' marker to be called recursively.")
        return self._recurse(iterable, self.depth)

    def __repr__(self):
        return "<LoopContext " + str(self.index) + "/" + str(self.length) + ">"


class Macro:
    def __init__(self, environment, func, name, arguments, catch_kwargs, catch_varargs, caller, default_autoescape=None):
        self._environment = environment
        self._func = func
        self._argument_count = len(arguments)
        self.name = name
        self.arguments = arguments
        self.catch_kwargs = catch_kwargs
        self.catch_varargs = catch_varargs
        self.caller = caller
        self.explicit_caller = "caller" in arguments
        if default_autoescape is None:
            if callable(environment.autoescape):
                default_autoescape = environment.autoescape(None)
            else:
                default_autoescape = environment.autoescape
        self._default_autoescape = default_autoescape

    def __call__(self, *args, **kwargs):
        return self._call(self._default_autoescape, args, kwargs)

    def _call(self, autoescape, args, kwargs):
        kwargs = dict(kwargs)
        arguments = list(args[:self._argument_count])
        off = len(arguments)
        found_caller = False
        if off != self._argument_count:
            for name in self.arguments[len(arguments):]:
                if name in kwargs:
                    value = kwargs.pop(name)
                else:
                    value = missing
                if name == "caller":
                    found_caller = True
                arguments.append(value)
        else:
            found_caller = self.explicit_caller
        if self.caller and not found_caller:
            caller = kwargs.pop("caller", None)
            if caller is None:
                caller = self._environment.undefined("No caller defined", name="caller")
            arguments.append(caller)
        if self.catch_kwargs:
            arguments.append(kwargs)
        elif kwargs:
            if "caller" in kwargs:
                raise TypeError("macro " + repr(self.name) + " was invoked with two values for the special caller argument. This is most likely a bug.")
            raise TypeError("macro " + repr(self.name) + " takes no keyword argument " + repr(next(iter(kwargs))))
        if self.catch_varargs:
            arguments.append(tuple(args[self._argument_count:]))
        elif len(args) > self._argument_count:
            raise TypeError("macro " + repr(self.name) + " takes not more than " + str(len(self.arguments)) + " argument(s)")
        rv = self._func(arguments)
        if autoescape:
            rv = Markup(rv)
        return rv

    def __repr__(self):
        name = "anonymous" if self.name is None else repr(self.name)
        return "<Macro " + name + ">"


class TemplateModule:
    def __init__(self, template, context, body_stream=None):
        if body_stream is None:
            body_stream = list(template._render_root(context))
        self._body_stream = body_stream
        self.__name__ = template.name
        for k, v in context.get_exported().items():
            setattr(self, k, v)

    def __html__(self):
        return Markup(_concat(self._body_stream))

    def __str__(self):
        return _concat(self._body_stream)

    def __repr__(self):
        if self.__name__ is None:
            return "<TemplateModule memory:" + format(id(self), "x") + ">"
        return "<TemplateModule " + repr(self.__name__) + ">"


class _BreakLoop(Exception):
    pass


class _ContinueLoop(Exception):
    pass


# -- the interpreter: what Jinja's generated code does, node by node


class _Frame:
    # set on the few frames that differ (a frame is made per loop iteration, so it sets only what it must)
    top = False
    toplevel = False
    block = None

    def __init__(self, r, parent=None, top=False):
        self.r = r
        self.parent = parent
        if top:
            self.top = True
        self.vars = {}
        self.require_output_check = False if parent is None else parent.require_output_check

    def inner(self):
        return _Frame(self.r, self)

    def lookup(self, name):
        f = self
        while f is not None:
            if name in f.vars:
                return f.vars[name]
            if f.parent is None:
                break
            f = f.parent
        if name == "self":
            return TemplateReference(self.r.ctx)
        if name == "super" and f.block is not None:
            return self.r.ctx.super(f.block.name, f.block)
        return self.r.ctx.resolve_or_missing(name)

    def store(self, name, value):
        self.vars[name] = value
        if self.top:
            self.r.ctx.vars[name] = value
            if not name.startswith("_"):
                self.r.ctx.exported_vars.add(name)

    def locals(self):
        chain = []
        f = self
        while f is not None:
            chain.append(f)
            f = f.parent
        d = {}
        for f in reversed(chain):
            if f.top:
                continue
            for k, v in f.vars.items():
                d[k] = v
        return d


class _Render:
    # One run of a template's root or of one of its blocks, against a context
    def __init__(self, template, ctx):
        self.t = template
        self.env = template.environment
        self.ctx = ctx
        self.parent_template = None
        # Environment.getattr's own dict rule may be inlined (a subclass, like the sandbox's, may differ)
        self.plain_getattr = type(self.env).getattr is Environment.getattr

    @property
    def autoescape(self):
        return self.ctx.eval_ctx.autoescape

    def position(self, node):
        rv = "line " + str(node.lineno)
        if self.t.name is not None:
            rv = rv + " in " + repr(self.t.name)
        return rv

    # statements

    def body(self, nodes, frame, buf):
        for node in nodes:
            c = node._s
            if c is None:
                c = _st(node)
            c(self, frame, buf)

    def stmt(self, node, frame, buf):
        c = node._s
        if c is None:
            c = _st(node)
        c(self, frame, buf)

    def _stmt(self, node, frame, buf):
        k = type(node)
        if k is Output:
            self.s_output(node, frame, buf)
        elif k is If:
            if self.ev(node.test, frame):
                self.body(node.body, frame, buf)
                return
            for elif_ in node.elif_:
                if self.ev(elif_.test, frame):
                    self.body(elif_.body, frame, buf)
                    return
            self.body(node.else_, frame, buf)
        elif k is For:
            self.s_for(node, frame, buf)
        elif k is Assign:
            self.assign(node.target, self.ev(node.node, frame), frame)
        elif k is AssignBlock:
            bf = frame.inner()
            bf.require_output_check = False
            sub = []
            self.body(node.body, bf, sub)
            if node.filter is not None:
                val = self.filter(node.filter, bf, self.buf_value(sub))
            else:
                val = _concat(sub)
            if self.autoescape:
                val = Markup(val)
            self.assign(node.target, val, frame)
        elif k is MacroNode:
            m = self.make_macro(node, frame, node.name)
            frame.store(node.name, m)
        elif k is CallBlock:
            caller = self.make_macro(node, frame, None)
            buf.append(self.e_call(node.call, frame, caller))
        elif k is FilterBlock:
            ff = frame.inner()
            sub = []
            self.body(node.body, ff, sub)
            buf.append(self.filter(node.filter, ff, self.buf_value(sub)))
        elif k is With:
            wf = frame.inner()
            for i in range(len(node.targets)):
                v = self.ev(node.values[i], frame)
                self.assign(node.targets[i], v, wf)
            self.body(node.body, wf, buf)
        elif k is Block:
            self.s_block(node, frame, buf)
        elif k is Extends:
            if self.parent_template is not None:
                raise TemplateRuntimeError("extended multiple times")
            parent = self.env.get_template(self.ev(node.template, frame), self.t.name)
            for name, parent_block in parent.blocks.items():
                self.ctx.blocks.setdefault(name, []).append(parent_block)
            self.parent_template = parent
        elif k is Include:
            self.s_include(node, frame, buf)
        elif k is Import:
            mod = self.import_common(node, frame)
            frame.vars[node.target] = mod
            if frame.top:
                self.ctx.vars[node.target] = mod
                self.ctx.exported_vars.discard(node.target)
        elif k is FromImport:
            self.s_from_import(node, frame)
        elif k is ExprStmt:
            self.ev(node.node, frame)
        elif k is Break:
            raise _BreakLoop()
        elif k is Continue:
            raise _ContinueLoop()
        elif k is Scope:
            sf = frame.inner()
            self.body(node.body, sf, buf)
        elif k is ScopedEvalContextModifier:
            old = self.ctx.eval_ctx.save()
            for kw in node.options:
                if kw.key == "autoescape":
                    self.ctx.eval_ctx.autoescape = self.ev(kw.value, frame)
            try:
                self.body(node.body, frame, buf)
            finally:
                self.ctx.eval_ctx.revert(old)
        else:
            raise NotImplementedError("Brevi's jinja2 cannot run " + type(node).__name__ + " nodes")

    def buf_value(self, sub):
        if self.autoescape:
            return Markup(_concat(sub))
        return _concat(sub)

    def s_output(self, node, frame, buf):
        if frame.require_output_check and self.parent_template is not None:
            return
        fin = self.env.finalize
        for child in node.nodes:
            if type(child) is TemplateData:
                buf.append(child.data)
                continue
            v = self.ev(child, frame)
            if fin is not None:
                pa = _pass_arg(fin)
                if pa == _PASS_CONTEXT:
                    v = fin(self.ctx, v)
                elif pa == _PASS_EVAL_CONTEXT:
                    v = fin(self.ctx.eval_ctx, v)
                elif pa == _PASS_ENVIRONMENT:
                    v = fin(self.env, v)
                else:
                    v = fin(v)
            if self.autoescape:
                buf.append(escape(v))
            else:
                buf.append(str(v))

    def assign(self, target, value, frame):
        k = type(target)
        if k is Name:
            frame.store(target.name, value)
        elif k is NSRef:
            ns = frame.lookup(target.name)
            if ns is missing:
                ns = self.env.undefined(name=target.name)
            if not isinstance(ns, Namespace):
                raise TemplateRuntimeError("cannot assign attribute on non-namespace object")
            ns[target.attr] = value
        elif k is Tuple:
            for item in target.items:
                if type(item) is NSRef:
                    ns = frame.lookup(item.name)
                    if not isinstance(ns, Namespace):
                        raise TemplateRuntimeError("cannot assign attribute on non-namespace object")
            vals = self.unpack(value, len(target.items))
            for i in range(len(target.items)):
                self.assign(target.items[i], vals[i], frame)
        else:
            raise TemplateRuntimeError("cannot assign to " + type(target).__name__)

    def unpack(self, value, n):
        try:
            it = iter(value)
        except TypeError:
            raise TypeError("cannot unpack non-iterable " + _type_name(value) + " object") from None
        vals = []
        for v in it:
            vals.append(v)
            if len(vals) > n:
                raise ValueError("too many values to unpack (expected " + str(n) + ")")
        if len(vals) < n:
            raise ValueError("not enough values to unpack (expected " + str(n) + ", got " + str(len(vals)) + ")")
        return vals

    def s_for(self, node, frame, buf):
        iterable = self.ev(node.iter, frame)
        self.for_loop(node, frame, buf, iterable, 0)

    def for_loop(self, node, frame, buf, iterable, depth0):
        if node.test is not None:
            iterable = self.loop_filter(node, frame, iterable)
        recurse = None
        if node.recursive:
            def recurse(it, depth):
                sub = []
                self.for_loop(node, frame, sub, it, depth)
                return self.buf_value(sub)
        loop = LoopContext(iterable, self.env.undefined, recurse, depth0)
        ran = False
        while True:
            item = loop._next()
            if item is missing:
                break
            lf = frame.inner()
            self.assign(node.target, item, lf)
            lf.vars["loop"] = loop
            try:
                self.body(node.body, lf, buf)
            except _ContinueLoop:
                continue
            except _BreakLoop:
                break
            ran = True
        if node.else_ and not ran:
            ef = frame.inner()
            self.body(node.else_, ef, buf)

    def loop_filter(self, node, frame, iterable):
        for item in iterable:
            tf = frame.inner()
            self.assign(node.target, item, tf)
            if self.ev(node.test, tf):
                yield item

    def make_macro(self, node, frame, name):
        args = [a.name for a in node.args]
        undeclared = _find_undeclared(node.body, ("caller", "kwargs", "varargs"))
        explicit_caller = None
        skip = set()
        for idx in range(len(args)):
            if args[idx] == "caller":
                explicit_caller = idx
            if args[idx] in ("kwargs", "varargs"):
                skip.add(args[idx])
        accesses_caller = False
        if "caller" in undeclared:
            accesses_caller = explicit_caller is None
            if explicit_caller is not None:
                accesses_caller = False
        accesses_kwargs = "kwargs" in undeclared and "kwargs" not in skip
        accesses_varargs = "varargs" in undeclared and "varargs" not in skip
        r = self
        defaults = node.defaults
        n_args = len(args)

        def func(arguments):
            mf = _Frame(r, frame)
            mf.require_output_check = False
            for i in range(n_args):
                mf.vars[args[i]] = arguments[i]
            pos = n_args
            if accesses_caller:
                mf.vars["caller"] = arguments[pos]
                pos += 1
            if accesses_kwargs:
                mf.vars["kwargs"] = arguments[pos]
                pos += 1
            if accesses_varargs:
                mf.vars["varargs"] = arguments[pos]
                pos += 1
            for i in range(n_args):
                if mf.vars[args[i]] is missing:
                    di = i - n_args + len(defaults)
                    if di >= 0:
                        mf.vars[args[i]] = r.ev(defaults[di], mf)
                    else:
                        mf.vars[args[i]] = r.env.undefined("parameter " + repr(args[i]) + " was not provided", name=args[i])
            sub = []
            r.body(node.body, mf, sub)
            return _concat(sub)

        return Macro(self.env, func, name, tuple(args), accesses_kwargs, accesses_varargs, accesses_caller, self.autoescape)

    def s_block(self, node, frame, buf):
        if frame.toplevel and self.parent_template is not None:
            return
        if node.scoped:
            context = self.ctx.derived(frame.locals())
        else:
            context = self.ctx
        if node.required and len(context.blocks[node.name]) <= 1:
            raise TemplateRuntimeError("Required block " + repr(node.name) + " not found")
        for piece in context.blocks[node.name][0].render(context):
            buf.append(piece)

    def s_include(self, node, frame, buf):
        tn = node.template
        try:
            if type(tn) is Const and isinstance(tn.value, str):
                template = self.env.get_template(self.ev(tn, frame), self.t.name)
            elif (type(tn) is Const and isinstance(tn.value, (tuple, list))) or type(tn) is Tuple or type(tn) is List:
                template = self.env.select_template(self.ev(tn, frame), self.t.name)
            else:
                template = self.env.get_or_select_template(self.ev(tn, frame), self.t.name)
        except TemplateNotFound:
            if node.ignore_missing:
                return
            raise
        if node.with_context:
            # Jinja passes an included template the frame's names, but not the enclosing loop
            lv = frame.locals()
            if "loop" in lv:
                del lv["loop"]
            ctx = template.new_context(self.ctx.get_all(), True, lv)
            for piece in template._render_root(ctx):
                buf.append(piece)
        else:
            for piece in template._get_default_module()._body_stream:
                buf.append(piece)

    def import_common(self, node, frame):
        template = self.env.get_template(self.ev(node.template, frame), self.t.name)
        if node.with_context:
            return template.make_module(self.ctx.get_all(), True, frame.locals())
        return template._get_default_module(self.ctx)

    def s_from_import(self, node, frame):
        mod = self.import_common(node, frame)
        for name in node.names:
            if isinstance(name, tuple):
                name, alias = name
            else:
                alias = name
            v = getattr(mod, name, missing)
            if v is missing:
                v = self.env.undefined("the template " + repr(mod.__name__) + " (imported on " + self.position(node) + ") does not export the requested name " + repr(name), name=name)
            frame.vars[alias] = v
            if frame.top:
                self.ctx.vars[alias] = v
                self.ctx.exported_vars.discard(alias)

    # expressions

    def ev(self, node, frame):
        c = node._x
        if c is None:
            c = _ex(node)
        return c(self, frame)

    def _ev(self, node, frame):
        k = type(node)
        if k is Const:
            return node.value
        if k is Name:
            v = frame.lookup(node.name)
            if v is missing:
                return self.env.undefined(name=node.name)
            return v
        if k is Getattr:
            return self.env.getattr(self.ev(node.node, frame), node.attr)
        if k is Getitem:
            obj = self.ev(node.node, frame)
            if type(node.arg) is Slice:
                return obj[self.ev(node.arg, frame)]
            return self.env.getitem(obj, self.ev(node.arg, frame))
        if k is Filter:
            return self.filter(node, frame, None)
        if k is Test:
            return self.test(node, frame)
        if k is Call:
            return self.e_call(node, frame, None)
        if k is Concat:
            vals = [self.ev(n, frame) for n in node.nodes]
            if self.autoescape:
                return markup_join(vals)
            return str_join(vals)
        if k is CondExpr:
            if self.ev(node.test, frame):
                return self.ev(node.expr1, frame)
            if node.expr2 is None:
                return Undefined("the inline if-expression on " + self.position(node) + " evaluated to false and no else section was defined.")
            return self.ev(node.expr2, frame)
        if k is And:
            left = self.ev(node.left, frame)
            if not left:
                return left
            return self.ev(node.right, frame)
        if k is Or:
            left = self.ev(node.left, frame)
            if left:
                return left
            return self.ev(node.right, frame)
        if k is Not:
            return not self.ev(node.node, frame)
        if k is Compare:
            return self.compare(node, frame)
        if k is Add:
            return self.ev(node.left, frame) + self.ev(node.right, frame)
        if k is Sub:
            return self.ev(node.left, frame) - self.ev(node.right, frame)
        if k is Mul:
            return self.ev(node.left, frame) * self.ev(node.right, frame)
        if k is Div:
            return self.ev(node.left, frame) / self.ev(node.right, frame)
        if k is FloorDiv:
            return self.ev(node.left, frame) // self.ev(node.right, frame)
        if k is Mod:
            return self.ev(node.left, frame) % self.ev(node.right, frame)
        if k is Pow:
            return self.ev(node.left, frame) ** self.ev(node.right, frame)
        if k is Neg:
            return -self.ev(node.node, frame)
        if k is Pos:
            return +self.ev(node.node, frame)
        if k is Tuple:
            return tuple([self.ev(n, frame) for n in node.items])
        if k is List:
            return [self.ev(n, frame) for n in node.items]
        if k is Dict:
            d = {}
            for p in node.items:
                key = self.ev(p.key, frame)
                d[key] = self.ev(p.value, frame)
            return d
        if k is Slice:
            start = None if node.start is None else self.ev(node.start, frame)
            stop = None if node.stop is None else self.ev(node.stop, frame)
            step = None if node.step is None else self.ev(node.step, frame)
            return slice(start, stop, step)
        if k is TemplateData:
            if self.autoescape:
                return Markup(node.data)
            return node.data
        raise NotImplementedError("Brevi's jinja2 cannot evaluate " + type(node).__name__ + " nodes")

    def compare(self, node, frame):
        left = self.ev(node.expr, frame)
        result = True
        for op in node.ops:
            right = self.ev(op.expr, frame)
            o = op.op
            if o == "eq":
                result = left == right
            elif o == "ne":
                result = left != right
            elif o == "lt":
                result = left < right
            elif o == "lteq":
                result = left <= right
            elif o == "gt":
                result = left > right
            elif o == "gteq":
                result = left >= right
            elif o == "in":
                result = left in right
            else:
                result = left not in right
            if not result:
                return result
            left = right
        return result

    def call_args(self, node, frame):
        args = [self.ev(a, frame) for a in node.args]
        kwargs = {}
        for kw in node.kwargs:
            kwargs[kw.key] = self.ev(kw.value, frame)
        if node.dyn_args is not None:
            for a in self.ev(node.dyn_args, frame):
                args.append(a)
        if node.dyn_kwargs is not None:
            dk = self.ev(node.dyn_kwargs, frame)
            for key in dk.keys():
                if key in kwargs:
                    raise TypeError("got multiple values for keyword argument " + repr(key))
                kwargs[key] = dk[key]
        return args, kwargs

    def e_call(self, node, frame, caller):
        func = self.ev(node.node, frame)
        args, kwargs = self.call_args(node, frame)
        if caller is not None:
            kwargs["caller"] = caller
        return self.ctx.call(func, *args, **kwargs)

    def filter(self, node, frame, value):
        func = self.env.filters.get(node.name)
        if node.node is not None:
            value = self.ev(node.node, frame)
        args, kwargs = self.call_args(node, frame)
        if func is None:
            raise TemplateRuntimeError("No filter named " + repr(node.name) + " found.")
        pa = _pass_arg(func)
        if pa == _PASS_CONTEXT:
            return func(self.ctx, value, *args, **kwargs)
        if pa == _PASS_EVAL_CONTEXT:
            return func(self.ctx.eval_ctx, value, *args, **kwargs)
        if pa == _PASS_ENVIRONMENT:
            return func(self.env, value, *args, **kwargs)
        return func(value, *args, **kwargs)

    def test(self, node, frame):
        func = self.env.tests.get(node.name)
        value = self.ev(node.node, frame)
        args, kwargs = self.call_args(node, frame)
        if func is None:
            raise TemplateRuntimeError("No test named " + repr(node.name) + " found.")
        pa = _pass_arg(func)
        if pa == _PASS_CONTEXT:
            return func(self.ctx, value, *args, **kwargs)
        if pa == _PASS_EVAL_CONTEXT:
            return func(self.ctx.eval_ctx, value, *args, **kwargs)
        if pa == _PASS_ENVIRONMENT:
            return func(self.env, value, *args, **kwargs)
        return func(value, *args, **kwargs)


# ---------- templates compiled to closures ----------
# Each node is turned once into a closure: an expression into fn(r, frame) -> value, a statement into
# fn(r, frame, buf). The closures do what _Render._ev / _Render._stmt do for that node, in the same order,
# without dispatching on the node's type or reading its fields on every render. A node kind without a
# closure of its own gets one that calls the interpreter, whose children are compiled in turn.


def _ex(node):
    c = node._x
    if c is None:
        c = _compile_expr(node)
        node._x = c
    return c


def _st(node):
    c = node._s
    if c is None:
        c = _compile_stmt(node)
        node._s = c
    return c


def _body_fns(nodes):
    return [_st(n) for n in nodes]


def _uses_names(nodes, names):
    # whether any node below reads one of the names, or could reach the frame's locals another way
    todo = list(nodes)
    while todo:
        n = todo.pop()
        k = type(n)
        if k is Name:
            if n.name in names:
                return True
            continue
        if k is Include or k is Block or k is Import or k is FromImport or k is CallBlock or k is MacroNode:
            return True
        for c in n.iter_child_nodes():
            todo.append(c)
    return False


def _has_jump(nodes):
    todo = list(nodes)
    while todo:
        n = todo.pop()
        if type(n) is Break or type(n) is Continue:
            return True
        for c in n.iter_child_nodes():
            todo.append(c)
    return False


def _args_fn(node):
    # node.args etc. as fn(r, f) -> (args, kwargs), as _Render.call_args
    pos = [_ex(a) for a in node.args]
    kws = [(kw.key, _ex(kw.value)) for kw in node.kwargs]
    dyn_args = None if node.dyn_args is None else _ex(node.dyn_args)
    dyn_kwargs = None if node.dyn_kwargs is None else _ex(node.dyn_kwargs)

    def fn(r, f):
        args = [c(r, f) for c in pos]
        kwargs = {}
        for key, c in kws:
            kwargs[key] = c(r, f)
        if dyn_args is not None:
            for a in dyn_args(r, f):
                args.append(a)
        if dyn_kwargs is not None:
            dk = dyn_kwargs(r, f)
            for key in dk.keys():
                if key in kwargs:
                    raise TypeError("got multiple values for keyword argument " + repr(key))
                kwargs[key] = dk[key]
        return args, kwargs

    return fn


def _ft_fn(node, table, what):
    # a Filter (table "filters") or Test (table "tests") node
    name = node.name
    cv = None if node.node is None else _ex(node.node)
    simple = not node.kwargs and node.dyn_args is None and node.dyn_kwargs is None
    pos = [_ex(a) for a in node.args]
    n_pos = len(pos)
    ca = None if simple else _args_fn(node)
    is_filter = table == "filters"
    memo = [None, None]

    def fn(r, f):
        env = r.env
        func = env.filters.get(name) if is_filter else env.tests.get(name)
        value = None if cv is None else cv(r, f)
        if simple:
            if n_pos == 0:
                args = None
            elif n_pos == 1:
                args = [pos[0](r, f)]
            else:
                args = [c(r, f) for c in pos]
            kwargs = None
        else:
            args, kwargs = ca(r, f)
        if func is None:
            raise TemplateRuntimeError("No " + what + " named " + repr(name) + " found.")
        if memo[0] is func:
            pa = memo[1]
        else:
            pa = _pass_arg(func)
            memo[0] = func
            memo[1] = pa
        if pa is None:
            if kwargs is None:
                if args is None:
                    return func(value)
                if n_pos == 1:
                    return func(value, args[0])
                return func(value, *args)
            return func(value, *args, **kwargs)
        if args is None:
            args = []
        if kwargs is None:
            kwargs = {}
        if pa == _PASS_CONTEXT:
            return func(r.ctx, value, *args, **kwargs)
        if pa == _PASS_EVAL_CONTEXT:
            return func(r.ctx.eval_ctx, value, *args, **kwargs)
        if pa == _PASS_ENVIRONMENT:
            return func(env, value, *args, **kwargs)
        return func(value, *args, **kwargs)

    return fn


def _cmp_one(o, left, right):
    if o == "eq":
        return left == right
    if o == "ne":
        return left != right
    if o == "lt":
        return left < right
    if o == "lteq":
        return left <= right
    if o == "gt":
        return left > right
    if o == "gteq":
        return left >= right
    if o == "in":
        return left in right
    return left not in right


def _compile_expr(node):
    k = type(node)
    if k is Const:
        value = node.value

        def fn(r, f):
            return value
        return fn
    if k is Name:
        name = node.name

        def fn(r, f):
            g = f
            while True:
                vs = g.vars
                if name in vs:
                    v = vs[name]
                    if v is missing:
                        return r.env.undefined(name=name)
                    return v
                p = g.parent
                if p is None:
                    break
                g = p
            if name == "self":
                return TemplateReference(r.ctx)
            if name == "super" and g.block is not None:
                v = r.ctx.super(g.block.name, g.block)
            else:
                v = r.ctx.resolve_or_missing(name)
            if v is missing:
                return r.env.undefined(name=name)
            return v
        return fn
    if k is Getattr:
        c0 = _ex(node.node)
        attr = node.attr
        plain = type(attr) is str and attr[:1] != "_" and attr not in _DICT_ATTRS

        def fn(r, f):
            obj = c0(r, f)
            if plain and type(obj) is dict and r.plain_getattr:
                if attr in obj:
                    return obj[attr]
            return r.env.getattr(obj, attr)
        return fn
    if k is Getitem:
        c0 = _ex(node.node)
        c1 = _ex(node.arg)
        if type(node.arg) is Slice:
            def fn(r, f):
                obj = c0(r, f)
                return obj[c1(r, f)]
            return fn

        def fn(r, f):
            obj = c0(r, f)
            return r.env.getitem(obj, c1(r, f))
        return fn
    if k is Filter:
        return _ft_fn(node, "filters", "filter")
    if k is Test:
        return _ft_fn(node, "tests", "test")
    if k is Call:
        cf = _ex(node.node)
        ca = _args_fn(node)

        def fn(r, f):
            func = cf(r, f)
            args, kwargs = ca(r, f)
            return r.ctx.call(func, *args, **kwargs)
        return fn
    if k is Concat:
        cs = [_ex(n) for n in node.nodes]

        def fn(r, f):
            vals = [c(r, f) for c in cs]
            if r.ctx.eval_ctx.autoescape:
                return markup_join(vals)
            return str_join(vals)
        return fn
    if k is CondExpr:
        ct = _ex(node.test)
        c1 = _ex(node.expr1)
        if node.expr2 is None:
            def fn(r, f):
                if ct(r, f):
                    return c1(r, f)
                return Undefined("the inline if-expression on " + r.position(node) + " evaluated to false and no else section was defined.")
            return fn
        c2 = _ex(node.expr2)

        def fn(r, f):
            if ct(r, f):
                return c1(r, f)
            return c2(r, f)
        return fn
    if k is And:
        cl = _ex(node.left)
        cr = _ex(node.right)

        def fn(r, f):
            left = cl(r, f)
            if not left:
                return left
            return cr(r, f)
        return fn
    if k is Or:
        cl = _ex(node.left)
        cr = _ex(node.right)

        def fn(r, f):
            left = cl(r, f)
            if left:
                return left
            return cr(r, f)
        return fn
    if k is Not:
        c0 = _ex(node.node)

        def fn(r, f):
            return not c0(r, f)
        return fn
    if k is Compare:
        ce = _ex(node.expr)
        ops = [(op.op, _ex(op.expr)) for op in node.ops]
        if len(ops) == 1:
            o, c1 = ops[0]
            if o == "eq":
                def fn(r, f):
                    left = ce(r, f)
                    return left == c1(r, f)
                return fn
            if o == "ne":
                def fn(r, f):
                    left = ce(r, f)
                    return left != c1(r, f)
                return fn

            def fn(r, f):
                left = ce(r, f)
                return _cmp_one(o, left, c1(r, f))
            return fn

        def fn(r, f):
            left = ce(r, f)
            result = True
            for o, c in ops:
                right = c(r, f)
                result = _cmp_one(o, left, right)
                if not result:
                    return result
                left = right
            return result
        return fn
    if k is Add:
        cl = _ex(node.left)
        cr = _ex(node.right)

        def fn(r, f):
            return cl(r, f) + cr(r, f)
        return fn
    if k is Sub:
        cl = _ex(node.left)
        cr = _ex(node.right)

        def fn(r, f):
            return cl(r, f) - cr(r, f)
        return fn
    if k is Mul:
        cl = _ex(node.left)
        cr = _ex(node.right)

        def fn(r, f):
            return cl(r, f) * cr(r, f)
        return fn
    if k is Mod:
        cl = _ex(node.left)
        cr = _ex(node.right)

        def fn(r, f):
            return cl(r, f) % cr(r, f)
        return fn
    if k is Tuple:
        cs = [_ex(n) for n in node.items]

        def fn(r, f):
            return tuple([c(r, f) for c in cs])
        return fn
    if k is List:
        cs = [_ex(n) for n in node.items]

        def fn(r, f):
            return [c(r, f) for c in cs]
        return fn
    if k is TemplateData:
        data = node.data

        def fn(r, f):
            if r.ctx.eval_ctx.autoescape:
                return Markup(data)
            return data
        return fn

    def fn(r, f):
        return r._ev(node, f)
    return fn


def _compile_stmt(node):
    k = type(node)
    if k is Output:
        parts = []
        for child in node.nodes:
            if type(child) is TemplateData:
                parts.append((True, child.data))
            else:
                parts.append((False, _ex(child)))

        def fn(r, f, buf):
            if f.require_output_check and r.parent_template is not None:
                return
            fin = r.env.finalize
            for is_data, p in parts:
                if is_data:
                    buf.append(p)
                    continue
                v = p(r, f)
                if fin is not None:
                    pa = _pass_arg(fin)
                    if pa == _PASS_CONTEXT:
                        v = fin(r.ctx, v)
                    elif pa == _PASS_EVAL_CONTEXT:
                        v = fin(r.ctx.eval_ctx, v)
                    elif pa == _PASS_ENVIRONMENT:
                        v = fin(r.env, v)
                    else:
                        v = fin(v)
                if r.ctx.eval_ctx.autoescape:
                    buf.append(escape(v))
                elif type(v) is str:
                    buf.append(v)
                else:
                    buf.append(str(v))
        return fn
    if k is If:
        branches = [(_ex(node.test), _body_fns(node.body))]
        for elif_ in node.elif_:
            branches.append((_ex(elif_.test), _body_fns(elif_.body)))
        else_ = _body_fns(node.else_)

        def fn(r, f, buf):
            for ct, body in branches:
                if ct(r, f):
                    for c in body:
                        c(r, f, buf)
                    return
            for c in else_:
                c(r, f, buf)
        return fn
    if k is For and not node.recursive and node.test is None:
        ci = _ex(node.iter)
        body = _body_fns(node.body)
        else_ = _body_fns(node.else_) if node.else_ else None
        target = node.target
        tname = target.name if type(target) is Name else None
        jumps = _has_jump(node.body)
        # Jinja itself makes no loop object for a loop whose body cannot see it
        needs_loop = _uses_names(node.body, ("loop",))

        def fn(r, f, buf):
            iterable = ci(r, f)
            if needs_loop:
                loop = LoopContext(iterable, r.env.undefined, None, 0)
                nxt = loop._next
            else:
                loop = None
                it = iter(iterable)
            ran = False
            while True:
                if loop is not None:
                    item = nxt()
                else:
                    item = next(it, missing)
                if item is missing:
                    break
                lf = _Frame(r, f)
                if tname is not None:
                    lf.vars[tname] = item
                else:
                    r.assign(target, item, lf)
                if loop is not None:
                    lf.vars["loop"] = loop
                if jumps:
                    try:
                        for c in body:
                            c(r, lf, buf)
                    except _ContinueLoop:
                        continue
                    except _BreakLoop:
                        break
                else:
                    for c in body:
                        c(r, lf, buf)
                ran = True
            if else_ is not None and not ran:
                ef = _Frame(r, f)
                for c in else_:
                    c(r, ef, buf)
        return fn
    if k is Assign and type(node.target) is Name:
        cv = _ex(node.node)
        name = node.target.name

        def fn(r, f, buf):
            f.store(name, cv(r, f))
        return fn
    if k is ExprStmt:
        cv = _ex(node.node)

        def fn(r, f, buf):
            cv(r, f)
        return fn

    def fn(r, f, buf):
        r._stmt(node, f, buf)
    return fn


def _find_undeclared(nodes, names):
    # The names of `names` the nodes read before (or without) declaring them, as Jinja's
    # UndeclaredNameVisitor finds them: block bodies are not looked into.
    found = set()
    todo = list(nodes)
    while todo:
        n = todo.pop()
        if type(n) is Name:
            if n.ctx == "load" and n.name in names:
                found.add(n.name)
            continue
        if type(n) is Block:
            continue
        for c in n.iter_child_nodes():
            todo.append(c)
    return found


class _BlockFunc:
    def __init__(self, template, node):
        self.template = template
        self.node = node
        self.name = node.name

    def render(self, ctx):
        r = _Render(self.template, ctx)
        f = _Frame(r, None)
        f.block = self
        buf = []
        r.body(self.node.body, f, buf)
        return buf


def _check_template(node, env, name, filename):
    # What Jinja's compiler refuses before any rendering
    blocks = {}
    for block in node.find_all(Block):
        if block.name in blocks:
            raise TemplateAssertionError("block " + repr(block.name) + " defined twice", block.lineno, name, filename)
        blocks[block.name] = block
    _check_body(node.body, env, name, filename, True, False)
    return blocks


def _check_body(nodes, env, name, filename, toplevel, soft):
    for n in nodes:
        _check_node(n, env, name, filename, toplevel, soft)


def _check_node(n, env, name, filename, toplevel, soft):
    k = type(n)
    if k is Extends and not toplevel:
        raise TemplateAssertionError("cannot use extend from a non top-level scope", n.lineno, name, filename)
    if k is If:
        _check_node(n.test, env, name, filename, toplevel, True)
        _check_body(n.body, env, name, filename, toplevel, True)
        for e in n.elif_:
            _check_node(e.test, env, name, filename, toplevel, True)
            _check_body(e.body, env, name, filename, toplevel, True)
        _check_body(n.else_, env, name, filename, toplevel, True)
        return
    if k is CondExpr:
        for c in n.iter_child_nodes():
            _check_node(c, env, name, filename, toplevel, True)
        return
    if k is For:
        for nm in n.find_all(Name):
            if nm.ctx == "store" and nm.name == "loop":
                raise TemplateAssertionError("Can't assign to special loop variable in for-loop target", nm.lineno, name, filename)
        _check_node(n.iter, env, name, filename, toplevel, soft)
        for c in n.iter_child_nodes():
            if c is not n.iter:
                _check_node(c, env, name, filename, False, False)
        return
    if k is MacroNode or k is CallBlock:
        explicit_caller = None
        for idx in range(len(n.args)):
            if n.args[idx].name == "caller":
                explicit_caller = idx
        if explicit_caller is not None and "caller" in _find_undeclared(n.body, ("caller",)):
            if explicit_caller - len(n.args) + len(n.defaults) < 0:
                raise TemplateAssertionError('When defining macros or call blocks the special "caller" argument must be omitted or be given a default.', n.lineno, name, filename)
        if k is CallBlock:
            _check_node(n.call, env, name, filename, toplevel, soft)
        for d in n.defaults:
            _check_node(d, env, name, filename, False, False)
        _check_body(n.body, env, name, filename, False, False)
        return
    if k is Filter or k is Test:
        func = env.filters.get(n.name) if k is Filter else env.tests.get(n.name)
        if func is None and not soft:
            raise TemplateAssertionError("No " + ("filter" if k is Filter else "test") + " named " + repr(n.name) + ".", n.lineno, name, filename)
    if k is Block or k is With or k is FilterBlock or k is AssignBlock or k is Scope:
        for c in n.iter_child_nodes():
            _check_node(c, env, name, filename, False, False)
        return
    for c in n.iter_child_nodes():
        _check_node(c, env, name, filename, toplevel, soft)


# -- filters


def _ignore_case(value):
    if isinstance(value, str):
        return value.lower()
    return value


def _prepare_attribute_parts(attr):
    if attr is None:
        return []
    if isinstance(attr, str):
        return [int(x) if x.isdigit() else x for x in attr.split(".")]
    return [attr]


def make_attrgetter(environment, attribute, postprocess=None, default=None):
    parts = _prepare_attribute_parts(attribute)

    def attrgetter(item):
        for part in parts:
            item = environment.getitem(item, part)
            if default is not None and isinstance(item, Undefined):
                item = default
        if postprocess is not None:
            item = postprocess(item)
        return item

    return attrgetter


def make_multi_attrgetter(environment, attribute, postprocess=None):
    if isinstance(attribute, str):
        split = attribute.split(",")
    else:
        split = [attribute]
    parts = [_prepare_attribute_parts(item) for item in split]

    def attrgetter(item):
        items = [None] * len(parts)
        for i in range(len(parts)):
            item_i = item
            for part in parts[i]:
                item_i = environment.getitem(item_i, part)
            if postprocess is not None:
                item_i = postprocess(item_i)
            items[i] = item_i
        return items

    return attrgetter


def do_forceescape(value):
    if hasattr(value, "__html__"):
        value = value.__html__()
    return escape(str(value))


def do_urlencode(value):
    if isinstance(value, str) or not _is_iterable(value):
        return url_quote(value)
    if isinstance(value, dict):
        items = value.items()
    else:
        items = value
    return "&".join([url_quote(k, for_qs=True) + "=" + url_quote(v, for_qs=True) for k, v in items])


def _is_iterable(value):
    try:
        iter(value)
    except TypeError:
        return False
    return True


@pass_eval_context
def do_replace(eval_ctx, s, old, new, count=None):
    if count is None:
        count = -1
    if not eval_ctx.autoescape:
        return str(s).replace(str(old), str(new), count)
    if hasattr(old, "__html__") or (hasattr(new, "__html__") and not hasattr(s, "__html__")):
        s = escape(s)
    else:
        s = soft_str(s)
    return s.replace(soft_str(old), soft_str(new), count)


def do_upper(s):
    return soft_str(s).upper()


def do_lower(s):
    return soft_str(s).lower()


def do_items(value):
    if isinstance(value, Undefined):
        return
    if not _is_mapping(value):
        raise TypeError("Can only get item pairs from a mapping.")
    for kv in value.items():
        yield kv


_attr_key_re = re.compile(r"[ \t\n\r\x0b\x0c\x1c\x1d\x1e\x1f/>=]")


@pass_eval_context
def do_xmlattr(eval_ctx, d, autospace=True):
    items = []
    for key, value in d.items():
        if value is None or isinstance(value, Undefined):
            continue
        if _attr_key_re.search(key) is not None:
            raise ValueError("Invalid character in attribute name: " + repr(key))
        items.append(str(escape(key)) + '="' + str(escape(value)) + '"')
    rv = " ".join(items)
    if autospace and rv:
        rv = " " + rv
    if eval_ctx.autoescape:
        rv = Markup(rv)
    return rv


def do_capitalize(s):
    return soft_str(s).capitalize()


_word_beginning_split_re = re.compile(r"([-\s({\[<]+)")


def do_title(s):
    return "".join([item[0].upper() + item[1:].lower() for item in _word_beginning_split_re.split(soft_str(s)) if item])


def do_dictsort(value, case_sensitive=False, by="key", reverse=False):
    if by == "key":
        pos = 0
    elif by == "value":
        pos = 1
    else:
        raise FilterArgumentError('You can only sort by either "key" or "value"')

    def sort_func(item):
        value = item[pos]
        if not case_sensitive:
            value = _ignore_case(value)
        return value

    return sorted(value.items(), key=sort_func, reverse=reverse)


@pass_environment
def do_sort(environment, value, reverse=False, case_sensitive=False, attribute=None):
    key_func = make_multi_attrgetter(environment, attribute, postprocess=_ignore_case if not case_sensitive else None)
    return sorted(value, key=key_func, reverse=reverse)


@pass_environment
def do_unique(environment, value, case_sensitive=False, attribute=None):
    getter = make_attrgetter(environment, attribute, postprocess=_ignore_case if not case_sensitive else None)
    seen = set()
    for item in value:
        key = getter(item)
        if key not in seen:
            seen.add(key)
            yield item


def _min_or_max(environment, value, func, case_sensitive, attribute):
    it = iter(value)
    first = next(it, missing)
    if first is missing:
        return environment.undefined("No aggregated item, sequence was empty.")
    key_func = make_attrgetter(environment, attribute, postprocess=_ignore_case if not case_sensitive else None)
    rest = [first]
    for x in it:
        rest.append(x)
    return func(rest, key=key_func)


@pass_environment
def do_min(environment, value, case_sensitive=False, attribute=None):
    return _min_or_max(environment, value, min, case_sensitive, attribute)


@pass_environment
def do_max(environment, value, case_sensitive=False, attribute=None):
    return _min_or_max(environment, value, max, case_sensitive, attribute)


def do_default(value, default_value="", boolean=False):
    if isinstance(value, Undefined) or (boolean and not value):
        return default_value
    return value


@pass_eval_context
def do_join(eval_ctx, value, d="", attribute=None):
    if attribute is not None:
        getter = make_attrgetter(eval_ctx.environment, attribute)
        value = [getter(x) for x in value]
    if not eval_ctx.autoescape:
        return str(d).join([str(x) for x in value])
    if not hasattr(d, "__html__"):
        value = list(value)
        do_esc = False
        for idx in range(len(value)):
            item = value[idx]
            if hasattr(item, "__html__"):
                do_esc = True
            else:
                value[idx] = str(item)
        if do_esc:
            d = escape(d)
        else:
            d = str(d)
        return d.join(value)
    return soft_str(d).join([soft_str(x) for x in value])


def do_center(value, width=80):
    return soft_str(value).center(width)


@pass_environment
def do_first(environment, seq):
    rv = next(iter(seq), missing)
    if rv is missing:
        return environment.undefined("No first item, sequence was empty.")
    return rv


@pass_environment
def do_last(environment, seq):
    rv = next(iter(reversed(seq)), missing)
    if rv is missing:
        return environment.undefined("No last item, sequence was empty.")
    return rv


@pass_context
def do_random(context, seq):
    import random
    try:
        return random.choice(seq)
    except IndexError:
        return context.environment.undefined("No random item, sequence was empty.")


def do_filesizeformat(value, binary=False):
    bytes_ = float(value)
    base = 1024 if binary else 1000
    prefixes = [
        "KiB" if binary else "kB", "MiB" if binary else "MB", "GiB" if binary else "GB", "TiB" if binary else "TB",
        "PiB" if binary else "PB", "EiB" if binary else "EB", "ZiB" if binary else "ZB", "YiB" if binary else "YB",
    ]
    if bytes_ == 1:
        return "1 Byte"
    elif bytes_ < base:
        return str(int(bytes_)) + " Bytes"
    else:
        unit = base
        prefix = prefixes[0]
        for i in range(len(prefixes)):
            prefix = prefixes[i]
            unit = base ** (i + 2)
            if bytes_ < unit:
                return f"{base * bytes_ / unit:.1f} {prefix}"
        return f"{base * bytes_ / unit:.1f} {prefix}"


def do_pprint(value):
    return _pformat(value)


_uri_scheme_re = re.compile(r"^([\w.+-]{2,}:(/){0,2})$")
_http_re = re.compile(
    r"^((https?://|www\.)(([\w%-]+\.)+)?([a-z]{2,63}|xn--[\w%]{2,59})|([\w%-]{2,63}\.)+(com|net|int|edu|gov|org|info|mil)|(https?://)((([\d]{1,3})(\.[\d]{1,3}){3})|(\[([\da-f]{0,4}:){2}([\da-f]{0,4}:?){1,6}])))(?::[\d]{1,5})?(?:[/?#]\S*)?$",
    re.IGNORECASE,
)
_email_re = re.compile(r"^\S+@\w[\w.-]*\.\w+$")


def urlize(text, trim_url_limit=None, rel=None, target=None, extra_schemes=None):
    if trim_url_limit is not None:
        def trim_url(x):
            if len(x) > trim_url_limit:
                return x[:trim_url_limit] + "..."
            return x
    else:
        def trim_url(x):
            return x
    words = re.split(r"(\s+)", str(escape(text)))
    rel_attr = ' rel="' + str(escape(rel)) + '"' if rel else ""
    target_attr = ' target="' + str(escape(target)) + '"' if target else ""
    for i in range(len(words)):
        word = words[i]
        head, middle, tail = "", word, ""
        match = re.match(r"^([(<]|&lt;)+", middle)
        if match:
            head = match.group()
            middle = middle[match.end():]
        if middle.endswith((")", ">", ".", ",", "\n", "&gt;")):
            match = re.search(r"([)>.,\n]|&gt;)+$", middle)
            if match:
                tail = match.group()
                middle = middle[:match.start()]
        for start_char, end_char in (("(", ")"), ("<", ">"), ("&lt;", "&gt;")):
            start_count = middle.count(start_char)
            if start_count <= middle.count(end_char):
                continue
            for _ in range(min(start_count, tail.count(end_char))):
                end_index = tail.index(end_char) + len(end_char)
                middle += tail[:end_index]
                tail = tail[end_index:]
        if _http_re.match(middle):
            if middle.startswith("https://") or middle.startswith("http://"):
                middle = '<a href="' + middle + '"' + rel_attr + target_attr + ">" + trim_url(middle) + "</a>"
            else:
                middle = '<a href="https://' + middle + '"' + rel_attr + target_attr + ">" + trim_url(middle) + "</a>"
        elif middle.startswith("mailto:") and _email_re.match(middle[7:]):
            middle = '<a href="' + middle + '">' + middle[7:] + "</a>"
        elif "@" in middle and not middle.startswith("www.") and not middle.startswith("@") and ":" not in middle and _email_re.match(middle):
            middle = '<a href="mailto:' + middle + '">' + middle + "</a>"
        elif extra_schemes is not None:
            for scheme in extra_schemes:
                if middle != scheme and middle.startswith(scheme):
                    middle = '<a href="' + middle + '"' + rel_attr + target_attr + ">" + middle + "</a>"
        words[i] = head + middle + tail
    return "".join(words)


@pass_eval_context
def do_urlize(eval_ctx, value, trim_url_limit=None, nofollow=False, target=None, rel=None, extra_schemes=None):
    policies = eval_ctx.environment.policies
    rel_parts = set((rel or "").split())
    if nofollow:
        rel_parts.add("nofollow")
    for p in (policies["urlize.rel"] or "").split():
        rel_parts.add(p)
    rel = " ".join(sorted(rel_parts)) or None
    if target is None:
        target = policies["urlize.target"]
    if extra_schemes is None:
        extra_schemes = policies["urlize.extra_schemes"] or ()
    for scheme in extra_schemes:
        if _uri_scheme_re.fullmatch(scheme) is None:
            raise FilterArgumentError(repr(scheme) + " is not a valid URI scheme prefix.")
    rv = urlize(value, trim_url_limit=trim_url_limit, rel=rel, target=target, extra_schemes=extra_schemes)
    if eval_ctx.autoescape:
        rv = Markup(rv)
    return rv


def do_indent(s, width=4, first=False, blank=False):
    if isinstance(width, str):
        indention = width
    else:
        indention = " " * width
    newline = "\n"
    if isinstance(s, Markup):
        indention = Markup(indention)
        newline = Markup(newline)
    s += newline
    if blank:
        rv = (newline + indention).join(s.splitlines())
    else:
        lines = s.splitlines()
        rv = lines.pop(0)
        if lines:
            rv += newline + newline.join([indention + line if line else line for line in lines])
    if first:
        rv = indention + rv
    return rv


@pass_environment
def do_truncate(env, s, length=255, killwords=False, end="...", leeway=None):
    if leeway is None:
        leeway = env.policies["truncate.leeway"]
    if not length >= len(end):
        raise AssertionError("expected length >= " + str(len(end)) + ", got " + str(length))
    if not leeway >= 0:
        raise AssertionError("expected leeway >= 0, got " + str(leeway))
    if len(s) <= length + leeway:
        return s
    if killwords:
        return s[:length - len(end)] + end
    result = s[:length - len(end)].rsplit(" ", 1)[0]
    return result + end


@pass_environment
def do_wordwrap(environment, s, width=79, break_long_words=True, wrapstring=None, break_on_hyphens=True):
    import textwrap
    if wrapstring is None:
        wrapstring = environment.newline_sequence
    return wrapstring.join([wrapstring.join(textwrap.wrap(line, width=width, expand_tabs=False, replace_whitespace=False, break_long_words=break_long_words, break_on_hyphens=break_on_hyphens)) for line in s.splitlines()])


_word_re = re.compile(r"\w+")


def do_wordcount(s):
    return len(_word_re.findall(soft_str(s)))


def do_int(value, default=0, base=10):
    try:
        if isinstance(value, str):
            return int(value, base)
        return int(value)
    except (TypeError, ValueError):
        try:
            return int(float(value))
        except (TypeError, ValueError, OverflowError):
            return default


def do_float(value, default=0.0):
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def do_format(value, *args, **kwargs):
    if args and kwargs:
        raise FilterArgumentError("can't handle positional and keyword arguments at the same time")
    if kwargs:
        return soft_str(value) % kwargs
    return soft_str(value) % args


def do_trim(value, chars=None):
    return soft_str(value).strip(chars)


def do_striptags(value):
    if hasattr(value, "__html__"):
        value = value.__html__()
    return Markup(str(value)).striptags()


def do_slice(value, slices, fill_with=None):
    seq = list(value)
    length = len(seq)
    items_per_slice = length // slices
    slices_with_extra = length % slices
    offset = 0
    for slice_number in range(slices):
        start = offset + slice_number * items_per_slice
        if slice_number < slices_with_extra:
            offset += 1
        end = offset + (slice_number + 1) * items_per_slice
        tmp = seq[start:end]
        if fill_with is not None and slice_number >= slices_with_extra:
            tmp.append(fill_with)
        yield tmp


def do_batch(value, linecount, fill_with=None):
    tmp = []
    for item in value:
        if len(tmp) == linecount:
            yield tmp
            tmp = []
        tmp.append(item)
    if tmp:
        if fill_with is not None and len(tmp) < linecount:
            tmp += [fill_with] * (linecount - len(tmp))
        yield tmp


def do_round(value, precision=0, method="common"):
    if method not in ("common", "ceil", "floor"):
        raise FilterArgumentError("method must be common, ceil or floor")
    if method == "common":
        return round(value, precision)
    if method == "ceil":
        return math.ceil(value * 10 ** precision) / 10 ** precision
    return math.floor(value * 10 ** precision) / 10 ** precision


class _GroupTuple:
    def __init__(self, grouper, list_):
        self.grouper = grouper
        self.list = list_

    def __iter__(self):
        return iter((self.grouper, self.list))

    def __getitem__(self, i):
        return (self.grouper, self.list)[i]

    def __len__(self):
        return 2

    def __eq__(self, other):
        return (self.grouper, self.list) == other

    def __repr__(self):
        return repr((self.grouper, self.list))

    def __str__(self):
        return str((self.grouper, self.list))


@pass_environment
def do_groupby(environment, value, attribute, default=None, case_sensitive=False):
    expr = make_attrgetter(environment, attribute, postprocess=_ignore_case if not case_sensitive else None, default=default)
    out = []
    for item in sorted(value, key=expr):
        key = expr(item)
        if out and out[-1].grouper == key:
            out[-1].list.append(item)
        else:
            out.append(_GroupTuple(key, [item]))
    if not case_sensitive:
        output_expr = make_attrgetter(environment, attribute, default=default)
        out = [_GroupTuple(output_expr(g.list[0]), g.list) for g in out]
    return out


@pass_environment
def do_sum(environment, iterable, attribute=None, start=0):
    if attribute is not None:
        getter = make_attrgetter(environment, attribute)
        iterable = [getter(x) for x in iterable]
    return sum(iterable, start)


def do_list(value):
    return list(value)


def do_mark_safe(value):
    return Markup(value)


def do_mark_unsafe(value):
    return str(value)


def do_reverse(value):
    if isinstance(value, str):
        return value[::-1]
    try:
        return reversed(value)
    except TypeError:
        try:
            rv = list(value)
            rv.reverse()
            return rv
        except TypeError:
            raise FilterArgumentError("argument must be iterable") from None


@pass_environment
def do_attr(environment, obj, name):
    try:
        return getattr(obj, name)
    except AttributeError:
        return environment.undefined(obj=obj, name=name)


@pass_context
def do_map(context, value, *args, **kwargs):
    if value:
        func = _prepare_map(context, args, kwargs)
        for item in value:
            yield func(item)


@pass_context
def do_select(context, value, *args, **kwargs):
    return _select_or_reject(context, value, args, kwargs, False, False)


@pass_context
def do_reject(context, value, *args, **kwargs):
    return _select_or_reject(context, value, args, kwargs, True, False)


@pass_context
def do_selectattr(context, value, *args, **kwargs):
    return _select_or_reject(context, value, args, kwargs, False, True)


@pass_context
def do_rejectattr(context, value, *args, **kwargs):
    return _select_or_reject(context, value, args, kwargs, True, True)


@pass_eval_context
def do_tojson(eval_ctx, value, indent=None):
    policies = eval_ctx.environment.policies
    dumps = policies["json.dumps_function"]
    kwargs = policies["json.dumps_kwargs"]
    if indent is not None:
        kwargs = dict(kwargs)
        kwargs["indent"] = indent
    return htmlsafe_json_dumps(value, dumps=dumps, **kwargs)


def _prepare_map(context, args, kwargs):
    if not args and "attribute" in kwargs:
        kwargs = dict(kwargs)
        attribute = kwargs.pop("attribute")
        default = kwargs.pop("default", None)
        if kwargs:
            raise FilterArgumentError("Unexpected keyword argument " + repr(next(iter(kwargs))))
        return make_attrgetter(context.environment, attribute, default=default)
    if not args:
        raise FilterArgumentError("map requires a filter argument")
    name = args[0]
    rest = args[1:]

    def func(item):
        return context.environment.call_filter(name, item, rest, kwargs, context=context)

    return func


def _prepare_select_or_reject(context, args, kwargs, negate, lookup_attr):
    if lookup_attr:
        if not args:
            raise FilterArgumentError("Missing parameter for attribute name")
        transfunc = make_attrgetter(context.environment, args[0])
        off = 1
    else:
        off = 0
        transfunc = None
    if len(args) > off:
        name = args[off]
        rest = args[1 + off:]

        def func(item):
            return context.environment.call_test(name, item, rest, kwargs, context)
    else:
        func = bool

    def pick(item):
        if transfunc is not None:
            item = transfunc(item)
        rv = func(item)
        if negate:
            return not rv
        return rv

    return pick


def _select_or_reject(context, value, args, kwargs, negate, lookup_attr):
    if value:
        func = _prepare_select_or_reject(context, args, kwargs, negate, lookup_attr)
        for item in value:
            if func(item):
                yield item


DEFAULT_FILTERS = {
    "abs": abs, "attr": do_attr, "batch": do_batch, "capitalize": do_capitalize, "center": do_center, "count": len,
    "d": do_default, "default": do_default, "dictsort": do_dictsort, "e": escape, "escape": escape,
    "filesizeformat": do_filesizeformat, "first": do_first, "float": do_float, "forceescape": do_forceescape,
    "format": do_format, "groupby": do_groupby, "indent": do_indent, "int": do_int, "join": do_join, "last": do_last,
    "length": len, "list": do_list, "lower": do_lower, "items": do_items, "map": do_map, "min": do_min, "max": do_max,
    "pprint": do_pprint, "random": do_random, "reject": do_reject, "rejectattr": do_rejectattr, "replace": do_replace,
    "reverse": do_reverse, "round": do_round, "safe": do_mark_safe, "select": do_select, "selectattr": do_selectattr,
    "slice": do_slice, "sort": do_sort, "string": soft_str, "striptags": do_striptags, "sum": do_sum, "title": do_title,
    "trim": do_trim, "truncate": do_truncate, "unique": do_unique, "upper": do_upper, "urlencode": do_urlencode,
    "urlize": do_urlize, "wordcount": do_wordcount, "wordwrap": do_wordwrap, "xmlattr": do_xmlattr, "tojson": do_tojson,
}


# -- tests


def test_odd(value):
    return value % 2 == 1


def test_even(value):
    return value % 2 == 0


def test_divisibleby(value, num):
    return value % num == 0


def test_defined(value):
    return not isinstance(value, Undefined)


def test_undefined(value):
    return isinstance(value, Undefined)


@pass_environment
def test_filter(env, value):
    return value in env.filters


@pass_environment
def test_test(env, value):
    return value in env.tests


def test_none(value):
    return value is None


def test_boolean(value):
    return value is True or value is False


def test_false(value):
    return value is False


def test_true(value):
    return value is True


def test_integer(value):
    return isinstance(value, int) and value is not True and value is not False


def test_float(value):
    return isinstance(value, float)


def test_lower(value):
    return str(value).islower()


def test_upper(value):
    return str(value).isupper()


def test_string(value):
    return isinstance(value, str)


def _is_mapping(value):
    if isinstance(value, dict):
        return True
    if isinstance(value, (str, list, tuple, bytes, set, frozenset, int, float)):
        return False
    if isinstance(value, (Undefined, Namespace, TemplateModule, Context)):
        return isinstance(value, Context)
    return hasattr(value, "keys") and hasattr(value, "items") and hasattr(value, "__getitem__") and hasattr(value, "__len__") and hasattr(value, "__iter__") and hasattr(value, "get") and hasattr(value, "values") and hasattr(value, "__contains__")


def test_mapping(value):
    return _is_mapping(value)


def test_number(value):
    if isinstance(value, (int, float)):
        return True
    return type(value).__name__ in ("Decimal", "Fraction", "complex")


def test_sequence(value):
    try:
        len(value)
    except Exception:
        return False
    if isinstance(value, (str, list, tuple, dict, range, bytes)):
        return True
    if isinstance(value, Undefined):
        return isinstance(value, ChainableUndefined)
    return hasattr(value, "__getitem__")


def test_sameas(value, other):
    return value is other


def test_iterable(value):
    try:
        iter(value)
    except TypeError:
        return False
    return True


def test_escaped(value):
    return hasattr(value, "__html__")


def test_in(value, seq):
    return value in seq


def _op_eq(a, b):
    return a == b


def _op_ne(a, b):
    return a != b


def _op_gt(a, b):
    return a > b


def _op_ge(a, b):
    return a >= b


def _op_lt(a, b):
    return a < b


def _op_le(a, b):
    return a <= b


DEFAULT_TESTS = {
    "odd": test_odd, "even": test_even, "divisibleby": test_divisibleby, "defined": test_defined,
    "undefined": test_undefined, "filter": test_filter, "test": test_test, "none": test_none, "boolean": test_boolean,
    "false": test_false, "true": test_true, "integer": test_integer, "float": test_float, "lower": test_lower,
    "upper": test_upper, "string": test_string, "mapping": test_mapping, "number": test_number,
    "sequence": test_sequence, "iterable": test_iterable, "callable": callable, "sameas": test_sameas,
    "escaped": test_escaped, "in": test_in, "==": _op_eq, "eq": _op_eq, "equalto": _op_eq, "!=": _op_ne,
    "ne": _op_ne, ">": _op_gt, "gt": _op_gt, "greaterthan": _op_gt, "ge": _op_ge, ">=": _op_ge, "<": _op_lt,
    "lt": _op_lt, "lessthan": _op_lt, "<=": _op_le, "le": _op_le,
}

DEFAULT_NAMESPACE = {
    "range": range, "dict": dict, "lipsum": generate_lorem_ipsum, "cycler": Cycler, "joiner": Joiner,
    "namespace": Namespace,
}


def _default_policies():
    return {
        "compiler.ascii_str": True, "urlize.rel": "noopener", "urlize.target": None, "urlize.extra_schemes": None,
        "truncate.leeway": 5, "json.dumps_function": None, "json.dumps_kwargs": {"sort_keys": True},
        "ext.i18n.trimmed": False,
    }


# -- extensions


class Extension:
    tags = set()
    priority = 100
    identifier = "jinja2.ext.Extension"

    def __init__(self, environment):
        self.environment = environment


class ExprStmtExtension(Extension):
    tags = {"do"}
    identifier = "jinja2.ext.ExprStmtExtension"


class LoopControlExtension(Extension):
    tags = {"break", "continue"}
    identifier = "jinja2.ext.LoopControlExtension"


class DebugExtension(Extension):
    tags = {"debug"}
    identifier = "jinja2.ext.DebugExtension"


_EXT_NAMES = {
    "jinja2.ext.do": ExprStmtExtension, "jinja2.ext.ExprStmtExtension": ExprStmtExtension,
    "jinja2.ext.loopcontrols": LoopControlExtension, "jinja2.ext.LoopControlExtension": LoopControlExtension,
}


def _load_extension(ext):
    if isinstance(ext, str):
        cls = _EXT_NAMES.get(ext)
        if cls is None:
            _lacks("the extension " + repr(ext))
        return cls
    if ext is ExprStmtExtension or ext is LoopControlExtension:
        return ext
    _lacks("custom extensions")


# -- environment and templates


class Environment:
    sandboxed = False
    overlayed = False
    linked_to = None
    shared = False
    is_async = False

    def __init__(self, block_start_string="{%", block_end_string="%}", variable_start_string="{{", variable_end_string="}}",
                 comment_start_string="{#", comment_end_string="#}", line_statement_prefix=None, line_comment_prefix=None,
                 trim_blocks=False, lstrip_blocks=False, newline_sequence="\n", keep_trailing_newline=False, extensions=(),
                 optimized=True, undefined=Undefined, finalize=None, autoescape=False, loader=None, cache_size=400,
                 auto_reload=True, bytecode_cache=None, enable_async=False):
        self.block_start_string = block_start_string
        self.block_end_string = block_end_string
        self.variable_start_string = variable_start_string
        self.variable_end_string = variable_end_string
        self.comment_start_string = comment_start_string
        self.comment_end_string = comment_end_string
        self.line_statement_prefix = line_statement_prefix
        self.line_comment_prefix = line_comment_prefix
        self.trim_blocks = trim_blocks
        self.lstrip_blocks = lstrip_blocks
        self.newline_sequence = newline_sequence
        self.keep_trailing_newline = keep_trailing_newline
        self.undefined = undefined
        self.optimized = optimized
        self.finalize = finalize
        self.autoescape = autoescape
        self.filters = dict(DEFAULT_FILTERS)
        self.tests = dict(DEFAULT_TESTS)
        self.globals = dict(DEFAULT_NAMESPACE)
        self.loader = loader
        self.cache_size = cache_size
        self.cache = {} if cache_size != 0 else None
        self.bytecode_cache = bytecode_cache
        self.auto_reload = auto_reload
        self.policies = _default_policies()
        self.extensions = {}
        self._ext_tags = set()
        if enable_async:
            _lacks("async rendering")
        if bytecode_cache is not None:
            _lacks("bytecode caches")
        for ext in extensions:
            self.add_extension(ext)
        if not (self.block_start_string != self.variable_start_string != self.comment_start_string):
            raise AssertionError("block, variable and comment start strings must be different.")
        if self.newline_sequence not in ("\r", "\r\n", "\n"):
            raise AssertionError("'newline_sequence' must be one of '\\n', '\\r\\n', or '\\r'.")

    def add_extension(self, extension):
        cls = _load_extension(extension)
        inst = cls(self)
        self.extensions[cls.identifier] = inst
        for tag in cls.tags:
            self._ext_tags.add(tag)

    def extend(self, **attributes):
        for key, value in attributes.items():
            if not hasattr(self, key):
                setattr(self, key, value)

    @property
    def lexer(self):
        return get_lexer(self)

    def getitem(self, obj, argument):
        try:
            return obj[argument]
        except (AttributeError, TypeError, LookupError):
            if isinstance(argument, str):
                try:
                    return getattr(obj, argument)
                except AttributeError:
                    pass
            return self.undefined(obj=obj, name=argument)

    def getattr(self, obj, attribute):
        # a plain dict has no attribute by a name like this, so getattr() would raise and obj[attribute] decide:
        # go there directly instead of raising and catching an AttributeError on every {{ m.role }}
        if type(obj) is dict and type(attribute) is str and attribute[:1] != "_" and attribute not in _DICT_ATTRS:
            try:
                return obj[attribute]
            except (TypeError, LookupError, AttributeError):
                return self.undefined(obj=obj, name=attribute)
        try:
            return getattr(obj, attribute)
        except AttributeError:
            pass
        try:
            return obj[attribute]
        except (TypeError, LookupError, AttributeError):
            return self.undefined(obj=obj, name=attribute)

    def _filter_test_common(self, name, value, args, kwargs, context, eval_ctx, is_filter):
        if is_filter:
            env_map = self.filters
            type_name = "filter"
        else:
            env_map = self.tests
            type_name = "test"
        func = env_map.get(name)
        if func is None:
            msg = "No " + type_name + " named " + repr(name) + "."
            if isinstance(name, Undefined):
                try:
                    name._fail_with_undefined_error()
                except Exception as e:
                    msg = msg + " (" + str(e) + "; did you forget to quote the callable name?)"
            raise TemplateRuntimeError(msg)
        args = [value] + list(args if args is not None else ())
        kwargs = kwargs if kwargs is not None else {}
        pa = _pass_arg(func)
        if pa == _PASS_CONTEXT:
            if context is None:
                raise TemplateRuntimeError("Attempted to invoke a context " + type_name + " without context.")
            args.insert(0, context)
        elif pa == _PASS_EVAL_CONTEXT:
            if eval_ctx is None:
                if context is not None:
                    eval_ctx = context.eval_ctx
                else:
                    eval_ctx = EvalContext(self)
            args.insert(0, eval_ctx)
        elif pa == _PASS_ENVIRONMENT:
            args.insert(0, self)
        return func(*args, **kwargs)

    def call_filter(self, name, value, args=None, kwargs=None, context=None, eval_ctx=None):
        return self._filter_test_common(name, value, args, kwargs, context, eval_ctx, True)

    def call_test(self, name, value, args=None, kwargs=None, context=None, eval_ctx=None):
        return self._filter_test_common(name, value, args, kwargs, context, eval_ctx, False)

    def _tokenize(self, source, name, filename=None, state=None):
        return self.lexer.tokenize(source, name, filename, state)

    def _parse(self, source, name, filename):
        return Parser(self, source, name, filename).parse()

    def _compile(self, source, name=None, filename=None):
        try:
            if isinstance(source, str):
                node = self._parse(source, name, filename)
            else:
                node = source
            blocks = _check_template(node, self, name, filename)
        except TemplateSyntaxError as e:
            if not e.translated:
                e.translated = True
                e.source = source if isinstance(source, str) else None
            raise
        return node, blocks

    def compile_expression(self, source, undefined_to_none=True):
        parser = Parser(self, source, state="variable")
        try:
            expr = parser.parse_expression()
            if not parser.stream.eos:
                raise TemplateSyntaxError("chunk after expression", parser.stream.current.lineno, None, None)
        except TemplateSyntaxError as e:
            e.translated = True
            e.source = source
            raise
        body = [Assign(Name("result", "store"), expr, lineno=1)]
        template = self.from_string(TemplateNode(body, lineno=1))
        return TemplateExpression(template, undefined_to_none)

    def join_path(self, template, parent):
        return template

    def _load_template(self, name, globals):
        if self.loader is None:
            raise TypeError("no loader for this environment specified")
        if self.cache is not None:
            template = self.cache.get(name)
            if template is not None and (not self.auto_reload or template.is_up_to_date):
                if globals:
                    for k, v in globals.items():
                        template.globals[k] = v
                return template
        template = self.loader.load(self, name, self.make_globals(globals))
        if self.cache is not None:
            self.cache[name] = template
        return template

    def get_template(self, name, parent=None, globals=None):
        if isinstance(name, Template):
            return name
        if parent is not None:
            name = self.join_path(name, parent)
        return self._load_template(name, globals)

    def select_template(self, names, parent=None, globals=None):
        if isinstance(names, Undefined):
            names._fail_with_undefined_error()
        if not names:
            raise TemplatesNotFound(message="Tried to select from an empty list of templates.")
        for name in names:
            if isinstance(name, Template):
                return name
            if parent is not None:
                name = self.join_path(name, parent)
            try:
                return self._load_template(name, globals)
            except (TemplateNotFound, UndefinedError):
                pass
        raise TemplatesNotFound(names)

    def get_or_select_template(self, template_name_or_list, parent=None, globals=None):
        if isinstance(template_name_or_list, (str, Undefined)):
            return self.get_template(template_name_or_list, parent, globals)
        elif isinstance(template_name_or_list, Template):
            return template_name_or_list
        return self.select_template(template_name_or_list, parent, globals)

    def from_string(self, source, globals=None, template_class=None):
        gs = self.make_globals(globals)
        node, blocks = self._compile(source)
        cls = template_class or Template
        return cls(_Compiled(self, node, blocks, gs, None, None, None))

    def make_globals(self, d):
        if d is None:
            d = {}
        return _ChainMap(d, self.globals)

    def list_templates(self, extensions=None, filter_func=None):
        assert self.loader is not None, "No loader configured."
        names = self.loader.list_templates()
        if extensions is not None:
            if filter_func is not None:
                raise TypeError("either extensions or filter_func can be passed, but not both")

            def filter_func(x):
                return "." in x and x.rsplit(".", 1)[1] in extensions

        if filter_func is not None:
            names = [name for name in names if filter_func(name)]
        return names


class _Compiled:
    def __init__(self, env, node, blocks, globals, name, filename, uptodate):
        self.env = env
        self.node = node
        self.blocks = blocks
        self.globals = globals
        self.name = name
        self.filename = filename
        self.uptodate = uptodate


_spontaneous = {}


class Template:
    def __init__(self, source, block_start_string="{%", block_end_string="%}", variable_start_string="{{",
                 variable_end_string="}}", comment_start_string="{#", comment_end_string="#}",
                 line_statement_prefix=None, line_comment_prefix=None, trim_blocks=False, lstrip_blocks=False,
                 newline_sequence="\n", keep_trailing_newline=False, extensions=(), optimized=True, undefined=Undefined,
                 finalize=None, autoescape=False, enable_async=False):
        if not isinstance(source, _Compiled):
            key = (block_start_string, block_end_string, variable_start_string, variable_end_string,
                   comment_start_string, comment_end_string, line_statement_prefix, line_comment_prefix, trim_blocks,
                   lstrip_blocks, newline_sequence, keep_trailing_newline, tuple(extensions), optimized, id(undefined),
                   id(finalize), autoescape if not callable(autoescape) else id(autoescape), enable_async)
            env = _spontaneous.get(key)
            if env is None:
                env = Environment(block_start_string, block_end_string, variable_start_string, variable_end_string,
                                  comment_start_string, comment_end_string, line_statement_prefix, line_comment_prefix,
                                  trim_blocks, lstrip_blocks, newline_sequence, keep_trailing_newline, extensions,
                                  optimized, undefined, finalize, autoescape, None, 0, False, None, enable_async)
                env.shared = True
                _spontaneous[key] = env
            gs = env.make_globals(None)
            node, blocks = env._compile(source)
            source = _Compiled(env, node, blocks, gs, None, None, None)
        self.environment = source.env
        self.globals = source.globals
        self.name = source.name
        self.filename = source.filename
        self._node = source.node
        self.blocks = {}
        for bname, bnode in source.blocks.items():
            self.blocks[bname] = _BlockFunc(self, bnode)
        self._module = None
        self._uptodate = source.uptodate

    def render(self, *args, **kwargs):
        ctx = self.new_context(dict(*args, **kwargs))
        try:
            return _concat(self._render_root(ctx))
        except TemplateSyntaxError as e:
            if not e.translated:
                e.translated = True
            raise

    def render_async(self, *args, **kwargs):
        _lacks("async rendering")

    def generate(self, *args, **kwargs):
        _lacks("Template.generate")

    def stream(self, *args, **kwargs):
        _lacks("Template.stream")

    def _render_root(self, ctx):
        r = _Render(self, ctx)
        f = _Frame(r, None, True)
        f.toplevel = True
        f.require_output_check = True
        buf = []
        r.body(self._node.body, f, buf)
        if r.parent_template is not None:
            for piece in r.parent_template._render_root(ctx):
                buf.append(piece)
        return buf

    def new_context(self, vars=None, shared=False, locals=None):
        return new_context(self.environment, self.name, self.blocks, vars, shared, self.globals, locals)

    def make_module(self, vars=None, shared=False, locals=None):
        ctx = self.new_context(vars, shared, locals)
        return TemplateModule(self, ctx)

    def _get_default_module(self, ctx=None):
        if ctx is not None:
            keys = [k for k in ctx.globals_keys if k not in self.globals]
            if keys:
                return self.make_module({k: ctx.parent[k] for k in keys})
        if self._module is None:
            self._module = self.make_module()
        return self._module

    @property
    def module(self):
        return self._get_default_module()

    @property
    def is_up_to_date(self):
        if self._uptodate is None:
            return True
        return self._uptodate()

    def __repr__(self):
        if self.name is None:
            return "<Template memory:" + format(id(self), "x") + ">"
        return "<Template " + repr(self.name) + ">"


class TemplateExpression:
    def __init__(self, template, undefined_to_none):
        self._template = template
        self._undefined_to_none = undefined_to_none

    def __call__(self, *args, **kwargs):
        context = self._template.new_context(dict(*args, **kwargs))
        self._template._render_root(context)
        rv = context.vars["result"]
        if self._undefined_to_none and isinstance(rv, Undefined):
            rv = None
        return rv


_OWN_CLASSES = [
    (Namespace, "jinja2.utils.Namespace"), (Cycler, "jinja2.utils.Cycler"), (Joiner, "jinja2.utils.Joiner"),
    (LoopContext, "jinja2.runtime.LoopContext"), (Macro, "jinja2.runtime.Macro"), (Context, "jinja2.runtime.Context"),
    (TemplateModule, "jinja2.environment.TemplateModule"), (TemplateReference, "jinja2.runtime.TemplateReference"),
    (BlockReference, "jinja2.runtime.BlockReference"), (Template, "jinja2.environment.Template"),
    (Environment, "jinja2.environment.Environment"), (EvalContext, "jinja2.nodes.EvalContext"),
    (_GroupTuple, "jinja2.filters._GroupTuple"), (TemplateExpression, "jinja2.environment.TemplateExpression"),
]


# -- loaders


class BaseLoader:
    has_source_access = True

    def get_source(self, environment, template):
        if not self.has_source_access:
            raise RuntimeError(type(self).__name__ + " cannot provide access to the source")
        raise TemplateNotFound(template)

    def list_templates(self):
        raise TypeError("this loader cannot iterate over all templates")

    def load(self, environment, name, globals=None):
        if globals is None:
            globals = {}
        source, filename, uptodate = self.get_source(environment, name)
        node, blocks = environment._compile(source, name, filename)
        return Template(_Compiled(environment, node, blocks, globals, name, filename, uptodate))


class DictLoader(BaseLoader):
    def __init__(self, mapping):
        self.mapping = mapping

    def get_source(self, environment, template):
        if template in self.mapping:
            source = self.mapping[template]
            mapping = self.mapping

            def uptodate():
                return source == mapping.get(template)

            return source, None, uptodate
        raise TemplateNotFound(template)

    def list_templates(self):
        return sorted(self.mapping)


class FunctionLoader(BaseLoader):
    def __init__(self, load_func):
        self.load_func = load_func

    def get_source(self, environment, template):
        rv = self.load_func(template)
        if rv is None:
            raise TemplateNotFound(template)
        if isinstance(rv, str):
            return rv, None, None
        return rv


class PrefixLoader(BaseLoader):
    def __init__(self, mapping, delimiter="/"):
        self.mapping = mapping
        self.delimiter = delimiter

    def get_loader(self, template):
        try:
            prefix, name = template.split(self.delimiter, 1)
            loader = self.mapping[prefix]
        except (ValueError, KeyError):
            raise TemplateNotFound(template) from None
        return loader, name

    def get_source(self, environment, template):
        loader, name = self.get_loader(template)
        try:
            return loader.get_source(environment, name)
        except TemplateNotFound:
            raise TemplateNotFound(template) from None

    def load(self, environment, name, globals=None):
        loader, local_name = self.get_loader(name)
        try:
            return loader.load(environment, local_name, globals)
        except TemplateNotFound:
            raise TemplateNotFound(name) from None

    def list_templates(self):
        result = []
        for prefix, loader in self.mapping.items():
            for template in loader.list_templates():
                result.append(prefix + self.delimiter + template)
        return result


class ChoiceLoader(BaseLoader):
    def __init__(self, loaders):
        self.loaders = loaders

    def get_source(self, environment, template):
        for loader in self.loaders:
            try:
                return loader.get_source(environment, template)
            except TemplateNotFound:
                pass
        raise TemplateNotFound(template)

    def load(self, environment, name, globals=None):
        for loader in self.loaders:
            try:
                return loader.load(environment, name, globals)
            except TemplateNotFound:
                pass
        raise TemplateNotFound(name)

    def list_templates(self):
        found = set()
        for loader in self.loaders:
            for t in loader.list_templates():
                found.add(t)
        return sorted(found)


class FileSystemLoader(BaseLoader):
    def __init__(self, searchpath, encoding="utf-8", followlinks=False):
        _lacks("FileSystemLoader")


class PackageLoader(BaseLoader):
    def __init__(self, package_name, package_path="templates", encoding="utf-8"):
        _lacks("PackageLoader")


class ModuleLoader(BaseLoader):
    def __init__(self, path):
        _lacks("ModuleLoader")


def is_undefined(obj):
    return isinstance(obj, Undefined)


def clear_caches():
    for env in _spontaneous.values():
        if env.cache is not None:
            env.cache.clear()
    _spontaneous.clear()
    _lexer_cache.clear()
