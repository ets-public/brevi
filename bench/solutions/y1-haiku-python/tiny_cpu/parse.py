import re

LABEL_RE = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')
REG_RE = re.compile(r'r([0-7])')
NUM_RE = re.compile(r'-?[0-9]+')

# kinds: r = register, v = register or number, L = label
SPEC = {
    'MOV': 'rv',
    'ADD': 'rv',
    'SUB': 'rv',
    'MUL': 'rv',
    'DIV': 'rv',
    'MOD': 'rv',
    'JMP': 'L',
    'JZ': 'rL',
    'JNZ': 'rL',
    'JLT': 'vvL',
    'CALL': 'L',
    'RET': '',
    'PUSH': 'v',
    'POP': 'r',
    'READ': 'r',
    'PRINT': 'v',
    'HALT': '',
}


def parse_operand(text, kind):
    if kind == 'L':
        if LABEL_RE.fullmatch(text):
            return ('L', text)
        return None
    m = REG_RE.fullmatch(text)
    if m:
        return ('r', int(m.group(1)))
    if kind == 'v' and NUM_RE.fullmatch(text):
        return ('n', int(text))
    return None


def parse_program(lines):
    """Return (instrs, labels, errors).

    instrs: list of (lineno, op, operands); labels: name -> instruction index;
    errors: sorted list of (lineno, message)."""
    instrs = []
    labels = {}
    errors = []
    for lineno, raw in enumerate(lines, 1):
        code = raw.split(';', 1)[0].strip()
        if not code:
            continue
        if code.endswith(':'):
            name = code[:-1]
            if LABEL_RE.fullmatch(name):
                if name in labels:
                    errors.append((lineno, 'duplicate label %s' % name))
                else:
                    labels[name] = len(instrs)
            else:
                errors.append((lineno, 'bad instruction'))
            continue
        parts = code.split(None, 1)
        op = parts[0]
        rest = parts[1].strip() if len(parts) > 1 else ''
        if op not in SPEC:
            errors.append((lineno, 'bad instruction'))
            continue
        kinds = SPEC[op]
        if not kinds:
            if rest:
                errors.append((lineno, 'bad instruction'))
            else:
                instrs.append((lineno, op, []))
            continue
        if not rest:
            errors.append((lineno, 'bad instruction'))
            continue
        texts = [x.strip() for x in rest.split(',')]
        if len(texts) != len(kinds):
            errors.append((lineno, 'bad instruction'))
            continue
        ops = []
        ok = True
        for text, kind in zip(texts, kinds):
            o = parse_operand(text, kind)
            if o is None:
                ok = False
                break
            ops.append(o)
        if not ok:
            errors.append((lineno, 'bad instruction'))
            continue
        instrs.append((lineno, op, ops))

    for lineno, op, ops in instrs:
        for o in ops:
            if o[0] == 'L' and o[1] not in labels:
                errors.append((lineno, 'unknown label %s' % o[1]))
    errors.sort(key=lambda e: e[0])
    return instrs, labels, errors
