# Calls that change nothing a held place reaches keep the typed build: a module-level lambda that only reads,
# a module's attribute passed to a builtin.
import math

ok = lambda p: 0 <= p < 10
xs = [3, 12, 7]
for x in xs[1:]:
    print(x, ok(x), round(math.pi, x % 3))
