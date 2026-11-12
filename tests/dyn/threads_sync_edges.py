import threading, array
b = threading.Barrier(2)
b.abort()
try:
    b.wait()
except threading.BrokenBarrierError as e:
    print(repr(e), e.args, isinstance(e, RuntimeError))
s = threading.Semaphore(2)
print(s.acquire(), s.acquire(), s.acquire(False), s.acquire(timeout=0.01))
bs = threading.BoundedSemaphore(1)
try:
    bs.release()
except ValueError as e:
    print(e)
c = threading.Condition()
try:
    c.notify()
except RuntimeError as e:
    print(e)
with c:
    print(c.wait(0.01), c.wait_for(lambda: True))
