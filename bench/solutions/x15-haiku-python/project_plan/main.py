import sys

from graph import Problem, check, parse, unlimited
from schedule import schedule


def main():
    text = sys.stdin.read()
    try:
        workers, tasks = parse(text)
        order = check(tasks)
    except Problem as e:
        print(str(e))
        return

    out = ["unlimited"]
    length, chain = unlimited(tasks, order)
    for t in tasks:
        out.append("  %s %d-%d" % (t.name, t.start, t.end))
    out.append("length %d" % length)
    out.append("critical " + " > ".join(chain))

    k_length, starts, ends = schedule(tasks, order, workers)
    out.append("workers %d" % workers)
    for t in tasks:
        out.append("  %s %d-%d" % (t.name, starts[t.name], ends[t.name]))
    out.append("length %d" % k_length)

    sys.stdout.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
