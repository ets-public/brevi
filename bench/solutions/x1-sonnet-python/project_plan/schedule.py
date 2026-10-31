from graph import tails


def plan(k, tasks):
    tl = tails(tasks)
    start = {}
    end = {}
    todo = [t for t in tasks]
    T = 0
    while todo or any(e > T for e in end.values()):
        running = sum(1 for e in end.values() if e > T)
        while running < k:
            ready = [t for t in todo if all(d in end and end[d] <= T for d in t[2])]
            if not ready:
                break
            best = ready[0]
            for t in ready:
                if tl[t[0]] > tl[best[0]]:
                    best = t
            todo.remove(best)
            start[best[0]] = T
            end[best[0]] = T + best[1]
            running += 1
        future = [e for e in end.values() if e > T]
        if not future:
            break
        T = min(future)
    length = max(end.values()) if end else 0
    return start, end, length
