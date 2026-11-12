# Text operations on a str holding a lone surrogate, as CPython gives them (handoffs/string-model-design.md,
# step 2): indexing, slicing, iteration, join, %, format(), f-strings and the str methods.
D, E, L = chr(0xD800), chr(0xDC80), chr(0xDFFF)
P = chr(0xF0000)  # a private-use character: it must stay distinct from every surrogate
a = 'ab' + D + 'cD' + E + ' x'
print(ascii(a[2]), ascii(a[-2]), ascii(a[1:4]), ascii(a[::-1]), ascii(a[::2]), a[2] is not a[2] or True)
print([ascii(c) for c in a], ascii(list(D + L)), ascii(''.join(reversed(a))), ascii(sorted(set(D + E + D))))
print(ascii(D * 3), len(E * 4), ascii('-'.join([D, 'x', E])), ascii(D.join('abc')), ascii(''.join([D, L])) == ascii(D + L))
print(ascii('%s|%5s|%-4s|%.1s|%c|%c' % (a, D, E, D + 'q', 0xDABC, L)), ascii(D + '%d' % 7), ascii('%r' % D))
print(ascii('{0}|{1:>4}|{2!s}|{0!r}'.format(D, E, L)), ascii('{x}-{y:^5}'.format_map({'x': D, 'y': E})), ascii(format(D, '*<3')))
print(ascii(f'{D}{E:>3}{L!r}{"z"}'), ascii(f'{a=}'), ascii(D + '{0}'.format('k')), ascii('x{}y'.format(D + P)))
print(ascii(a.split()), ascii(a.split(D)), ascii(a.rsplit('c', 1)), ascii((D + ' ' + E).split(' ')), ascii(a.partition(E)), ascii(a.rpartition('zz')))
print(ascii((' ' + D + ' ').strip()), ascii((D + 'ab' + D).strip(D)), ascii(a.lstrip('ab')), ascii(a.rstrip(' x')))
print(ascii(a.replace(D, '!')), ascii(a.replace('c', E)), ascii(a.replace('', '.', 3)), ascii('aa'.replace('a', D)))
print(a.find(D), a.find(E), a.rfind('c'), a.index(E), a.count(D), a.count(''), a.find(P), 'xyz'.find(D), (D + P).find(P))
print(a.startswith('ab' + D), a.endswith((E, ' x')), a.startswith(D, 2), D.endswith(''), 'abc'.startswith((D, 'a')))
print(ascii(a.upper()), ascii(a.lower()), ascii(a.title()), ascii(a.capitalize()), ascii(a.swapcase()), ascii(a.casefold()))
print(D.isalpha(), D.isalnum(), D.isdigit(), D.isdecimal(), D.isnumeric(), D.isspace(), D.isprintable(), D.isidentifier())
print(D.isupper(), D.islower(), D.istitle(), D.isascii(), ('A' + D).isupper(), ('A' + D + 'b').istitle(), ('x' + D).isidentifier())
print(ascii(D.center(5)), ascii(D.ljust(3, '.')), ascii(D.rjust(3, E)), ascii(('-' + D).zfill(4)), ascii(D.center(1)))
print(ascii(('a' + D + '\n' + E + '\r\nb').splitlines()), ascii((D + '\n').splitlines(True)), ascii(('\t' + D + '\tx').expandtabs(4)))
print(ascii(a.removeprefix('ab' + D)), ascii(a.removesuffix(' x')), ascii(D.removeprefix(E)))
s = D + 'k'
print(s.strip() is s, s.replace('z', 'y') is s, s.ljust(1) is s, s.partition('z')[0] is s)
try:
    a.index(P)
except ValueError as ex:
    print('ValueError', ex)
try:
    D.center(3, 'ab')
except TypeError as ex:
    print('TypeError', ex)
for bad in [(lambda: '%c' % 0x110000), (lambda: '%c' % 'ab'), (lambda: '%c' % (D + D))]:
    try:
        bad()
    except (TypeError, OverflowError) as ex:
        print(type(ex).__name__, ex)
it = iter(D + 'q' + E)
next(it)
print(ascii(it.__reduce__()[1]), it.__reduce__()[2], ascii(list(it)))
