import sys

from parse import check
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
        rest = []
    else:
        prog_lines = lines[:sep]
        rest = lines[sep + 1:]
    inputs = [int(t) for t in ' '.join(rest).split()]

    prog, labels, errors = check(prog_lines)
    if errors:
        for n, msg in errors:
            print(f'line {n}: {msg}')
        return

    out, err, steps, regs = run(prog, labels, inputs)
    for line in out:
        print(line)
    if err is not None:
        print(err)
    print(f'steps {steps}')
    if regs:
        print('regs ' + ' '.join(regs))
    else:
        print('regs none')


if __name__ == '__main__':
    main()
