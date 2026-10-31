import re

NAME_RE = re.compile(r'[A-Za-z0-9_]+')
DAYS_RE = re.compile(r'[0-9]+')


class Task:
    def __init__(self, name, days, deps):
        self.name = name
        self.days = days
        self.deps = deps


def parse_task(t):
    if len(t) < 2 or not NAME_RE.fullmatch(t[0]) or not DAYS_RE.fullmatch(t[1]):
        return None
    days = int(t[1])
    if days < 1:
        return None
    if len(t) == 2:
        return t[0], days, []
    if len(t) < 4 or t[2] != 'after':
        return None
    deps = []
    for p in ' '.join(t[3:]).split(','):
        p = p.strip()
        if not NAME_RE.fullmatch(p):
            return None
        deps.append(p)
    return t[0], days, deps


def parse(text):
    k = None
    tasks = []
    seen = set()
    for n, raw in enumerate(text.splitlines(), 1):
        t = raw.split()
        if not t:
            continue
        if k is None:
            if len(t) == 2 and t[0] == 'workers' and DAYS_RE.fullmatch(t[1]) and int(t[1]) >= 1:
                k = int(t[1])
                continue
            return None, None, "line %d: bad line" % n
        p = parse_task(t)
        if p is None:
            return None, None, "line %d: bad line" % n
        name, days, deps = p
        if name in seen:
            return None, None, "line %d: duplicate %s" % (n, name)
        seen.add(name)
        tasks.append(Task(name, days, deps))
    if k is None:
        return None, None, "line 1: bad line"
    return k, tasks, None


def dependents_of(tasks):
    dep = {t.name: [] for t in tasks}
    for t in tasks:
        for d in dict.fromkeys(t.deps):
            dep[d].append(t.name)
    return dep


def check(tasks):
    names = {t.name for t in tasks}
    for t in tasks:
        for d in t.deps:
            if d not in names:
                return "unknown %s" % d, None
    dependents = dependents_of(tasks)
    indeg = {t.name: len(set(t.deps)) for t in tasks}
    ready = [t.name for t in tasks if indeg[t.name] == 0]
    order = []
    while ready:
        n = ready.pop(0)
        order.append(n)
        for s in dependents[n]:
            indeg[s] -= 1
            if indeg[s] == 0:
                ready.append(s)
    if len(order) < len(tasks):
        return "cycle", None
    return None, order


def unlimited(tasks, order, dependents):
    by = {t.name: t for t in tasks}
    start = {}
    end = {}
    for n in order:
        t = by[n]
        s = max((end[d] for d in t.deps), default=0)
        start[n] = s
        end[n] = s + t.days
    L = max(end.values()) if end else 0
    good = {}
    for n in reversed(order):
        good[n] = end[n] == L or any(good[s] and start[s] == end[n] for s in dependents[n])
    cur = None
    for t in tasks:
        if start[t.name] == 0 and good[t.name]:
            cur = t.name
            break
    if cur is None:
        return start, end, L, []
    chain = [cur]
    while end[cur] != L:
        nxt = None
        for s in dependents[cur]:
            if start[s] == end[cur] and good[s]:
                nxt = s
                break
        chain.append(nxt)
        cur = nxt
    return start, end, L, chain
