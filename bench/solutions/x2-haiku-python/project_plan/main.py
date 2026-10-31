import sys

import graph
import schedule


def main():
    k, tasks, err = graph.parse(sys.stdin.read())
    if err:
        sys.stdout.write(err + "\n")
        return
    err, order = graph.check(tasks)
    if err:
        sys.stdout.write(err + "\n")
        return
    dependents = graph.dependents_of(tasks)
    start, end, L, chain = graph.unlimited(tasks, order, dependents)
    out = ["unlimited"]
    for t in tasks:
        out.append("  %s %d-%d" % (t.name, start[t.name], end[t.name]))
    out.append("length %d" % L)
    out.append("critical " + " > ".join(chain))
    s2, e2 = schedule.schedule(tasks, k)
    out.append("workers %d" % k)
    for t in tasks:
        out.append("  %s %d-%d" % (t.name, s2[t.name], e2[t.name]))
    out.append("length %d" % max(e2.values(), default=0))
    sys.stdout.write("\n".join(out) + "\n")


if __name__ == '__main__':
    main()
