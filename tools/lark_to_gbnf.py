"""Writes grammar/brevi.gbnf (llama.cpp GBNF) from grammar/brevi.lark. usage: lark_to_gbnf.py [--check]

GBNF has no %ignore, so optional whitespace and comments (`ws`) go between the parts of every rule.
Regex terminals are written by hand below, one per Lark terminal. With --check, exits 1 if the
committed .gbnf differs from what this script makes (CI runs it so the two never drift).
"""
import re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LARK = ROOT / "grammar" / "brevi.lark"
GBNF = ROOT / "grammar" / "brevi.gbnf"

TERMS = {
    "NAME": r'[A-Za-z_] [A-Za-z0-9_]*',
    "INT": r'[0-9] [0-9_]*',
    "FLOAT": r'[0-9] [0-9_]* "." [0-9]+ ([eE] [+-]? [0-9]+)? | [0-9] [0-9_]* [eE] [+-]? [0-9]+',
    "STRING": r'[fF]? "\"" ([^"\\\n{] | "\\" [^\n] | "{" [^}"\n]* ("\"" [^"\n]* "\"" [^}"\n]*)* "}" | "{")* "\""'
              r''' | [fF]? "'" ([^'\\\n{] | "\\" [^\n] | "{" [^}'\n]* ("'" [^'\n]* "'" [^}'\n]*)* "}" | "{")* "'"''',
    "ASSIGN": r'"=" | "+=" | "-=" | "*=" | "/="',
    "_NL": r'("\r"? "\n" [ \t]* (("#" | "//") [^\n]*)?)+',
}
WS = r'ws ::= ([ \t\f] | "#" [^\n]* | "//" [^\n]*)*'

TOKEN = re.compile(r'\s+|//[^\n]*|"(?:\\.|[^"\\])*"|/(?:\\.|[^/\\\n])+/|[A-Za-z_][A-Za-z0-9_]*|[()\[\]|?*+:]|%[a-z]+|.')


def gname(n):
    if n in TERMS:
        return "t-" + n.strip("_").lower()
    return n.strip("_").replace("_", "-") or "x"


def tokens(body):
    return [t for t in TOKEN.findall(body) if t.strip() and not t.startswith("//")]


def rules(text):
    out, cur = [], None
    for line in text.splitlines():
        if re.match(r"^\??[a-z_]+\s*:", line):
            name, body = line.split(":", 1)
            cur = [name.lstrip("?").strip(), body]
            out.append(cur)
        elif cur and line.startswith((" ", "\t")):
            cur[1] += "\n" + line
        else:
            cur = None
    return out


def seq(toks, i, stop):
    """One alternative: returns (gbnf text, next index)."""
    parts = []
    while i < len(toks) and toks[i] not in stop:
        t = toks[i]
        if t in ("(", "["):
            inner, many = alts(toks, i + 1, ")" if t == "(" else "]")
            i = many[1]
            q = "?" if t == "[" else ""
            i += 1
        else:
            inner = gname(t) if re.match(r"[A-Za-z_]", t) else t
            many = (False, i)
            i += 1
            q = ""
        if i < len(toks) and toks[i] in ("?", "*", "+") and not q:
            q = toks[i]
            i += 1
        if q:
            parts.append(f"({'' if not parts else 'ws '}{inner}){q}")
        else:
            parts.append(("ws " if parts else "") + (f"({inner})" if many[0] else inner))
    return " ".join(parts), i


def alts(toks, i, close):
    out = []
    while True:
        s, i = seq(toks, i, ("|", close))
        out.append(s)
        if i < len(toks) and toks[i] == "|":
            i += 1
            continue
        return " | ".join(out), (len(out) > 1, i)


def convert():
    lines = ["# Brevi grammar for llama.cpp, made from brevi.lark by tools/lark_to_gbnf.py. Do not edit by hand.",
             "root ::= ws start ws"]
    for name, body in rules(LARK.read_text()):
        g, _ = alts(tokens(body), 0, None)
        lines.append(f"{gname(name)} ::= {g}")
    for n, g in TERMS.items():
        lines.append(f"{gname(n)} ::= {g}")
    lines.append(WS)
    return "\n".join(lines) + "\n"


text = convert()
if "--check" in sys.argv:
    if not GBNF.exists() or GBNF.read_text() != text:
        print("grammar/brevi.gbnf is out of date: run python3 tools/lark_to_gbnf.py")
        sys.exit(1)
    print("grammar/brevi.gbnf matches brevi.lark")
else:
    GBNF.write_text(text)
    print(f"wrote {GBNF.relative_to(ROOT)}")
