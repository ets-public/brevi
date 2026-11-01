import sys
import re

HEAD_RE = re.compile(r'(#{1,6}) (.*)')
CLOSING_RE = re.compile(r' +#+$')


def main():
    headings = []  # (line number, level, text)
    warnings = []
    in_fence = False
    fence_line = 0

    for n, raw in enumerate(sys.stdin, 1):
        line = raw.rstrip('\r\n')
        if line.startswith('```'):
            if in_fence:
                in_fence = False
            else:
                in_fence = True
                fence_line = n
            continue
        if in_fence:
            continue
        m = HEAD_RE.fullmatch(line)
        if not m:
            continue
        text = m.group(2).strip()
        if not text:
            continue
        text = CLOSING_RE.sub('', text)
        headings.append((n, len(m.group(1)), text))

    if in_fence:
        warnings.append('line %d: unclosed code block' % fence_line)

    if not headings:
        out = warnings + ['no headings']
        print('\n'.join(out))
        return

    top = min(level for _, level, _ in headings)
    counters = [0] * (len(headings) + 8)
    used = {}
    prev_level = None
    prev_depth = None
    max_depth = 0
    body = []

    for n, level, text in headings:
        if prev_level is None:
            depth = 1 + (level - top)
        elif level - prev_level > 1:
            warnings.append('line %d: level jump' % n)
            depth = prev_depth + 1
        else:
            depth = 1 + (level - top)
        prev_level = level
        prev_depth = depth
        max_depth = max(max_depth, depth)

        counters[depth] += 1
        for k in range(depth + 1, len(counters)):
            counters[k] = 0
        number = '.'.join(str(counters[i]) for i in range(1, depth + 1))

        plain = re.sub(r'[^a-z0-9 -]', '', text.lower()).replace(' ', '-')
        if plain == '':
            plain = 'section'
        count = used.get(plain, 0)
        used[plain] = count + 1
        slug = plain if count == 0 else '%s-%d' % (plain, count)

        body.append('%s%s %s (#%s)' % ('  ' * (depth - 1), number, text, slug))

    out = warnings + body + ['%d headings, max depth %d' % (len(headings), max_depth)]
    print('\n'.join(out))


if __name__ == '__main__':
    main()
