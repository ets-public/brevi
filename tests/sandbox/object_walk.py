import math
x = math.gcd(4, 6)
print(len(x.__class__.__mro__[1].__subclasses__()) > 0)
