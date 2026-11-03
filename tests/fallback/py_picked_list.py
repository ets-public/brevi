# A list picked by a condition and changed through the new name, a re group that did not match,
# and setdefault(k, set()).add(x) on a plain dict.
import re


class O:
    def __init__(self, q):
        self.q = q


def main():
    bids = []
    asks = [O(1), O(2)]
    book = asks if len(bids) == 0 else bids
    book.remove(book[0])
    print(len(asks), [o.q for o in asks])
    m = re.match(r"(\w+)\s+(\d+)(?:\s+after\s+(.*))?", "design 3")
    print(m.group(3) is None, m.group(1))
    g = {}
    for w in ["ab", "ba", "c"]:
        g.setdefault("".join(sorted(w)), set()).add(w)
    print(sorted(len(s) for s in g.values()))


main()
