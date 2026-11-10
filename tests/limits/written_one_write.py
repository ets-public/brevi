# One 64 MB write where the caller allows 1 MB written: the count comes before the write, so not one byte of
# it reaches the file (the review's repro let 2 GB through a 1 MB cap with a single large write).
import os

open("limits-written-big.out", "w").write("x" * (64 << 20))
print("not reached")
