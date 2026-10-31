"""Schedule the tasks on K workers."""


def make_schedule(tasks, index, k, tail):
    """Return (start, end) lists for K workers, following the tail rule."""
    n = len(tasks)
    deps = [[index[d] for d in t.deps] for t in tasks]
    start = [0] * n
    end = [0] * n
    started = [False] * n
    done = [False] * n
    running = []
    finished = 0
    t_now = 0
    while finished < n:
        # finish the tasks that end today
        for i in list(running):
            if end[i] == t_now:
                running.remove(i)
                done[i] = True
                finished += 1
        # start ready tasks while workers are free: largest tail first
        ready = [i for i in range(n)
                 if not started[i] and all(done[j] for j in deps[i])]
        ready.sort(key=lambda i: (-tail[i], i))
        for i in ready:
            if len(running) >= k:
                break
            started[i] = True
            start[i] = t_now
            end[i] = t_now + tasks[i].days
            running.append(i)
        if not running:
            break
        t_now = min(end[i] for i in running)
    return start, end
