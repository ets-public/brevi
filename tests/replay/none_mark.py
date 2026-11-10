# A result equal to one of the typed build's None marks (-2**63, -max float, "\0None") is never shown as None.
import sys
xs = [-(2**62)]
xs[0] = xs[0] * 2
print(xs[0], xs)
f = [1.7976931348623157e308]
g = -f[0]
print(g)
s = chr(0) + "None"
print(repr(s), s is None)
y = [-(2**62)]
y[0] -= 2**62
print(y[0])
print(int("-9223372036854775808"), int("123456789012345678901234"))
