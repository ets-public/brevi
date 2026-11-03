import re
m = re.match(r'(a)(b)?(c)?', 'ac')
x = m.group(2)
print(x, m.group(2) is None, m.group(3) is not None, x == None, m.group(1) is None)
print(m.group(2) or "-", [m.group(i) for i in range(4)])
a, b, c = m.groups()
print(a, b is None, c)
print(re.sub(r'(x)(y)?', r'[\1\2]', 'xa xy'), re.findall(r'q(z)?', 'qq qz'))
for mm in re.finditer(r'(\d)(\.\d)?', '1 2.5'):
    frac = mm.group(2)
    print(mm.group(1), "none" if frac is None else frac)
def f(s):
    m = re.fullmatch(r'(\w+)(?:=(\w+))?', s)
    if m.group(2) is None:
        return m.group(1) + "!"
    return m.group(1) + ":" + m.group(2)
print(f("ab"), f("a=b"))


def repl(m):
    return m.group(1).upper() + "!"

def bold(m):
    t = m.group(2)
    if t is None:
        return "<" + m.group(1) + ">"
    return "<b>" + t + "</b>"

def main():
    print(re.sub(r'(\w)(\w*)', repl, "hello big world"))
    print(re.sub(r'\*(\w+)(?:=(\w+))?\*', bold, "*a* and *b=c*"))
    print(re.sub(r'\d+', lambda m: str(int(m.group()) * 2), "a1 b22"))
    env = {"x": "1"}
    print(re.sub(r'\$(\w+)', lambda m: env.get(m.group(1), m.group(0)), "$x $y"))
main()


class Cyc(Exception):
    pass


vals = {"a": "${b}!", "b": "x", "c": "${d}", "d": "${c}"}


def resolve(k, stack):
    if k in stack:
        raise Cyc()
    def rep(m):
        return resolve(m.group(1), stack | {k})
    return re.sub(r'\$\{(\w+)\}', rep, vals[k])


def get(k):
    try:
        return resolve(k, frozenset())
    except Cyc:
        return "<cycle>"


print(get("a"), get("c"), re.sub(r'\x00(\d)\x00', r'<\1>', 'a\x001\x00b'))
