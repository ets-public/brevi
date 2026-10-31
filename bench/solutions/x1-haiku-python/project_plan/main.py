import sys

import graph
import schedule


def main():
    try:
        K, tasks = graph.parse(sys.stdin.read())
    except graph.PlanError as e:
        sys.stdout.write(e.msg + '\n')
        return
    problem = graph.find_problem(tasks)
    if problem is not None:
        sys.stdout.write(problem + '\n')
        return

    order = graph.topo(tasks)
    out = ['unlimited']
    start, end, L, chain = graph.unlimited(tasks, order)
    for t in tasks:
        out.append('  %s %d-%d' % (t.name, start[t.name], end[t.name]))
    out.append('length %d' % L)
    out.append('critical ' + ' > '.join(chain))

    s2, e2, L2 = schedule.schedule(tasks, order, K)
    out.append('workers %d' % K)
    for t in tasks:
        out.append('  %s %d-%d' % (t.name, s2[t.name], e2[t.name]))
    out.append('length %d' % L2)
    sys.stdout.write('\n'.join(out) + '\n')


main()
