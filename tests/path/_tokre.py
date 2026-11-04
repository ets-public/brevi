import re

# A triple-quoted pattern whose lines start with ( and |, as a module-level name.
TOKEN_RE = re.compile(r'''\s*(?:
 (?P<num>\d+)
|(?P<id>[a-z]+)
)''', re.X)


def tokens(s):
    return [m.group(0).strip() for m in TOKEN_RE.finditer(s)]
