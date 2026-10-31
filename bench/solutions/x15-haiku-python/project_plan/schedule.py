"""Schedule the tasks on K workers."""


def tails(tasks, order):
    """tail = DAYS plus the largest tail of the tasks that depend on it."""
    names = {t.name: t for t in tasks}
    tail = {}
    for n in reversed(order):
        t = names[n]
        tail[n] = t.days + max((tail[s] for s in t.succ), default=0)
    return tail


def schedule(tasks, order, workers):
    """Return (length, starts, ends) as dicts keyed by task name."""
    names = {t.name: t for t in tasks}
    tail = tails(tasks, order)
    start = {}
    end = {}
    done = set()
    running = {}  # name -> end time
    now = 0
    while len(done) < len(tasks):
        for n in [n for n, e in running.items() if e == now]:
            del running[n]
            done.add(n)
        while len(running) < workers:
            ready = [
                t for t in tasks
                if t.name not in start
                and all(d in done for d in t.deps)
            ]
            if not ready:
                break
            best = max(ready, key=lambda t: (tail[t.name], -t.index))
            start[best.name] = now
            end[best.name] = now + best.days
            running[best.name] = now + best.days
        if len(done) >= len(tasks):
            break
        if not running:
            break
        now = min(running.values())
    length = max(end.values(), default=0)
    return length, start, end
