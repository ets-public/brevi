import re

# a repeated group over a long text: the typed matcher must not overflow its stack
print(repr(re.match('(x)*', 50000 * 'x').group(1)))
print(repr(re.match('(x)*y', 50000 * 'x' + 'y').group(1)))
print(repr(re.match('(x)*?y', 50000 * 'x' + 'y').group(1)))
print(len(re.findall('(?:ab)+', 'ab' * 30000 + '-' + 'ab' * 3)))
print(re.search('(a|b)*c', 'ab' * 20000 + 'c').span())
