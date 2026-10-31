import re

TASK = re.compile(r"^(\w+) (\d+)(?: after (\w+(?:, \w+)*))?$")


def parse(lines):
    """Return (k, tasks, error). tasks: list of (name, days, deps)."""
    k = None
    tasks = []
    seen = set()
    for n, line in enumerate(lines, 1):
        if k is None:
            if not line.strip():
                continue
            m = re.match(r"^workers (\d+)$", line)
            if not m or int(m.group(1)) < 1:
                return None, None, "line %d: bad line" % n
            k = int(m.group(1))
            continue
        if not line.strip():
            continue
        m = TASK.match(line)
        if not m or int(m.group(2)) < 1:
            return None, None, "line %d: bad line" % n
        name = m.group(1)
        if name in seen:
            return None, None, "line %d: duplicate %s" % (n, name)
        seen.add(name)
        deps = m.group(3).split(", ") if m.group(3) else []
        tasks.append((name, int(m.group(2)), deps))
    if k is None:
        return None, None, "line 1: bad line"
    for name, days, deps in tasks:
        for d in deps:
            if d not in seen:
                return None, None, "unknown " + d
    return k, tasks, None


def has_cycle(tasks):
    deps = {t[0]: t[2] for t in tasks}
    state = {}
    for root in deps:
        if root in state:
            continue
        stack = [(root, iter(deps[root]))]
        state[root] = 1
        while stack:
            node, it = stack[-1]
            for d in it:
                if state.get(d) == 1:
                    return True
                if d not in state:
                    state[d] = 1
                    stack.append((d, iter(deps[d])))
                    break
            else:
                state[node] = 2
                stack.pop()
    return False


def unlimited(tasks):
    deps = {t[0]: t[2] for t in tasks}
    days = {t[0]: t[1] for t in tasks}
    start = {}
    end = {}

    def calc(root):
        stack = [root]
        while stack:
            x = stack[-1]
            if x in end:
                stack.pop()
                continue
            pend = [d for d in deps[x] if d not in end]
            if pend:
                stack.extend(pend)
                continue
            s = max([end[d] for d in deps[x]] or [0])
            start[x] = s
            end[x] = s + days[x]
            stack.pop()

    for t in tasks:
        calc(t[0])
    length = max(end.values())
    order = [t[0] for t in tasks]
    dependents = {n: [] for n in order}
    for n in order:
        for d in deps[n]:
            dependents[d].append(n)
    crit = {}
    for n in sorted(order, key=lambda x: -start[x]):
        c = end[n] == length
        if not c:
            c = any(start[d] == end[n] and crit[d] for d in dependents[n])
        crit[n] = c
    chain = []
    cur = next(n for n in order if start[n] == 0 and crit[n])
    chain.append(cur)
    while end[cur] != length or any(start[d] == end[cur] and crit[d] for d in dependents[cur]):
        if end[cur] == length:
            break
        cur = next(d for d in order if d in dependents[cur] and start[d] == end[cur] and crit[d])
        chain.append(cur)
    return start, end, length, chain
