import re

REG_RE = re.compile(r'r([0-7])')
INT_RE = re.compile(r'-?[0-9]+')
WORD_RE = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')
LABEL_DEF_RE = re.compile(r'([A-Za-z_][A-Za-z0-9_]*):')

# operand kinds: R register, V register or value, L label
SPECS = {
    'MOV': 'RV', 'ADD': 'RV', 'SUB': 'RV', 'MUL': 'RV', 'DIV': 'RV', 'MOD': 'RV',
    'JMP': 'L', 'JZ': 'RL', 'JNZ': 'RL', 'JLT': 'VVL',
    'CALL': 'L', 'RET': '', 'PUSH': 'V', 'POP': 'R', 'READ': 'R',
    'PRINT': 'V', 'HALT': '',
}


def parse_operand(text, kind):
    """Return ('r', i), ('n', value), ('l', name) or None if it does not fit."""
    if kind == 'R':
        m = REG_RE.fullmatch(text)
        return ('r', int(m.group(1))) if m else None
    if kind == 'V':
        m = REG_RE.fullmatch(text)
        if m:
            return ('r', int(m.group(1)))
        if INT_RE.fullmatch(text):
            return ('n', int(text))
        return None
    if kind == 'L':
        return ('l', text) if WORD_RE.fullmatch(text) else None
    return None


def check(lines):
    """Check the program lines.

    Returns (prog, labels, errors): prog is a list of (line, op, operands),
    labels maps a name to the index of the instruction after it, and errors
    is a list of (line, message) in line order.
    """
    prog = []
    labels = {}
    errors = []
    for n, raw in enumerate(lines, 1):
        text = raw.split(';', 1)[0].strip()
        if not text:
            continue
        if text.endswith(':') or ':' in text:
            m = LABEL_DEF_RE.fullmatch(text)
            if not m:
                errors.append((n, 'bad instruction'))
                continue
            name = m.group(1)
            if name in labels:
                errors.append((n, f'duplicate label {name}'))
            else:
                labels[name] = len(prog)
            continue
        parts = text.split(None, 1)
        op = parts[0]
        rest = parts[1] if len(parts) > 1 else ''
        if op not in SPECS:
            errors.append((n, 'bad instruction'))
            continue
        spec = SPECS[op]
        if rest.strip() == '':
            texts = []
        else:
            texts = [x.strip() for x in rest.split(',')]
        if len(texts) != len(spec) or any(t == '' for t in texts):
            errors.append((n, 'bad instruction'))
            continue
        ops = []
        ok = True
        for t, kind in zip(texts, spec):
            o = parse_operand(t, kind)
            if o is None:
                ok = False
                break
            ops.append(o)
        if not ok:
            errors.append((n, 'bad instruction'))
            continue
        prog.append((n, op, ops))

    for n, op, ops in prog:
        for o in ops:
            if o[0] == 'l' and o[1] not in labels:
                errors.append((n, f'unknown label {o[1]}'))

    errors.sort(key=lambda e: e[0])
    return prog, labels, errors
