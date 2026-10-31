import re

REG = re.compile(r'r[0-7]')
NUM = re.compile(r'-?\d+')
WORD = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')

def is_reg(x):
    return REG.fullmatch(x) is not None

def is_val(x):
    return is_reg(x) or NUM.fullmatch(x) is not None

def is_lab(x):
    return WORD.fullmatch(x) is not None

# operand kinds: r register, v value, l label
SIGS = {
    'MOV': 'rv', 'ADD': 'rv', 'SUB': 'rv', 'MUL': 'rv', 'DIV': 'rv', 'MOD': 'rv',
    'JMP': 'l', 'JZ': 'rl', 'JNZ': 'rl', 'JLT': 'vvl', 'CALL': 'l', 'RET': '',
    'PUSH': 'v', 'POP': 'r', 'READ': 'r', 'PRINT': 'v', 'HALT': '',
}
CHECK = {'r': is_reg, 'v': is_val, 'l': is_lab}

def parse_program(lines):
    """lines: program lines. Returns (instrs, labels, errors)."""
    instrs = []
    labels = {}
    errors = []
    for n, raw in enumerate(lines, 1):
        text = raw.split(';', 1)[0].strip()
        if not text:
            continue
        if text.endswith(':'):
            name = text[:-1]
            if not is_lab(name):
                errors.append((n, 'line %d: bad instruction' % n))
            elif name in labels:
                errors.append((n, 'line %d: duplicate label %s' % (n, name)))
            else:
                labels[name] = len(instrs)
            continue
        parts = text.split(None, 1)
        op = parts[0]
        rest = parts[1] if len(parts) > 1 else ''
        if op not in SIGS:
            errors.append((n, 'line %d: bad instruction' % n))
            continue
        sig = SIGS[op]
        args = rest.split(', ') if rest else []
        if len(args) != len(sig) or not all(CHECK[k](a) for k, a in zip(sig, args)):
            errors.append((n, 'line %d: bad instruction' % n))
            continue
        instrs.append((op, args, n))
    for op, args, n in instrs:
        if SIGS[op].endswith('l') and args[-1] not in labels:
            errors.append((n, 'line %d: unknown label %s' % (n, args[-1])))
    errors.sort(key=lambda e: e[0])
    return instrs, labels, [e[1] for e in errors]
