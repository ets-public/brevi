import sys
import re

RANKS = {"2": 2, "3": 3, "4": 4, "5": 5, "6": 6, "7": 7, "8": 8, "9": 9,
         "T": 10, "J": 11, "Q": 12, "K": 13, "A": 14}
CARD_RE = re.compile(r"^[23456789TJQKA][SHDC]$")

CATEGORY_NAMES = [
    "high card",
    "pair",
    "two pair",
    "three of a kind",
    "straight",
    "flush",
    "full house",
    "four of a kind",
    "straight flush",
]


def evaluate(cards):
    ranks = [RANKS[c[0]] for c in cards]
    suits = [c[1] for c in cards]
    flush = len(set(suits)) == 1

    counts = {}
    for r in ranks:
        counts[r] = counts.get(r, 0) + 1
    ordered = sorted(counts, key=lambda r: (counts[r], r), reverse=True)
    freq = sorted(counts.values(), reverse=True)

    straight_high = None
    if len(counts) == 5:
        if max(ranks) - min(ranks) == 4:
            straight_high = max(ranks)
        elif sorted(ranks) == [2, 3, 4, 5, 14]:
            straight_high = 5

    if straight_high is not None and flush:
        return (8, [straight_high])
    if freq[0] == 4:
        return (7, ordered)
    if freq[0] == 3 and freq[1] == 2:
        return (6, ordered)
    if flush:
        return (5, sorted(ranks, reverse=True))
    if straight_high is not None:
        return (4, [straight_high])
    if freq[0] == 3:
        return (3, ordered)
    if freq[0] == 2 and freq[1] == 2:
        return (2, ordered)
    if freq[0] == 2:
        return (1, ordered)
    return (0, sorted(ranks, reverse=True))


def parse_line(line):
    parts = line.split()
    if len(parts) != 6:
        return None
    name = parts[0]
    cards = parts[1:]
    for c in cards:
        if not CARD_RE.match(c):
            return None
    if len(set(cards)) != 5:
        return None
    return name, cards


def main():
    lines = sys.stdin.read().split("\n")
    if lines and lines[-1] == "":
        lines.pop()

    out = []
    round_no = 0

    players = []  # (key, name, category_index)
    names = set()
    dealt = set()
    lines_in_round = 0

    def finish():
        nonlocal round_no, players, names, dealt, lines_in_round
        round_no += 1
        out.append("round %d" % round_no)
        if not players:
            out.append("no players")
        else:
            ranked = sorted(players, key=lambda p: p[1])
            ranked = sorted(ranked, key=lambda p: p[0], reverse=True)
            for key, name, cat in ranked:
                position = 1 + sum(1 for q in players if q[0] > key)
                out.append("%d. %s %s" % (position, name, CATEGORY_NAMES[cat]))
        players = []
        names = set()
        dealt = set()
        lines_in_round = 0

    for idx, raw in enumerate(lines):
        lineno = idx + 1
        line = raw.rstrip("\r")
        if line.strip() == "":
            if lines_in_round > 0:
                finish()
            continue

        lines_in_round += 1
        parsed = parse_line(line)
        if parsed is None:
            out.append("line %d: bad hand" % lineno)
            continue
        name, cards = parsed

        if name in names:
            out.append("line %d: duplicate player" % lineno)
            continue

        clash = None
        for c in cards:
            if c in dealt:
                clash = c
                break
        if clash is not None:
            out.append("line %d: card %s already dealt" % (lineno, clash))
            continue

        cat, tiebreak = evaluate(cards)
        players.append(((cat, tiebreak), name, cat))
        names.add(name)
        dealt.update(cards)

    if lines_in_round > 0:
        finish()

    out.append("%d rounds" % round_no)
    print("\n".join(out))


if __name__ == "__main__":
    main()
