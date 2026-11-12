# Extracted by tools/conformance.py from CPython v3.13.13 Lib/test/test_functools.py, TestLRUC.test_lru_cache_threaded2,
# with the threads' prints collected by the main thread (lru_threaded2.py printed differently on each CPython run)
# (CPython's test suite is under the PSF licence: https://docs.python.org/3/license.html)
import contextlib
import functools as c_functools
import threading
import time

@contextlib.contextmanager
def _conf_start_threads(threads, unlock=None):
    threads = list(threads)
    started = []
    try:
        for t in threads:
            t.start()
            started.append(t)
        yield
    finally:
        try:
            if unlock:
                unlock()
            endtime = time.monotonic()
            for timeout in range(1, 16):
                endtime += 60
                for t in started:
                    t.join(max(endtime - time.monotonic(), 0.01))
                started = [t for t in started if t.is_alive()]
                if not started:
                    break
        finally:
            started = [t for t in started if t.is_alive()]
            if started:
                raise AssertionError('Unable to join %d threads' % len(started))

class _Base1:
    def test_lru_cache_threaded2(self):
        # Simultaneous call with the same arguments
        n, m = 5, 7
        start = threading.Barrier(n+1)
        pause = threading.Barrier(n+1)
        stop = threading.Barrier(n+1)
        @self.module.lru_cache(maxsize=m*n)
        def f(x):
            pause.wait(10)
            return 3 * x
        print(repr(f.cache_info()))
        print(repr((0, 0, m*n, 0)))
        def test():
            for i in range(m):
                start.wait(10)
                got = (f(i), 3 * i)
                with seen_lock:
                    seen.append(got)
                stop.wait(10)
        # the threads hand their results to the main thread instead of printing them: CPython's prints from
        # threads that run at once interleave differently on every run, so they cannot be compared
        seen, seen_lock = [], threading.Lock()
        threads = [threading.Thread(target=test) for k in range(n)]
        with _conf_start_threads(threads):
            for i in range(m):
                start.wait(10)
                stop.reset()
                pause.wait(10)
                start.reset()
                stop.wait(10)
                pause.reset()
                print(sorted(seen))
                seen.clear()
                print(repr(f.cache_info()))
                print(repr((0, (i+1)*n, m*n, i+1)))

class TestLRUC(_Base1):
    module = c_functools

_t = TestLRUC()
_t.test_lru_cache_threaded2()

