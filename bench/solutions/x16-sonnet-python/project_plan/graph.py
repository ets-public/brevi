import re

W = re.compile(r'^workers\s+([0-9]+)$')
T = re.compile(r'^(\w+)\s+([0-9]+)(?:\s+after\s+(.+))?$')
NAME = re.compile(r'^\w+$')


def parse(text):
    """Returns (error, workers, tasks) where tasks is a list of [name, days, deps]."""
    lines = text.split('\n')
    workers = None
    tasks = []
    seen = set()
    for n, line in enumerate(lines, 1):
        s = line.strip()
        if workers is None:
            if not s:
                continue
            m = W.match(s)
            if not m or int(m.group(1)) < 1:
                return "line %d: bad line" % n, None, None
            workers = int(m.group(1))
            continue
        if not s:
            continue
        m = T.match(s)
        if not m or int(m.group(2)) < 1:
            return "line %d: bad line" % n, None, None
        deps = []
        if m.group(3) is not None:
            deps = [d.strip() for d in m.group(3).split(',')]
            if not all(NAME.match(d) for d in deps):
                return "line %d: bad line" % n, None, None
        if m.group(1) in seen:
            return "line %d: duplicate %s" % (n, m.group(1)), None, None
        seen.add(m.group(1))
        tasks.append([m.group(1), int(m.group(2)), deps])
    if workers is None:
        return "line 1: bad line", None, None
    for name, days, deps in tasks:
        for d in deps:
            if d not in seen:
                return "unknown " + d, None, None
    # cycle check (Kahn)
    idx = {t[0]: i for i, t in enumerate(tasks)}
    indeg = [len(set(t[2])) for t in tasks]
    users = [[] for _ in tasks]
    for i, t in enumerate(tasks):
        for d in set(t[2]):
            users[idx[d]].append(i)
    stack = [i for i, x in enumerate(indeg) if x == 0]
    cnt = 0
    while stack:
        i = stack.pop()
        cnt += 1
        for u in users[i]:
            indeg[u] -= 1
            if indeg[u] == 0:
                stack.append(u)
    if cnt != len(tasks):
        return "cycle", None, None
    return None, workers, tasks


def order(tasks):
    """Topological order of indices."""
    idx = {t[0]: i for i, t in enumerate(tasks)}
    indeg = [len(set(t[2])) for t in tasks]
    users = [[] for _ in tasks]
    for i, t in enumerate(tasks):
        for d in set(t[2]):
            users[idx[d]].append(i)
    stack = [i for i, x in enumerate(indeg) if x == 0]
    res = []
    while stack:
        i = stack.pop()
        res.append(i)
        for u in users[i]:
            indeg[u] -= 1
            if indeg[u] == 0:
                stack.append(u)
    return res, users, idx


def unlimited(tasks):
    """Returns (starts, ends, length, critical chain names)."""
    topo, users, idx = order(tasks)
    n = len(tasks)
    start = [0] * n
    end = [0] * n
    for i in topo:
        s = 0
        for d in tasks[i][2]:
            s = max(s, end[idx[d]])
        start[i] = s
        end[i] = s + tasks[i][1]
    length = max(end)
    crit = [False] * n
    for i in reversed(topo):
        if end[i] == length:
            crit[i] = True
        else:
            for u in users[i]:
                if crit[u] and start[u] == end[i]:
                    crit[i] = True
    chain = []
    cur = None
    for i in range(n):
        if start[i] == 0 and crit[i]:
            cur = i
            break
    while cur is not None:
        chain.append(tasks[cur][0])
        if end[cur] == length:
            break
        nxt = None
        for u in sorted(users[cur]):
            if crit[u] and start[u] == end[cur]:
                nxt = u
                break
        cur = nxt
    return start, end, length, chain


def tails(tasks):
    topo, users, idx = order(tasks)
    tail = [0] * len(tasks)
    for i in reversed(topo):
        best = 0
        for u in users[i]:
            best = max(best, tail[u])
        tail[i] = tasks[i][1] + best
    return tail
