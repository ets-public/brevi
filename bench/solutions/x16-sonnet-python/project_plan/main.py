import sys
import graph
import schedule


def main():
    err, workers, tasks = graph.parse(sys.stdin.read())
    if err:
        print(err)
        return
    out = ["unlimited"]
    s, e, length, chain = graph.unlimited(tasks)
    for i, t in enumerate(tasks):
        out.append("  %s %d-%d" % (t[0], s[i], e[i]))
    out.append("length %d" % length)
    out.append("critical " + " > ".join(chain))
    out.append("workers %d" % workers)
    s, e, length = schedule.plan(tasks, workers)
    for i, t in enumerate(tasks):
        out.append("  %s %d-%d" % (t[0], s[i], e[i]))
    out.append("length %d" % length)
    print("\n".join(out))


main()
