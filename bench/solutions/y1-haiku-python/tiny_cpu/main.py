import sys

from parse import parse_program
from machine import run


def main():
    data = sys.stdin.read()
    lines = data.split('\n')
    sep = None
    for i, line in enumerate(lines):
        if line.strip() == '---':
            sep = i
            break
    if sep is None:
        prog_lines = lines
        input_text = ''
    else:
        prog_lines = lines[:sep]
        input_text = '\n'.join(lines[sep + 1:])

    instrs, labels, errors = parse_program(prog_lines)
    if errors:
        for lineno, msg in errors:
            print('line %d: %s' % (lineno, msg))
        return

    inputs = [int(t) for t in input_text.split()]
    out, err, steps, regs = run(instrs, labels, inputs)
    for line in out:
        print(line)
    if err:
        print(err)
    print('steps %d' % steps)
    parts = ['r%d=%d' % (i, v) for i, v in enumerate(regs) if v != 0]
    if parts:
        print('regs ' + ' '.join(parts))
    else:
        print('regs none')


if __name__ == '__main__':
    main()
