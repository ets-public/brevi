"""Writes src/pynames/unicode_names.txt, the character names src/unicode_names.rs reads for Python's \\N{NAME}.
Every name CPython's unicodedata has, plus the aliases in tools/unicode_aliases.txt that CPython accepts, in code
point order. Left out: CJK UNIFIED IDEOGRAPH-* and HANGUL SYLLABLE *, which CPython computes rather than stores;
such a program runs in python3. A name missing here is only ever a refusal, never wrong text.
A line is one character giving how many leading characters the name shares with the line before (chr(33 + n)), the
rest of the name, and, unless the code point is the one after the line before's, a tab and the code point in hex.
usage: python3 tools/unicode_names.py   (run with the newest CPython)"""
import os, unicodedata

HERE = os.path.dirname(os.path.abspath(__file__))
COMPUTED = ('CJK UNIFIED IDEOGRAPH-', 'HANGUL SYLLABLE ')

entries = []
for c in range(0x110000):
    n = unicodedata.name(chr(c), '')
    if n and not n.startswith(COMPUTED):
        entries.append((c, 1, n))
for n in open(os.path.join(HERE, 'unicode_aliases.txt')).read().split('\n'):
    if n:
        # eval, not unicodedata.lookup: \N{} alone turns down named sequences.
        entries.append((ord(eval('"\\N{%s}"' % n)), 0, n))

lines, prev, code = [], '', -1
for c, _, n in sorted(entries):
    k = 0
    while k < min(len(prev), len(n)) and prev[k] == n[k]:
        k += 1
    assert 33 + k < 127
    lines.append(chr(33 + k) + n[k:] + ('' if c == code + 1 else '\t%x' % c))
    prev, code = n, c
open(os.path.join(HERE, '..', 'src', 'pynames', 'unicode_names.txt'), 'w').write('\n'.join(lines) + '\n')
