import sys

from graph import (parse_workers, parse_task, first_unknown, topo_order,
                   unlimited, critical, tails)
from schedule import make_schedule


def main():
    lines = sys.stdin.read().split('\n')
    workers = None
    tasks = []
    index = {}

    for n, raw in enumerate(lines, 1):
        line = raw.strip()
        if not line:
            continue
        if workers is None:
            workers = parse_workers(line)
            if workers is None:
                print('line %d: bad line' % n)
                return
            continue
        t = parse_task(line)
        if t is None:
            print('line %d: bad line' % n)
            return
        if t.name in index:
            print('line %d: duplicate %s' % (n, t.name))
            return
        index[t.name] = len(tasks)
        tasks.append(t)

    if workers is None:
        return

    missing = first_unknown(tasks, index)
    if missing is not None:
        print('unknown %s' % missing)
        return

    order = topo_order(tasks, index)
    if order is None:
        print('cycle')
        return

    out = []
    start, end = unlimited(tasks, index, order)
    length = max(end) if tasks else 0
    out.append('unlimited')
    for i, t in enumerate(tasks):
        out.append('  %s %d-%d' % (t.name, start[i], end[i]))
    out.append('length %d' % length)
    chain = critical(tasks, index, order, start, end)
    out.append('critical ' + ' > '.join(tasks[i].name for i in chain))

    tail = tails(tasks, index, order)
    s2, e2 = make_schedule(tasks, index, workers, tail)
    out.append('workers %d' % workers)
    for i, t in enumerate(tasks):
        out.append('  %s %d-%d' % (t.name, s2[i], e2[i]))
    out.append('length %d' % (max(e2) if tasks else 0))

    sys.stdout.write('\n'.join(out) + '\n')


if __name__ == '__main__':
    main()
