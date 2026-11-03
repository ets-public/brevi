# Lists and sets searched with `in` (kept with a hash index in typed builds): every use must match CPython.
def count_new(xs, seen):
    n = 0
    for x in xs:
        if x not in seen:
            n += 1
    return n


def path_search(adj, start, goal):
    path = [start]
    best = []

    def go(node):
        if node == goal:
            best.append(len(path))
            return
        for nxt in adj[node]:
            if nxt not in path:
                path.append(nxt)
                go(nxt)
                path.pop()

    go(start)
    return min(best)


def main():
    seen = set()
    for i in range(100):
        seen.add(i * 3 % 50)
    print(len(seen), 7 in seen, 49 in seen, 50 in seen)
    seen.discard(7)
    seen.discard(1000)
    print(len(seen), 7 in seen, sorted(seen)[:5], sum(seen), min(seen), max(seen))
    print(count_new(list(range(60)), seen))
    # A list with repeats, searched after items are taken out.
    dup = []
    for i in range(40):
        dup.append(i % 20)
    dup.remove(5)
    print(5 in dup, len(dup))
    dup.remove(5)
    print(5 in dup, len(dup), dup.pop(), 19 in dup, dup.pop(), 19 in dup)
    words = {"a", "b"}
    words.update(["c", "a", "d"])
    for w in ["x", "y", "a", "b"]:
        words.add(w)
    print(sorted(words), "x" in words, "z" in words, len(words))
    tags = []
    tags += ["p", "q"] * 10
    print("p" in tags, "r" in tags, len(tags))
    if seen:
        print("nonempty")
    adj = {i: [j for j in range(8) if j != i and (i * j) % 3 != 1] for i in range(8)}
    print(path_search(adj, 0, 7))


main()
