import re
p = re.compile('random pattern')
print(isinstance(p, re.Pattern), isinstance(p, re.Match), re.compile(p) is p)
m = p.match('random pattern!')
print(isinstance(m, re.Match), type(m) is re.Match, type(p) is re.Pattern)
print(re.Pattern, re.Match, re.Pattern.__name__, re.Pattern.__module__)
try: re.compile(0)
except TypeError as e: print('TE', e)
try: re.Pattern()
except TypeError as e: print('TE', e)
print(issubclass(re.Pattern, object), isinstance('x', re.Pattern))
from re import Pattern, Match
print(isinstance(p, Pattern), isinstance(m, Match))
