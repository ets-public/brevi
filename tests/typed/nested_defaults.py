graph = {}


def add_edge(a, b, f):
    graph.setdefault(a, {})[b] = f
    graph.setdefault(b, {})[a] = 1 / f


def table(rows):
    widths = [max(len(r[j]) for r in rows) for j in range(len(rows[0]))]

    def render(cells, header=False):
        parts = [c.ljust(w) if header else c.rjust(w) for c, w in zip(cells, widths)]
        return "| " + " | ".join(parts) + " |"

    out = [render(rows[0], True)]
    for r in rows[1:]:
        out.append(render(r))
    return "\n".join(out)


def main():
    add_edge("m", "cm", 100.0)
    add_edge("km", "m", 1000.0)
    for u in sorted(graph):
        print(u, sorted(graph[u].items()))
    print(table([["name", "qty"], ["apple", "3"], ["kiwi", "12"]]))
    x = 5
    print(~x, ~-3, ~0)
    (only,) = [42]
    print(only)


main()
