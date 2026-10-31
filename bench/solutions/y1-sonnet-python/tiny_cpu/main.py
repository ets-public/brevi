import sys
from parse import parse_program
from machine import run


def main():
    lines = sys.stdin.read().split('\n')
    prog = lines
    rest = []
    for i, l in enumerate(lines):
        if l.strip() == '---':
            prog = lines[:i]
            rest = lines[i + 1:]
            break
    try:
        inputs = [int(t) for t in ' '.join(rest).split()]
    except ValueError:
        inputs = []
    instrs, labels, errors = parse_program(prog)
    if errors:
        print('\n'.join(errors))
        return
    out = []
    steps, regs = run(instrs, labels, inputs, out)
    out.append('steps %d' % steps)
    nz = ''.join(' r%d=%d' % (i, v) for i, v in enumerate(regs) if v != 0)
    out.append('regs' + (nz if nz else ' none'))
    print('\n'.join(out))


main()
