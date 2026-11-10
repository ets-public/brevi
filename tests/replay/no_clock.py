# A second run that writes something else first (here the clock) is stopped, never shown spliced together.
import sys
import time

print(time.time(), file=sys.stderr)
xs = [1]
for i in range(1, 30):
    xs[0] = xs[0] * i
print(xs[0])
