def add(xs, n):
    xs += [n, n]
    n += 1
    return n

def ext_gen(xs, k):
    xs.extend(i * i for i in range(k))

def ret(xs):
    xs += [7]
    return xs


def add_back(xs):
    xs.append(8)
    return xs

def main():
    a = [1]
    m = 5
    r = add(a, m)
    print(a, m, r)
    g = []
    ext_gen(g, 4)
    print(g)
    q = [1]
    w = ret(q)
    print(q, w)
    e = [2]
    print(add_back(e), e)
main()


# Found by the e2e shadow check (a Haiku poker solution): errors gathered in a local list, then out.extend(errs).
def round_lines(num, lines, out):
    errs = []
    players = {}
    for lineno, line in lines:
        toks = line.split()
        if len(toks) != 2:
            errs.append("line %d: bad" % lineno)
            continue
        players[toks[0]] = (int(toks[1]), "x")
    out.extend(errs)
    out.append("round %d" % num)
    if not players:
        out.append("no players")
        return
    for idx, (name, (key, tag)) in enumerate(sorted(players.items())):
        out.append("%d. %s %s %d" % (idx, name, tag, key))


out = []
for r, group in enumerate([[(1, "a 1"), (2, "b 2"), (3, "zz")], [(4, "q")]], start=1):
    round_lines(r, group, out)
print("\n".join(out))
