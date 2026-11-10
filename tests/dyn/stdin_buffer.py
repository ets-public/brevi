# sys.stdin.buffer / sys.stdout.buffer, the fast-I/O idiom (DeepSeek writes it often): bytes in, bytes out,
# kept in order with text writes.
import sys
from collections import OrderedDict

cap = int(sys.stdin.buffer.readline())
head = sys.stdin.buffer.read(4)
rest = sys.stdin.buffer.read().split(b"\n")
lines = [head + rest[0]] + rest[1:]
cache = OrderedDict()
out = []
for line in lines:
    parts = line.split()
    if not parts:
        continue
    if parts[0] == b"get":
        if parts[1] in cache:
            cache.move_to_end(parts[1])
            out.append(cache[parts[1]])
        else:
            out.append(b"-1")
    elif parts[0] == b"put":
        if parts[1] not in cache and len(cache) >= cap:
            cache.popitem(last=False)
        cache[parts[1]] = parts[2]
        cache.move_to_end(parts[1])
sys.stdout.write("text first\n")
n = sys.stdout.buffer.write(b"\n".join(out) + b"\n")
sys.stdout.buffer.flush()
print("wrote", n, "bytes")
print(sys.stdin.buffer.read() == b"", sys.stdin.buffer.readline() == b"", sys.stdin.buffer.readlines())
sys.stderr.buffer.write(b"")
for x in sys.stdin.buffer:
    print("never", x)
try:
    sys.stdout.buffer.write("not bytes")
except TypeError as e:
    print("TypeError:", e)
