"""For py_share_kept and py_share_clash: a function that keeps the list it is given."""
kept = []


def keep(xs):
    kept.append(xs)
    return len(kept)
