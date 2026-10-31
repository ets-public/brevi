import re

NAME = r'[A-Za-z0-9_]+'
TASK_RE = re.compile(r'(%s)\s+([0-9]+)(?:\s+after\s+(%s(?:\s*,\s*%s)*))?' % (NAME, NAME, NAME))
WORKERS_RE = re.compile(r'workers\s+([0-9]+)')


class PlanError(Exception):
    def __init__(self, msg):
        Exception.__init__(self, msg)
        self.msg = msg


class Task:
    def __init__(self, name, days, deps):
        self.name = name
        self.days = days
        self.deps = deps  # unique dependency names, in the order they are named


def parse(text):
    """Return (K, tasks) or raise PlanError with the first problem found while reading."""
    K = None
    tasks = []
    seen = set()
    for lineno, raw in enumerate(text.split('\n'), 1):
        line = raw.strip()
        if not line:
            continue
        if K is None:
            m = WORKERS_RE.fullmatch(line)
            if not m or int(m.group(1)) < 1:
                raise PlanError('line %d: bad line' % lineno)
            K = int(m.group(1))
            continue
        m = TASK_RE.fullmatch(line)
        if not m or int(m.group(2)) < 1:
            raise PlanError('line %d: bad line' % lineno)
        name = m.group(1)
        days = int(m.group(2))
        deps = []
        if m.group(3) is not None:
            for d in re.split(r'\s*,\s*', m.group(3)):
                if d not in deps:
                    deps.append(d)
        if name in seen:
            raise PlanError('line %d: duplicate %s' % (lineno, name))
        seen.add(name)
        tasks.append(Task(name, days, deps))
    if K is None:
        raise PlanError('line 1: bad line')
    return K, tasks


def find_problem(tasks):
    """Return an error message for the first missing dependency or a cycle, else None."""
    names = set(t.name for t in tasks)
    for t in tasks:
        for d in t.deps:
            if d not in names:
                return 'unknown %s' % d
    if topo(tasks) is None:
        return 'cycle'
    return None


def dependents(tasks):
    """Map each task name to the names of tasks that depend on it, in input order."""
    dep = dict((t.name, []) for t in tasks)
    for t in tasks:
        for d in t.deps:
            dep[d].append(t.name)
    return dep


def topo(tasks):
    """Topological order (Kahn), or None when there is a cycle."""
    indeg = dict((t.name, len(t.deps)) for t in tasks)
    dep = dependents(tasks)
    queue = [t.name for t in tasks if indeg[t.name] == 0]
    i = 0
    while i < len(queue):
        n = queue[i]
        i += 1
        for s in dep[n]:
            indeg[s] -= 1
            if indeg[s] == 0:
                queue.append(s)
    if len(queue) != len(tasks):
        return None
    return queue


def unlimited(tasks, order):
    """Earliest-start plan. Returns (start, end, length, critical chain names)."""
    by = dict((t.name, t) for t in tasks)
    dep = dependents(tasks)
    start = {}
    end = {}
    for n in order:
        t = by[n]
        s = max([end[d] for d in t.deps] or [0])
        start[n] = s
        end[n] = s + t.days
    L = max(end.values()) if end else 0

    # good[n]: a chain of gapless dependencies runs from n to a task ending at L
    good = {}
    for n in reversed(order):
        good[n] = end[n] == L or any(good[s] and start[s] == end[n] for s in dep[n])

    chain = []
    for t in tasks:
        if start[t.name] == 0 and good[t.name]:
            chain.append(t.name)
            break
    while chain and end[chain[-1]] != L:
        cur = chain[-1]
        nxt = None
        for s in dep[cur]:
            if good[s] and start[s] == end[cur]:
                nxt = s
                break
        if nxt is None:
            break
        chain.append(nxt)
    return start, end, L, chain
