# re.Scanner: phrases tried in order; without re.UNICODE its \w and IGNORECASE are ASCII-only, as CPython's
import re
from re import Scanner
def s_ident(scanner, token): return token
def s_operator(scanner, token): return "op%s" % token
def s_float(scanner, token): return float(token)
def s_int(scanner, token): return int(token)
scanner = Scanner([
    (r"[a-zA-Z_]\w*", s_ident),
    (r"\d+\.\d*", s_float),
    (r"\d+", s_int),
    (r"=|\+|-|\*|/", s_operator),
    (r"\s+", None),
    ])
print(repr(bool(scanner.scanner.scanner("").pattern)))
print(repr(scanner.scan("sum = 3*foo + 312.50 + bar")))
print(scanner.scan("x = 1 ; y"))
print(scanner.scanner.pattern, repr(scanner.scanner), scanner.scanner.flags, type(scanner.match).__name__, scanner.match.group())
s=re.Scanner([(r'\w+', lambda s,t:t), (r'\s+', None), (r'.', lambda s,t:('o',t))])
print(s.scan('héllo wörld ٣'), s.scanner.flags, repr(s.scanner))
s=re.Scanner([(r'\w+', lambda s,t:t), (r'.', lambda s,t:('o',t))], re.U)
print(s.scan('héllo'), s.scanner.flags)
s=re.Scanner([(r'é', lambda s,t:t), (r'.', lambda s,t:('o',t))], re.I)
print(s.scan('ÉéE'), s.scanner.flags, repr(s.scanner))
s=re.Scanner([(r'a', 1), (r'b', None)]); print(s.scan('abab c'))
s=re.Scanner([(r'a  # letter a', 'A'), (r'\d+ # digits', 'D')], re.X); print(s.scan('a12a!'))
s=re.Scanner([(r'x*', 1)]); print(s.scan('yy'))
try: re.Scanner([(r'(', 1)])
except re.error as e: print('err', e)
print(re.Scanner([(r'x', 1)], re.I|re.M).scanner.flags)
