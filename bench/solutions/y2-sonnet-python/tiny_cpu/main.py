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
    inputs = [int(x) for x in ' '.join(rest).split()]
    instrs, labels, errors = parse_program(prog)
    if errors:
        for e in errors:
            print(e)
        return
    out, error, steps, regs = run(instrs, labels, inputs)
    for o in out:
        print(o)
    if error:
        print(error)
    print('steps %d' % steps)
    nz = ['r%d=%d' % (i, v) for i, v in enumerate(regs) if v != 0]
    print('regs ' + ' '.join(nz) if nz else 'regs none')

main()
