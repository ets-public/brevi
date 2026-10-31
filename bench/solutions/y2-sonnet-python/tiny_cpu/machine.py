def run(instrs, labels, inputs):
    """Returns (output, error, steps, regs)."""
    regs = [0] * 8
    out = []
    data = []
    calls = []
    inp = list(inputs)
    ip = 0
    pc = 0
    steps = 0
    error = None

    def val(x):
        if x[0] == 'r' and len(x) == 2:
            return regs[int(x[1])]
        return int(x)

    def tdiv(a, b):
        q = abs(a) // abs(b)
        return q if (a >= 0) == (b >= 0) else -q

    while pc < len(instrs):
        if steps >= 10000:
            error = 'error: too many steps'
            break
        op, a, n = instrs[pc]
        steps += 1
        nxt = pc + 1
        if op == 'MOV':
            regs[int(a[0][1])] = val(a[1])
        elif op == 'ADD':
            regs[int(a[0][1])] += val(a[1])
        elif op == 'SUB':
            regs[int(a[0][1])] -= val(a[1])
        elif op == 'MUL':
            regs[int(a[0][1])] *= val(a[1])
        elif op in ('DIV', 'MOD'):
            d = val(a[1])
            if d == 0:
                error = 'error line %d: division by zero' % n
                break
            r = int(a[0][1])
            q = tdiv(regs[r], d)
            regs[r] = q if op == 'DIV' else regs[r] - q * d
        elif op == 'JMP':
            nxt = labels[a[0]]
        elif op == 'JZ':
            if val(a[0]) == 0:
                nxt = labels[a[1]]
        elif op == 'JNZ':
            if val(a[0]) != 0:
                nxt = labels[a[1]]
        elif op == 'JLT':
            if val(a[0]) < val(a[1]):
                nxt = labels[a[2]]
        elif op == 'CALL':
            calls.append(pc + 1)
            nxt = labels[a[0]]
        elif op == 'RET':
            if not calls:
                error = 'error line %d: stack empty' % n
                break
            nxt = calls.pop()
        elif op == 'PUSH':
            data.append(val(a[0]))
        elif op == 'POP':
            if not data:
                error = 'error line %d: stack empty' % n
                break
            regs[int(a[0][1])] = data.pop()
        elif op == 'READ':
            if ip >= len(inp):
                error = 'error line %d: no input' % n
                break
            regs[int(a[0][1])] = inp[ip]
            ip += 1
        elif op == 'PRINT':
            out.append(str(val(a[0])))
        elif op == 'HALT':
            break
        pc = nxt
    return out, error, steps, regs
