MAX_STEPS = 10000


def tdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a < 0) == (b < 0) else -q


def run(instrs, labels, inputs, out):
    """Runs the program; appends output lines to out. Returns (steps, regs)."""
    regs = [0] * 8
    data = []
    calls = []
    pc = 0
    steps = 0
    inp = 0

    def val(a):
        return regs[a[1]] if a[0] == 'r' else a[1]

    while pc < len(instrs):
        if steps >= MAX_STEPS:
            out.append('error: too many steps')
            break
        op, args, n = instrs[pc]
        steps += 1
        pc += 1
        if op == 'MOV':
            regs[args[0][1]] = val(args[1])
        elif op == 'ADD':
            regs[args[0][1]] += val(args[1])
        elif op == 'SUB':
            regs[args[0][1]] -= val(args[1])
        elif op == 'MUL':
            regs[args[0][1]] *= val(args[1])
        elif op in ('DIV', 'MOD'):
            b = val(args[1])
            if b == 0:
                out.append('error line %d: division by zero' % n)
                break
            a = regs[args[0][1]]
            q = tdiv(a, b)
            regs[args[0][1]] = q if op == 'DIV' else a - q * b
        elif op == 'JMP':
            pc = labels[args[0][1]]
        elif op == 'JZ':
            if val(args[0]) == 0:
                pc = labels[args[1][1]]
        elif op == 'JNZ':
            if val(args[0]) != 0:
                pc = labels[args[1][1]]
        elif op == 'JLT':
            if val(args[0]) < val(args[1]):
                pc = labels[args[2][1]]
        elif op == 'CALL':
            calls.append(pc)
            pc = labels[args[0][1]]
        elif op == 'RET':
            if not calls:
                out.append('error line %d: stack empty' % n)
                break
            pc = calls.pop()
        elif op == 'PUSH':
            data.append(val(args[0]))
        elif op == 'POP':
            if not data:
                out.append('error line %d: stack empty' % n)
                break
            regs[args[0][1]] = data.pop()
        elif op == 'READ':
            if inp >= len(inputs):
                out.append('error line %d: no input' % n)
                break
            regs[args[0][1]] = inputs[inp]
            inp += 1
        elif op == 'PRINT':
            out.append(str(val(args[0])))
        elif op == 'HALT':
            break
    return steps, regs
