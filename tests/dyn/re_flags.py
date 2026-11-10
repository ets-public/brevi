import re

print(repr(re.I), str(re.A), repr(re.I | re.S | re.X), repr(re.I | re.S | (1 << 20)))
print(repr(~re.I), repr(~(re.I | re.S | re.X | (1 << 20))))
print(repr(re.NOFLAG), re.I == 2, re.I | 4, int(re.M), isinstance(re.I, int), re.RegexFlag.DOTALL is re.S)
print(repr(re.compile('random pattern')), repr(re.compile('p', re.IGNORECASE)))
print(repr(re.compile('p', re.I | re.S | re.X)), repr(re.compile('p', 0x123000 | re.I)))
print(repr(re.compile('(?i)pattern')), repr(re.compile("it's")), repr(re.compile('both \'s\' and "d"')))
r = repr(re.compile('Very %spattern' % ('long ' * 1000), re.I))
print(len(r), r[:30], r[-16:])
print(re.compile('a', re.A).flags, re.compile('a').flags, re.compile('a', re.I | re.M).flags)
print(re.findall('A', 'aAa', re.I), re.findall('^b', 'a\nb', flags=re.M))
p = re.compile('(?P<first>a)(?P<other>b)')
print(p.groupindex, re.compile('(a)').groupindex, p.match('ab').regs, re.match('(a)(x)?', 'a').regs)
try:
    p.groupindex['other'] = 0
except TypeError as e:
    print('TE', e)
try:
    re.compile(p, re.I)
except ValueError as e:
    print('VE', e)
