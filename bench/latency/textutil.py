"""Text helpers for the latency fixture: plain agent-style string code."""


def words(line: str) -> list[str]:
    out = []
    cur = ""
    for ch in line:
        if ch.isalnum():
            cur += ch.lower()
        elif cur:
            out.append(cur)
            cur = ""
    if cur:
        out.append(cur)
    return out


def word_counts(lines: list[str]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for line in lines:
        for w in words(line):
            counts[w] = counts.get(w, 0) + 1
    return counts


def top_words(counts: dict[str, int], n: int) -> list[tuple[str, int]]:
    items = sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))
    return items[:n]


def is_palindrome(s: str) -> bool:
    t = "".join(ch.lower() for ch in s if ch.isalnum())
    return t == t[::-1]


def caesar(s: str, k: int) -> str:
    out = []
    for ch in s:
        if "a" <= ch <= "z":
            out.append(chr((ord(ch) - 97 + k) % 26 + 97))
        elif "A" <= ch <= "Z":
            out.append(chr((ord(ch) - 65 + k) % 26 + 65))
        else:
            out.append(ch)
    return "".join(out)


def wrap(text: str, width: int) -> list[str]:
    lines = []
    cur = ""
    for w in text.split():
        if not cur:
            cur = w
        elif len(cur) + 1 + len(w) <= width:
            cur = cur + " " + w
        else:
            lines.append(cur)
            cur = w
    if cur:
        lines.append(cur)
    return lines


def run_length(s: str) -> str:
    if not s:
        return ""
    out = []
    prev = s[0]
    n = 1
    for ch in s[1:]:
        if ch == prev:
            n += 1
        else:
            out.append(prev + str(n))
            prev = ch
            n = 1
    out.append(prev + str(n))
    return "".join(out)
