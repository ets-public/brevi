import sys
import graph
import schedule


def main():
    k, tasks, err = graph.parse(sys.stdin.read().split("\n"))
    if err is None and graph.has_cycle(tasks):
        err = "cycle"
    if err is not None:
        print(err)
        return
    out = ["unlimited"]
    s, e, length, chain = graph.unlimited(tasks)
    for t in tasks:
        out.append("  %s %d-%d" % (t[0], s[t[0]], e[t[0]]))
    out.append("length %d" % length)
    out.append("critical " + " > ".join(chain))
    out.append("workers %d" % k)
    s, e, length = schedule.plan(k, tasks)
    for t in tasks:
        out.append("  %s %d-%d" % (t[0], s[t[0]], e[t[0]]))
    out.append("length %d" % length)
    print("\n".join(out))


main()
