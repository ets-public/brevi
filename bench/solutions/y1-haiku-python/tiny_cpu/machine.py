MAX_STEPS = 10000


def run(instrs, labels, inputs):
    """Run a checked program. Return (output_lines, error_or_None, steps, regs)."""
    regs = [0] * 8
    call_stack = []
    data_stack = []
    out = []
    steps = 0
    pc = 0
    ip = 0
    err = None

    def val(o):
        return regs[o[1]] if o[0] == 'r' else o[1]

    while pc < len(instrs):
        if steps >= MAX_STEPS:
            err = 'error: too many steps'
            break
        lineno, op, ops = instrs[pc]
        steps += 1
        nxt = pc + 1

        if op == 'MOV':
            regs[ops[0][1]] = val(ops[1])
        elif op == 'ADD':
            regs[ops[0][1]] += val(ops[1])
        elif op == 'SUB':
            regs[ops[0][1]] -= val(ops[1])
        elif op == 'MUL':
            regs[ops[0][1]] *= val(ops[1])
        elif op in ('DIV', 'MOD'):
            b = val(ops[1])
            if b == 0:
                err = 'error line %d: division by zero' % lineno
                break
            a = regs[ops[0][1]]
            q = abs(a) // abs(b)
            if (a < 0) != (b < 0):
                q = -q
            if op == 'DIV':
                regs[ops[0][1]] = q
            else:
                regs[ops[0][1]] = a - q * b
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
            call_stack.append(pc + 1)
            nxt = labels[ops[0][1]]
        elif op == 'RET':
            if not call_stack:
                err = 'error line %d: stack empty' % lineno
                break
            nxt = call_stack.pop()
        elif op == 'PUSH':
            data_stack.append(val(ops[0]))
        elif op == 'POP':
            if not data_stack:
                err = 'error line %d: stack empty' % lineno
                break
            regs[ops[0][1]] = data_stack.pop()
        elif op == 'READ':
            if ip >= len(inputs):
                err = 'error line %d: no input' % lineno
                break
            regs[ops[0][1]] = inputs[ip]
            ip += 1
        elif op == 'PRINT':
            out.append(str(val(ops[0])))
        elif op == 'HALT':
            break
        pc = nxt

    return out, err, steps, regs
