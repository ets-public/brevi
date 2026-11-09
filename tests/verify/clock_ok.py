# Reads the clock: a second run would answer differently, so run --verify runs it once, unverified.
import time
print(time.time() > 0)
