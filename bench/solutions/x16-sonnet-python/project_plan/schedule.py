from graph import tails


def plan(tasks, workers):
    n = len(tasks)
    idx = {t[0]: i for i, t in enumerate(tasks)}
    tail = tails(tasks)
    start = [None] * n
    end = [None] * n
    finished = set()
    running = []
    t = 0
    started = 0
    while started < n or running:
        for i in [r for r in running if end[r] == t]:
            running.remove(i)
            finished.add(i)
        while len(running) < workers:
            best = None
            for i in range(n):
                if start[i] is None and all(idx[d] in finished for d in tasks[i][2]):
                    if best is None or tail[i] > tail[best]:
                        best = i
            if best is None:
                break
            start[best] = t
            end[best] = t + tasks[best][1]
            running.append(best)
            started += 1
        if running:
            t = min(end[r] for r in running)
        else:
            break
    return start, end, max(end)
