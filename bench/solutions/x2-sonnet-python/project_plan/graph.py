import re

NAME = r"[A-Za-z0-9_]+"
R_TASK = re.compile(r"(%s) (\d+)(?: after (%s(?:, %s)*))?" % (NAME, NAME, NAME))
R_WORKERS = re.compile(r"workers (\d+)")


class Task:
    def __init__(self, name, days, deps, idx):
        self.name = name
        self.days = days
        self.deps = deps
        self.idx = idx
        self.start = 0
        self.end = 0
        self.users = []


def parse(lines):
    """Return (k, tasks, error)."""
    if not lines:
        return 0, [], "line 1: bad line"
    m = R_WORKERS.fullmatch(lines[0].strip())
    if not m or int(m.group(1)) < 1:
        return 0, [], "line 1: bad line"
    k = int(m.group(1))
    tasks = []
    byname = {}
    for n, line in enumerate(lines[1:], 2):
        s = line.strip()
        if not s:
            continue
        m = R_TASK.fullmatch(s)
        if not m or int(m.group(2)) < 1:
            return k, tasks, "line %d: bad line" % n
        name = m.group(1)
        if name in byname:
            return k, tasks, "line %d: duplicate %s" % (n, name)
        deps = m.group(3).split(", ") if m.group(3) else []
        t = Task(name, int(m.group(2)), deps, len(tasks))
        byname[name] = t
        tasks.append(t)
    for t in tasks:
        for d in t.deps:
            if d not in byname:
                return k, tasks, "unknown " + d
    for t in tasks:
        for d in t.deps:
            byname[d].users.append(t)
    state = {}
    order = []

    def visit(t):
        stack = [(t, 0)]
        while stack:
            cur, i = stack.pop()
            if i == 0:
                if state.get(cur.name) == 2:
                    continue
                if state.get(cur.name) == 1:
                    return False
                state[cur.name] = 1
            if i < len(cur.deps):
                stack.append((cur, i + 1))
                d = byname[cur.deps[i]]
                if state.get(d.name) == 1:
                    return False
                if state.get(d.name) != 2:
                    stack.append((d, 0))
            else:
                state[cur.name] = 2
                order.append(cur)
        return True

    for t in tasks:
        if not visit(t):
            return k, tasks, "cycle"
    for t in order:
        t.start = max([byname[d].end for d in t.deps], default=0)
        t.end = t.start + t.days
    return k, tasks, None


def unlimited(tasks):
    length = max(t.end for t in tasks) if tasks else 0
    reaches = {}
    for t in sorted(tasks, key=lambda t: -t.end):
        reaches[t.name] = t.end == length or any(
            u.start == t.end and reaches[u.name] for u in t.users)
    chain = []
    cands = [t for t in tasks if t.start == 0 and reaches[t.name]]
    cur = cands[0] if cands else None
    while cur is not None:
        chain.append(cur.name)
        nxt = [u for u in cur.users if u.start == cur.end and reaches[u.name]]
        cur = min(nxt, key=lambda u: u.idx) if nxt else None
    return length, chain
