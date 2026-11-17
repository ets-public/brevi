from collections import namedtuple

# namedtuple checks names with str.isidentifier() (CPython's XID tables), not "alphabetic or numeric"
def check(name):
    try:
        namedtuple('T', [name])
        return 'ok'
    except ValueError as e:
        return str(e)

for name in ['abc', '_x', 'a·b', '℘x', 'x℘', 'a²', '²a', 'ͺ', 'aͺ',
             'café', 'x٠', '٠x', 'ᢅ', 'aᢅ', 'xⸯ', 'ⸯ', 'a゛',
             '℮', 'a᧚', '\U0001d7ce', '\U000e0100', 'x\U000e0100', 'def', 'match', 'type']:
    print(ascii(name), check(name))

# the type name gets the same check
for tn in ['P²', '℘P', 'class']:
    try:
        print(namedtuple(tn, 'x').__name__ == tn)
    except ValueError as e:
        print(e)

# a str of field names splits on CPython's whitespace, which includes \x1c-\x1f
print(namedtuple('S', 'a\x1fb\x1cc,d　e\x85f')._fields)

# rename=True replaces exactly the names isidentifier() rejects; swept over every code point as a first
# and as a later character, 2,000 names per class. Characters NFKC normalization may change (Brevi stops on
# them, see tests/check.sh) are left out: CPython compiles the normalized names, so 'ﬁ' and 'fi' collide.
import unicodedata
seconds = set(range(0x1161, 0x1176)) | set(range(0x11a8, 0x11c3))
for cp in range(0x80, 0x110000):
    d = unicodedata.decomposition(chr(cp))
    if d and not d.startswith('<'):
        seconds.update(int(h, 16) for h in d.split()[1:])
def may_change(c):
    first = unicodedata.normalize('NFKD', c)[0]
    return (unicodedata.normalize('NFKC', c) != c or unicodedata.combining(c) != 0 or ord(c) in seconds
            or unicodedata.combining(first) != 0 or ord(first) in seconds)
starts = conts = ssum = csum = left_out = 0
batch = []
def flush():
    global starts, conts, ssum, csum
    fields = []
    for c in batch:
        fields.append(c)
        fields.append('x' + c)
    got = namedtuple('B', fields, rename=True)._fields
    for i, c in enumerate(batch):
        if got[2 * i] == c:
            starts += 1
            ssum += ord(c)
        if got[2 * i + 1] == 'x' + c:
            conts += 1
            csum += ord(c)
    batch.clear()
for cp in range(0x110000):
    if 0xd800 <= cp < 0xe000:
        continue
    c = chr(cp)
    if cp >= 0x80 and may_change(c):
        left_out += 1
        continue
    batch.append(c)
    if len(batch) == 1000:
        flush()
flush()
print(starts, conts, ssum, csum, left_out)
