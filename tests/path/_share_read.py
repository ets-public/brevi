"""For py_share_read: a function that only reads the dict it is given."""


def top(counts, n):
    items = sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))
    return items[:n]
