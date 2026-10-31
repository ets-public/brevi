import re

WORD = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')
REG = re.compile(r'r[0-7]')
NUM = re.compile(r'-?[0-9]+')

# kinds: r register, v value, l label
SIGS = {
    'MOV': 'rv', 'ADD': 'rv', 'SUB': 'rv', 'MUL': 'rv', 'DIV': 'rv', 'MOD': 'rv',
    'JMP': 'l', 'JZ': 'rl', 'JNZ': 'rl', 'JLT': 'vvl', 'CALL': 'l', 'RET': '',
    'PUSH': 'v', 'POP': 'r', 'READ': 'r', 'PRINT': 'v', 'HALT': '',
}


def operand(kind, s):
    if kind == 'r':
        return ('r', int(s[1])) if REG.fullmatch(s) else None
    if kind == 'v':
        if REG.fullmatch(s):
            return ('r', int(s[1]))
        if NUM.fullmatch(s):
            return ('n', int(s))
        return None
    if kind == 'l':
        return ('l', s) if WORD.fullmatch(s) else None
    return None


def parse_program(lines):
    """lines: list of program lines. Returns (instrs, errors)."""
    instrs = []  # (op, args, lineno)
    labels = {}
    errors = []  # (lineno, text)
    for idx, raw in enumerate(lines):
        n = idx + 1
        line = raw.split(';', 1)[0].strip()
        if not line:
            continue
        if line.endswith(':'):
            name = line[:-1]
            if WORD.fullmatch(name):
                if name in labels:
                    errors.append((n, 'line %d: duplicate label %s' % (n, name)))
                else:
                    labels[name] = len(instrs)
            else:
                errors.append((n, 'line %d: bad instruction' % n))
            continue
        parts = line.split(None, 1)
        op = parts[0]
        if op not in SIGS:
            errors.append((n, 'line %d: bad instruction' % n))
            continue
        sig = SIGS[op]
        ops = parts[1].split(', ') if len(parts) > 1 else []
        if len(ops) != len(sig):
            errors.append((n, 'line %d: bad instruction' % n))
            continue
        args = [operand(k, s) for k, s in zip(sig, ops)]
        if any(a is None for a in args):
            errors.append((n, 'line %d: bad instruction' % n))
            continue
        instrs.append((op, args, n))
    for op, args, n in instrs:
        for a in args:
            if a[0] == 'l' and a[1] not in labels:
                errors.append((n, 'line %d: unknown label %s' % (n, a[1])))
    errors.sort(key=lambda e: e[0])
    return instrs, labels, [e[1] for e in errors]
