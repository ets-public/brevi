# for line in sys.stdin.buffer: each line as bytes, newline kept.
import sys

total = 0
for line in sys.stdin.buffer:
    nums = list(map(int, line.split()))
    total += sum(nums)
    print(repr(line), sum(nums))
print(total)
