import sys

def wrap(x):
    x &= (1 << 64) - 1
    return x - (1 << 64) if x >= (1 << 63) else x

prog = []  # (lineno, op, arg)
labels = {}
NOARG = {"pop", "dup", "swap", "add", "sub", "mul", "div", "mod", "ret", "print", "halt"}
JUMPS = {"jmp", "jz", "jnz", "call"}
err = None
for ln, raw in enumerate(sys.stdin.read().split("\n"), 1):
    s = raw.split("#", 1)[0].strip()
    if not s:
        continue
    if s.endswith(":") and len(s.split()) == 1:
        labels[s[:-1]] = len(prog)
        continue
    p = s.split()
    op = p[0]
    arg = None
    ok = False
    if op in NOARG and len(p) == 1:
        ok = True
    elif op in JUMPS and len(p) == 2:
        ok = True
        arg = p[1]
    elif op == "push" and len(p) == 2:
        try:
            arg = int(p[1])
            ok = True
        except ValueError:
            pass
    if not ok:
        if err is None:
            err = "error line %d: unknown instruction %s" % (ln, op)
        break
    prog.append((ln, op, arg))

if err is None:
    for ln, op, arg in prog:
        if op in JUMPS and arg not in labels:
            err = "error line %d: unknown label %s" % (ln, arg)
            break
if err:
    print(err)
    sys.exit(0)

out = []
st = []
cs = []
pc = 0
steps = 0
need = {"pop": 1, "dup": 1, "swap": 2, "add": 2, "sub": 2, "mul": 2, "div": 2, "mod": 2,
        "jz": 1, "jnz": 1, "print": 1}
while pc < len(prog):
    if steps >= 10000:
        out.append("error: step limit")
        break
    steps += 1
    ln, op, arg = prog[pc]
    pc += 1
    if len(st) < need.get(op, 0):
        out.append("error line %d: stack underflow" % ln)
        break
    if op == "push":
        st.append(arg)
    elif op == "pop":
        st.pop()
    elif op == "dup":
        st.append(st[-1])
    elif op == "swap":
        st[-1], st[-2] = st[-2], st[-1]
    elif op in ("add", "sub", "mul", "div", "mod"):
        b = st.pop()
        a = st.pop()
        if op == "add":
            r = a + b
        elif op == "sub":
            r = a - b
        elif op == "mul":
            r = a * b
        else:
            if b == 0:
                out.append("error line %d: division by zero" % ln)
                break
            q = abs(a) // abs(b)
            if (a < 0) != (b < 0):
                q = -q
            r = q if op == "div" else a - q * b
        st.append(wrap(r))
    elif op == "jmp":
        pc = labels[arg]
    elif op == "jz":
        if st.pop() == 0:
            pc = labels[arg]
    elif op == "jnz":
        if st.pop() != 0:
            pc = labels[arg]
    elif op == "call":
        cs.append(pc)
        pc = labels[arg]
    elif op == "ret":
        if not cs:
            break
        pc = cs.pop()
    elif op == "print":
        out.append(str(st.pop()))
    elif op == "halt":
        break
if out:
    print("\n".join(out))
