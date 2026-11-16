# This file has CRLF line ends (and one lone CR line end below): CPython reads every one as a newline.
x = """\
  a
  b
"""
print(repr(x))
y = "one \
two"
print(repr(y))
z = 1 + \
    2
print(z)
w = '''first
second'''
print(repr(w), len(w))
def f(s):
    """A docstring
    on two lines."""
    return s.upper()
print(f("ok"), repr(f.__doc__))
total = 0
for i in range(3):
    total += i
v = 5print(total, v)
