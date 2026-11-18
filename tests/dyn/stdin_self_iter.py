# sys.stdin is its own iterator: iter() gives it back, and reading through next(), readline() and a for loop
# shares one position. type() of it and of sys.stdin.buffer's iterator names the stream classes.
import sys
n = 0
for line in sys.stdin:
    n += len(line)
    if n > 20:
        break
print(n, repr(next(sys.stdin)), repr(sys.stdin.readline()))
it = iter(sys.stdin)
print(next(it), it is sys.stdin, iter(sys.stdin.buffer) is sys.stdin.buffer)
print(type(it).__name__, type(iter(sys.stdin.buffer)).__name__)
rest = [l for l in sys.stdin]
print(len(rest), rest[-1].strip())
print(list(iter(sys.stdin)), next(sys.stdin, "end"))
