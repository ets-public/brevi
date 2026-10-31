"""Parse and check the task list, and work out the unlimited plan."""

import re

WORKERS_RE = re.compile(r"\s*workers\s+(\d+)\s*")
TASK_RE = re.compile(
    r"\s*([A-Za-z0-9_]+)\s+(\d+)(?:\s+after\s+([A-Za-z0-9_]+(?:\s*,\s*[A-Za-z0-9_]+)*))?\s*"
)


class Problem(Exception):
    pass


class Task:
    def __init__(self, name, days, deps, index):
        self.name = name
        self.days = days
        self.deps = deps          # names, in the order they are listed
        self.index = index        # position in the input
        self.succ = []            # names of tasks that depend on this one
        self.start = 0
        self.end = 0


def parse(text):
    """Return (workers, tasks). Raises Problem with the first line problem found."""
    lines = text.split("\n")
    workers = None
    tasks = []
    seen = {}
    for n, raw in enumerate(lines, 1):
        line = raw.strip()
        if not line:
            continue
        if workers is None:
            m = WORKERS_RE.fullmatch(line)
            if not m or int(m.group(1)) < 1:
                raise Problem("line %d: bad line" % n)
            workers = int(m.group(1))
            continue
        m = TASK_RE.fullmatch(line)
        if not m:
            raise Problem("line %d: bad line" % n)
        name = m.group(1)
        days = int(m.group(2))
        if days < 1:
            raise Problem("line %d: bad line" % n)
        deps = []
        if m.group(3):
            for d in re.split(r"\s*,\s*", m.group(3)):
                if d not in deps:
                    deps.append(d)
        if name in seen:
            raise Problem("line %d: duplicate %s" % (n, name))
        seen[name] = True
        tasks.append(Task(name, days, deps, len(tasks)))
    if workers is None:
        raise Problem("line 1: bad line")
    return workers, tasks


def check(tasks):
    """Raise Problem for a missing dependency or a cycle."""
    names = {t.name: t for t in tasks}
    for t in tasks:
        for d in t.deps:
            if d not in names:
                raise Problem("unknown %s" % d)
    for t in tasks:
        for d in t.deps:
            names[d].succ.append(t.name)
    order = topo_order(tasks)
    if len(order) != len(tasks):
        raise Problem("cycle")
    return order


def topo_order(tasks):
    """Kahn's algorithm; returns task names in a dependency-respecting order."""
    names = {t.name: t for t in tasks}
    indeg = {t.name: len(t.deps) for t in tasks}
    ready = [t.name for t in tasks if indeg[t.name] == 0]
    order = []
    while ready:
        n = ready.pop(0)
        order.append(n)
        for s in names[n].succ:
            indeg[s] -= 1
            if indeg[s] == 0:
                ready.append(s)
    return order


def unlimited(tasks, order):
    """Earliest-start schedule and the critical chain. Returns (length, chain)."""
    names = {t.name: t for t in tasks}
    for n in order:
        t = names[n]
        t.start = max((names[d].end for d in t.deps), default=0)
        t.end = t.start + t.days
    length = max((t.end for t in tasks), default=0)

    good = {}
    for n in reversed(order):
        t = names[n]
        good[n] = t.end == length or any(
            good[s] and names[s].start == t.end for s in t.succ
        )

    chain = []
    starts = [t for t in tasks if t.start == 0 and good[t.name]]
    if starts:
        cur = starts[0]
        chain.append(cur.name)
        while cur.end != length:
            nxt = None
            for s in sorted(cur.succ, key=lambda x: names[x].index):
                st = names[s]
                if st.start == cur.end and good[s]:
                    nxt = st
                    break
            if nxt is None:
                break
            chain.append(nxt.name)
            cur = nxt
    return length, chain
