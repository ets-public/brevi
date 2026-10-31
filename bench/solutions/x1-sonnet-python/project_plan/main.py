import sys
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from graph import parse, unlimited
from schedule import plan


def main():
    k, tasks, err = parse(sys.stdin.read())
    if err:
        print(err)
        return
    out = ["unlimited"]
    s, e, length, chain = unlimited(tasks)
    for t in tasks:
        out.append("  %s %d-%d" % (t[0], s[t[0]], e[t[0]]))
    out.append("length %d" % length)
    out.append("critical " + " > ".join(chain))
    s, e, length = plan(k, tasks)
    out.append("workers %d" % k)
    for t in tasks:
        out.append("  %s %d-%d" % (t[0], s[t[0]], e[t[0]]))
    out.append("length %d" % length)
    print("\n".join(out))


main()
