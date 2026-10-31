import sys

OPS0 = {"pop", "dup", "swap", "add", "sub", "mul", "div", "mod", "ret", "print", "halt"}
OPS_LABEL = {"jmp", "jz", "jnz", "call"}
LIMIT = 10000


def wrap(x):
    return ((x + (1 << 63)) % (1 << 64)) - (1 << 63)


def tdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def main():
    raw_lines = sys.stdin.read().split("\n")
    prog = []  # (lineno, tokens)
    labels = {}
    for lineno, raw in enumerate(raw_lines, 1):
        line = raw.split("#", 1)[0].strip()
        if line == "":
            continue
        if line.endswith(":") and " " not in line and len(line) > 1:
            name = line[:-1]
            if name not in labels:
                labels[name] = len(prog)
            continue
        prog.append((lineno, line.split()))

    def out_error(msg):
        sys.stdout.write(msg + "\n")

    # Pass 1: unknown instructions.
    for lineno, toks in prog:
        op = toks[0]
        ok = False
        if op in OPS0 and len(toks) == 1:
            ok = True
        elif op == "push" and len(toks) == 2:
            try:
                int(toks[1])
                ok = True
            except ValueError:
                ok = False
        elif op in OPS_LABEL and len(toks) == 2:
            ok = True
        if not ok:
            out_error("error line %d: unknown instruction %s" % (lineno, op))
            return

    # Pass 2: jumps to undefined labels.
    for lineno, toks in prog:
        if toks[0] in OPS_LABEL and toks[1] not in labels:
            out_error("error line %d: unknown label %s" % (lineno, toks[1]))
            return

    stack = []
    calls = []
    out = []
    pc = 0
    steps = 0
    n = len(prog)

    def emit():
        if out:
            sys.stdout.write("\n".join(out) + "\n")

    while pc < n:
        lineno, toks = prog[pc]
        if steps >= LIMIT:
            emit()
            out_error("error: step limit")
            return
        steps += 1
        op = toks[0]

        def need(k):
            if len(stack) < k:
                emit()
                out_error("error line %d: stack underflow" % lineno)
                return False
            return True

        nxt = pc + 1
        if op == "push":
            stack.append(wrap(int(toks[1])))
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
        elif op in ("add", "sub", "mul", "div", "mod"):
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
                    emit()
                    out_error("error line %d: division by zero" % lineno)
                    return
                q = tdiv(a, b)
                r = q if op == "div" else a - b * q
            stack.append(wrap(r))
        elif op == "jmp":
            nxt = labels[toks[1]]
        elif op in ("jz", "jnz"):
            if not need(1):
                return
            v = stack.pop()
            if (v == 0) == (op == "jz"):
                nxt = labels[toks[1]]
        elif op == "call":
            calls.append(pc + 1)
            nxt = labels[toks[1]]
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

    emit()


if __name__ == "__main__":
    main()
