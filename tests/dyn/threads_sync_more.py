# Barrier, Condition, Semaphore and BoundedSemaphore as CPython's threading.py has them.
import threading, time

log = []
b = threading.Barrier(3, action=lambda: log.append("action"))
print(b.parties, b.n_waiting, b.broken)
def party(k):
    i = b.wait()
    log.append(("passed", k, i))
ts = [threading.Thread(target=party, args=(k,)) for k in range(2)]
for t in ts:
    t.start()
print("waiting", b.n_waiting)
print("main index", b.wait())
for t in ts:
    t.join()
print(log[0], sorted(log[1:]))

# a timeout breaks the barrier for everyone waiting
b2 = threading.Barrier(2)
try:
    b2.wait(0.01)
except threading.BrokenBarrierError as e:
    print("broken", type(e).__name__, b2.broken)
try:
    b2.wait()
except threading.BrokenBarrierError:
    print("still broken")
b2.reset()
print("after reset", b2.broken, b2.n_waiting)
b3 = threading.Barrier(2, timeout=0.01)
try:
    b3.wait()
except threading.BrokenBarrierError:
    print("default timeout breaks")
b4 = threading.Barrier(2)
def aborter():
    time.sleep(0.01)
    b4.abort()
threading.Thread(target=aborter).start()
try:
    b4.wait()
except threading.BrokenBarrierError:
    print("aborted")
try:
    threading.Barrier(0)
except ValueError as e:
    print("ValueError", e)

# Condition: wait releases the lock, notify wakes in order
cv = threading.Condition()
items = []
got = []
def consumer(k):
    with cv:
        while not items:
            cv.wait()
        got.append(items.pop(0))
cs = [threading.Thread(target=consumer, args=(k,)) for k in range(3)]
for c in cs:
    c.start()
for x in "abc":
    with cv:
        items.append(x)
        cv.notify()
for c in cs:
    c.join()
print("consumers got", sorted(got))
with cv:
    print("wait timeout", cv.wait(0.01))
    print("wait_for", cv.wait_for(lambda: True), cv.wait_for(lambda: False, 0.01))
try:
    cv.notify()
except RuntimeError:
    pass
cv2 = threading.Condition()
try:
    cv2.notify()
except RuntimeError as e:
    print("RuntimeError", e)
try:
    cv2.wait()
except RuntimeError as e:
    print("RuntimeError", e)
done = []
def waiter(k):
    with cv2:
        cv2.wait()
        done.append(k)
ws = [threading.Thread(target=waiter, args=(k,)) for k in range(4)]
for w in ws:
    w.start()
with cv2:
    cv2.notify(2)
time.sleep(0.01)
print("after notify(2)", sorted(done))
with cv2:
    cv2.notify_all()
for w in ws:
    w.join()
print("after notify_all", sorted(done))
cv3 = threading.Condition(threading.Lock())
with cv3:
    print("condition over Lock", cv3.wait(0))

# Semaphore and BoundedSemaphore
s = threading.Semaphore(2)
print(s.acquire(), s.acquire(), s.acquire(blocking=False), s.acquire(timeout=0.01))
order = []
def holder(k):
    with s:
        order.append(("in", k))
        time.sleep(0.01)
        order.append(("out", k))
s.release(2)
hs = [threading.Thread(target=holder, args=(k,)) for k in range(4)]
for h in hs:
    h.start()
for h in hs:
    h.join()
print(len(order), max(sum(1 for e in order[:i + 1] if e[0] == "in") - sum(1 for e in order[:i + 1] if e[0] == "out") for i in range(len(order))))
bs = threading.BoundedSemaphore(1)
bs.acquire()
bs.release()
try:
    bs.release()
except ValueError as e:
    print("ValueError", e)
try:
    threading.Semaphore(-1)
except ValueError as e:
    print("ValueError", e)
