def tdiv(a, b):
    """Integer division rounded toward zero."""
    q = abs(a) // abs(b)
    return -q if (a < 0) != (b < 0) else q


def run(prog, labels, inputs):
    """Run a checked program. Returns (output lines, error line or None, steps, regs)."""
    regs = [0] * 8
    calls = []
    stack = []
    out = []
    pos = 0
    steps = 0
    inp = 0
    err = None

    def val(o):
        return regs[o[1]] if o[0] == 'r' else o[1]

    while pos < len(prog):
        if steps >= 10000:
            err = 'error: too many steps'
            break
        ln, op, ops = prog[pos]
        steps += 1
        nxt = pos + 1
        if op in ('MOV', 'ADD', 'SUB', 'MUL', 'DIV', 'MOD'):
            r = ops[0][1]
            v = val(ops[1])
            if op == 'MOV':
                regs[r] = v
            elif op == 'ADD':
                regs[r] += v
            elif op == 'SUB':
                regs[r] -= v
            elif op == 'MUL':
                regs[r] *= v
            else:
                if v == 0:
                    err = f'error line {ln}: division by zero'
                    break
                q = tdiv(regs[r], v)
                regs[r] = q if op == 'DIV' else regs[r] - q * v
        elif op == 'JMP':
            nxt = labels[ops[0][1]]
        elif op == 'JZ':
            if regs[ops[0][1]] == 0:
                nxt = labels[ops[1][1]]
        elif op == 'JNZ':
            if regs[ops[0][1]] != 0:
                nxt = labels[ops[1][1]]
        elif op == 'JLT':
            if val(ops[0]) < val(ops[1]):
                nxt = labels[ops[2][1]]
        elif op == 'CALL':
            calls.append(pos + 1)
            nxt = labels[ops[0][1]]
        elif op == 'RET':
            if not calls:
                err = f'error line {ln}: stack empty'
                break
            nxt = calls.pop()
        elif op == 'PUSH':
            stack.append(val(ops[0]))
        elif op == 'POP':
            if not stack:
                err = f'error line {ln}: stack empty'
                break
            regs[ops[0][1]] = stack.pop()
        elif op == 'READ':
            if inp >= len(inputs):
                err = f'error line {ln}: no input'
                break
            regs[ops[0][1]] = inputs[inp]
            inp += 1
        elif op == 'PRINT':
            out.append(str(val(ops[0])))
        elif op == 'HALT':
            break
        pos = nxt

    regs_out = [f'r{i}={regs[i]}' for i in range(8) if regs[i] != 0]
    return out, err, steps, regs_out
