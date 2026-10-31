import sys

KNOWN = {"push", "pop", "dup", "swap", "add", "sub", "mul", "div", "mod",
         "jmp", "jz", "jnz", "call", "ret", "print", "halt"}
JUMPS = {"jmp", "jz", "jnz", "call"}
BINOPS = {"add", "sub", "mul", "div", "mod"}


def wrap(v):
    v &= (1 << 64) - 1
    if v >= (1 << 63):
        v -= (1 << 64)
    return v


def trunc_divmod(a, b):
    q = abs(a) // abs(b)
    if (a < 0) != (b < 0):
        q = -q
    return q, a - b * q


def main():
    lines = sys.stdin.read().split("\n")
    prog = []  # (lineno, op, arg)
    labels = {}
    for n, raw in enumerate(lines, 1):
        s = raw.split("#", 1)[0].strip()
        if s == "":
            continue
        if s.endswith(":") and len(s.split()) == 1:
            labels[s[:-1]] = len(prog)
            continue
        parts = s.split()
        prog.append((n, parts[0], parts[1] if len(parts) > 1 else None))

    out = []

    for n, op, arg in prog:
        if op not in KNOWN:
            out.append("error line %d: unknown instruction %s" % (n, op))
            print("\n".join(out))
            return
    for n, op, arg in prog:
        if op in JUMPS and arg not in labels:
            out.append("error line %d: unknown label %s" % (n, arg))
            print("\n".join(out))
            return

    stack = []
    calls = []
    pc = 0
    steps = 0
    limit = 10000

    def finish():
        if out:
            sys.stdout.write("\n".join(out) + "\n")

    while pc < len(prog):
        if steps == limit:
            out.append("error: step limit")
            finish()
            return
        n, op, arg = prog[pc]
        steps += 1
        nxt = pc + 1

        def need(k):
            if len(stack) < k:
                out.append("error line %d: stack underflow" % n)
                finish()
                return False
            return True

        if op == "push":
            stack.append(wrap(int(arg)))
        elif op == "pop":
            if not need(1):
                return
            stack.pop()
        elif op == "dup":
            if not need(1):
                return
            stack.append(stack[-1])
        elif op == "swap":
            if not need(2):
                return
            stack[-1], stack[-2] = stack[-2], stack[-1]
        elif op in BINOPS:
            if not need(2):
                return
            b = stack.pop()
            a = stack.pop()
            if op == "add":
                r = a + b
            elif op == "sub":
                r = a - b
            elif op == "mul":
                r = a * b
            else:
                if b == 0:
                    out.append("error line %d: division by zero" % n)
                    finish()
                    return
                q, m = trunc_divmod(a, b)
                r = q if op == "div" else m
            stack.append(wrap(r))
        elif op == "jmp":
            nxt = labels[arg]
        elif op == "jz":
            if not need(1):
                return
            if stack.pop() == 0:
                nxt = labels[arg]
        elif op == "jnz":
            if not need(1):
                return
            if stack.pop() != 0:
                nxt = labels[arg]
        elif op == "call":
            calls.append(pc + 1)
            nxt = labels[arg]
        elif op == "ret":
            if not calls:
                break
            nxt = calls.pop()
        elif op == "print":
            if not need(1):
                return
            out.append(str(stack.pop()))
        elif op == "halt":
            break
        pc = nxt

    finish()


if __name__ == "__main__":
    main()
