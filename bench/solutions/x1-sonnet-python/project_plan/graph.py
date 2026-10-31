import re

TASK = re.compile(r'^(\w+)\s+(\d+)(?:\s+after\s+(\w+(?:\s*,\s*\w+)*))?$')
HEAD = re.compile(r'^workers\s+(\d+)$')


def parse(text):
    """Return (k, tasks, error). tasks: list of (name, days, deps)."""
    k = None
    tasks = []
    seen = set()
    for n, line in enumerate(text.split('\n'), 1):
        s = line.strip()
        if not s:
            continue
        if k is None:
            m = HEAD.match(s)
            if not m or int(m.group(1)) < 1:
                return None, None, "line %d: bad line" % n
            k = int(m.group(1))
            continue
        m = TASK.match(s)
        if not m or int(m.group(2)) < 1:
            return None, None, "line %d: bad line" % n
        name = m.group(1)
        if name in seen:
            return None, None, "line %d: duplicate %s" % (n, name)
        seen.add(name)
        deps = re.split(r'\s*,\s*', m.group(3)) if m.group(3) else []
        tasks.append((name, int(m.group(2)), deps))
    if k is None:
        return None, None, "line 1: bad line"
    names = set(t[0] for t in tasks)
    for t in tasks:
        for d in t[2]:
            if d not in names:
                return None, None, "unknown " + d
    if has_cycle(tasks):
        return None, None, "cycle"
    return k, tasks, None


def has_cycle(tasks):
    deps = {t[0]: t[2] for t in tasks}
    state = {}
    for t in tasks:
        stack = [(t[0], iter(deps[t[0]]))]
        if state.get(t[0]):
            continue
        state[t[0]] = 1
        while stack:
            node, it = stack[-1]
            adv = False
            for d in it:
                if state.get(d) == 1:
                    return True
                if not state.get(d):
                    state[d] = 1
                    stack.append((d, iter(deps[d])))
                    adv = True
                    break
            if not adv:
                state[node] = 2
                stack.pop()
    return False


def unlimited(tasks):
    deps = {t[0]: t[2] for t in tasks}
    days = {t[0]: t[1] for t in tasks}
    start = {}

    def st(x):
        if x not in start:
            start[x] = max([st(d) + days[d] for d in deps[x]] or [0])
        return start[x]

    import sys
    sys.setrecursionlimit(100000)
    for t in tasks:
        st(t[0])
    end = {x: start[x] + days[x] for x in start}
    length = max(end.values()) if end else 0
    names = [t[0] for t in tasks]
    good = {}

    def g(x):
        if x not in good:
            r = end[x] == length
            if not r:
                for s in names:
                    if x in deps[s] and start[s] == end[x] and g(s):
                        r = True
                        break
            good[x] = r
        return good[x]

    chain = []
    cur = None
    for x in names:
        if start[x] == 0 and g(x):
            cur = x
            break
    while cur is not None:
        chain.append(cur)
        nxt = None
        for s in names:
            if cur in deps[s] and start[s] == end[cur] and g(s):
                nxt = s
                break
        cur = nxt
    return start, end, length, chain


def tails(tasks):
    days = {t[0]: t[1] for t in tasks}
    users = {t[0]: [] for t in tasks}
    for t in tasks:
        for d in t[2]:
            users[d].append(t[0])
    memo = {}

    def tl(x):
        if x not in memo:
            memo[x] = days[x] + max([tl(u) for u in users[x]] or [0])
        return memo[x]

    return {t[0]: tl(t[0]) for t in tasks}
