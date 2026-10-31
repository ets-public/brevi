"""Parse and check the tasks; work out the unlimited plan and the critical chain."""
import re
from collections import deque

NAME_RE = re.compile(r'[A-Za-z0-9_]+\Z')
WORKERS_RE = re.compile(r'workers\s+([0-9]+)\Z')
TASK_RE = re.compile(r'([A-Za-z0-9_]+)\s+([0-9]+)(?:\s+after\s+(.*?))?\s*\Z')


class Task:
    def __init__(self, name, days, deps):
        self.name = name
        self.days = days
        self.deps = deps


def parse_workers(line):
    """Return K for a valid 'workers K' line, else None."""
    m = WORKERS_RE.match(line)
    if not m:
        return None
    k = int(m.group(1))
    return k if k >= 1 else None


def parse_task(line):
    """Return a Task for a valid task line, else None."""
    m = TASK_RE.match(line)
    if not m:
        return None
    days = int(m.group(2))
    if days < 1:
        return None
    deps = []
    if m.group(3) is not None:
        for d in m.group(3).split(','):
            d = d.strip()
            if not NAME_RE.match(d):
                return None
            deps.append(d)
    return Task(m.group(1), days, deps)


def first_unknown(tasks, index):
    for t in tasks:
        for d in t.deps:
            if d not in index:
                return d
    return None


def topo_order(tasks, index):
    """Return task indices in dependency order, or None if there is a cycle."""
    n = len(tasks)
    indeg = [0] * n
    succ = [[] for _ in range(n)]
    for i, t in enumerate(tasks):
        for d in t.deps:
            j = index[d]
            succ[j].append(i)
            indeg[i] += 1
    queue = deque(i for i in range(n) if indeg[i] == 0)
    order = []
    while queue:
        i = queue.popleft()
        order.append(i)
        for s in succ[i]:
            indeg[s] -= 1
            if indeg[s] == 0:
                queue.append(s)
    if len(order) != n:
        return None
    return order


def unlimited(tasks, index, order):
    """Earliest start and end of every task with no worker limit."""
    n = len(tasks)
    start = [0] * n
    end = [0] * n
    for i in order:
        start[i] = max((end[index[d]] for d in tasks[i].deps), default=0)
        end[i] = start[i] + tasks[i].days
    return start, end


def critical(tasks, index, order, start, end):
    """Longest chain of tasks with no gaps, as a list of task indices."""
    n = len(tasks)
    if n == 0:
        return []
    length = max(end)
    # tight successors: s depends on j and starts exactly when j ends (input order)
    tight = [[] for _ in range(n)]
    for i, t in enumerate(tasks):
        for d in t.deps:
            j = index[d]
            if end[j] == start[i]:
                tight[j].append(i)
    reach = [False] * n
    for i in reversed(order):
        reach[i] = end[i] == length or any(reach[s] for s in tight[i])
    cur = None
    for i in range(n):
        if start[i] == 0 and reach[i]:
            cur = i
            break
    chain = [cur]
    while end[cur] != length:
        nxt = None
        for s in tight[cur]:
            if reach[s]:
                nxt = s
                break
        cur = nxt
        chain.append(cur)
    return chain


def tails(tasks, index, order):
    """Longest path from each task to the end, counting its own DAYS."""
    n = len(tasks)
    dependents = [[] for _ in range(n)]
    for i, t in enumerate(tasks):
        for d in t.deps:
            dependents[index[d]].append(i)
    tail = [0] * n
    for i in reversed(order):
        tail[i] = tasks[i].days + max((tail[s] for s in dependents[i]), default=0)
    return tail
