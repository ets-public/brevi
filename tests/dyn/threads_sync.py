# Threads take turns at their waits: Event, Lock, RLock, join and sleep order what they print as in CPython.
import threading, time, sys

ev = threading.Event()
lock = threading.Lock()
out = []

def gen(n):
    for i in range(n):
        yield i
        ev.wait()

def worker(k):
    for x in gen(3):
        with lock:
            out.append((k, x))

ts = [threading.Thread(target=worker, args=(k,)) for k in range(3)]
for t in ts:
    t.start()
print("started", [t.is_alive() for t in ts], len(out), ev.is_set())
time.sleep(0.01)
ev.set()
for t in ts:
    t.join()
print(sorted(out), [t.is_alive() for t in ts])

def slow():
    time.sleep(0.05)
    print("slow done")
t = threading.Thread(target=slow)
t.start()
print("main first")
t.join(0.001)
print("still alive", t.is_alive())
t.join()

# each thread has its own exception being handled
try:
    raise KeyError(1)
except KeyError:
    e2 = threading.Event()
    def w():
        print("thread sees", sys.exc_info()[0])
        e2.set()
    threading.Thread(target=w).start()
    e2.wait()
    print("main sees", sys.exc_info()[0].__name__)

r = threading.RLock()
with r:
    with r:
        print("rlock again", r.acquire(False))
        r.release()
def other():
    print("other thread gets rlock", r.acquire(False))
threading.Thread(target=other).start()

l2 = threading.Lock()
l2.acquire()
print("timed", l2.acquire(timeout=0.01), l2.acquire(False), l2.locked())
def releaser():
    time.sleep(0.01)
    l2.release()
threading.Thread(target=releaser).start()
print("got it after release", l2.acquire())
try:
    threading.Lock().release()
except RuntimeError as e:
    print("RuntimeError", e)
try:
    t.start()
except RuntimeError as e:
    print("RuntimeError", e)
print(ev.wait(0), threading.Event().wait(0.01), threading.Event().wait(-1))

def quick(n):
    print("quick", n)
q = threading.Thread(target=quick, args=(1,))
q.start()
q.join()
print("after quick", q.name.startswith("Thread-"), q.name.endswith(" (quick)"))

def nd():
    time.sleep(0.02)
    print("non-daemon thread finishes before exit")
threading.Thread(target=nd).start()
d = threading.Thread(target=lambda: time.sleep(10), daemon=True)
d.start()
print("main end", d.daemon)
