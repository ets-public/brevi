import sys
from graph import parse, unlimited
from schedule import plan


def main():
    lines = sys.stdin.read().split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    k, tasks, err = parse(lines)
    if err:
        print(err)
        return
    out = ["unlimited"]
    for t in tasks:
        out.append("  %s %d-%d" % (t.name, t.start, t.end))
    length, chain = unlimited(tasks)
    out.append("length %d" % length)
    out.append("critical " + " > ".join(chain))
    starts, ends, L = plan(tasks, k)
    out.append("workers %d" % k)
    for t in tasks:
        out.append("  %s %d-%d" % (t.name, starts[t.name], ends[t.name]))
    out.append("length %d" % L)
    print("\n".join(out))


main()
