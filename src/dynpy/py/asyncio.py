# asyncio for native builds (src/dynpy/mod.rs): CPython 3.13's own pure-Python asyncio (futures.py, tasks.py,
# base_events.py, events.py, locks.py, queues.py, timeouts.py, taskgroups.py, runners.py; PSF license, see
# LICENSE-PSF.txt), cut down to what runs without I/O: the event loop runs callbacks and timers only, so
# programs that wait on sockets, subprocesses, threads or signals run in CPython instead (caps.rs lists what
# this module has; anything else is refused when the program is built). The logic that decides the order
# tasks run in is CPython's, line for line, so tasks interleave (and print) in CPython's order.
#
# Coroutines are Brevi's own (rt_async.rs): coro.send()/coro.throw() run a task's coroutine until it awaits a
# future that is not done, and _brevi_async.suspend(None) is CPython's bare `yield` (sleep(0)).

import _brevi_async
import collections
import heapq
import math
import time

__all__ = ()

_PENDING = 'PENDING'
_CANCELLED = 'CANCELLED'
_FINISHED = 'FINISHED'

FIRST_COMPLETED = 'FIRST_COMPLETED'
FIRST_EXCEPTION = 'FIRST_EXCEPTION'
ALL_COMPLETED = 'ALL_COMPLETED'


class CancelledError(BaseException):
    pass


class InvalidStateError(Exception):
    pass


class QueueEmpty(Exception):
    pass


class QueueFull(Exception):
    pass


class QueueShutDown(Exception):
    pass


class BrokenBarrierError(RuntimeError):
    pass


TimeoutError = TimeoutError


def _unsupported(what):
    return NotImplementedError("brevi: " + what + " is not supported in dynamic builds")


def iscoroutine(obj):
    return _brevi_async.iscoroutine(obj)


def isfuture(obj):
    return getattr(obj, '_asyncio_future_blocking', None) is not None


def _is_awaitable(obj):
    return _brevi_async.isawaitable(obj)


# ---------- events.py ----------

class Handle:
    def __init__(self, callback, args, loop, context=None):
        self._context = context
        self._loop = loop
        self._callback = callback
        self._args = args
        self._cancelled = False

    def __repr__(self):
        raise _unsupported("repr of an asyncio Handle")

    def get_context(self):
        return self._context

    def cancel(self):
        if not self._cancelled:
            self._cancelled = True
            self._callback = None
            self._args = None

    def cancelled(self):
        return self._cancelled

    def _run(self):
        try:
            self._callback(*self._args)
        except (SystemExit, KeyboardInterrupt):
            raise
        except BaseException as exc:
            self._loop.call_exception_handler({
                'message': 'Exception in callback',
                'exception': exc,
                'handle': self,
            })


class TimerHandle(Handle):
    def __init__(self, when, callback, args, loop, context=None):
        super().__init__(callback, args, loop, context)
        self._when = when
        self._scheduled = False

    def __hash__(self):
        return hash(self._when)

    def __lt__(self, other):
        if isinstance(other, TimerHandle):
            return self._when < other._when
        return NotImplemented

    def __le__(self, other):
        if isinstance(other, TimerHandle):
            return self._when < other._when or self.__eq__(other)
        return NotImplemented

    def __gt__(self, other):
        if isinstance(other, TimerHandle):
            return self._when > other._when
        return NotImplemented

    def __ge__(self, other):
        if isinstance(other, TimerHandle):
            return self._when > other._when or self.__eq__(other)
        return NotImplemented

    def __eq__(self, other):
        if isinstance(other, TimerHandle):
            return (self._when == other._when and
                    self._callback == other._callback and
                    self._args == other._args and
                    self._cancelled == other._cancelled)
        return NotImplemented

    def cancel(self):
        if not self._cancelled:
            self._loop._timer_handle_cancelled(self)
        super().cancel()

    def when(self):
        return self._when


_running_loop = None
_event_loop = None


def _get_running_loop():
    return _running_loop


def _set_running_loop(loop):
    global _running_loop
    _running_loop = loop


def get_running_loop():
    loop = _running_loop
    if loop is None:
        raise RuntimeError('no running event loop')
    return loop


def get_event_loop():
    global _event_loop
    if _running_loop is not None:
        return _running_loop
    if _event_loop is None:
        _event_loop = new_event_loop()
    return _event_loop


def set_event_loop(loop):
    global _event_loop
    _event_loop = loop


def new_event_loop():
    return BaseEventLoop()


# ---------- base_events.py ----------

_MIN_SCHEDULED_TIMER_HANDLES = 100
_MIN_CANCELLED_TIMER_HANDLES_FRACTION = 0.5
MAXIMUM_SELECT_TIMEOUT = 24 * 3600


def _run_until_complete_cb(fut):
    if not fut.cancelled():
        exc = fut.exception()
        if isinstance(exc, (SystemExit, KeyboardInterrupt)):
            return
    fut.get_loop().stop()


class BaseEventLoop:
    def __init__(self):
        self._timer_cancelled_count = 0
        self._closed = False
        self._stopping = False
        self._ready = collections.deque()
        self._scheduled = []
        self._default_executor = None
        self._running = False
        self._clock_resolution = 1e-09
        self._exception_handler = None
        self._task_factory = None
        self._debug = False
        self._current_task = None

    def __repr__(self):
        return '<_UnixSelectorEventLoop running=' + str(self.is_running()) + ' closed=' + str(self.is_closed()) + ' debug=' + str(self.get_debug()) + '>'

    def create_future(self):
        return Future(loop=self)

    def create_task(self, coro, *, name=None, context=None):
        self._check_closed()
        if self._task_factory is None:
            task = Task(coro, loop=self, name=name, context=context)
        else:
            if context is None:
                task = self._task_factory(self, coro)
            else:
                task = self._task_factory(self, coro, context=context)
            if name is not None:
                task.set_name(name)
        return task

    def set_task_factory(self, factory):
        if factory is not None and not callable(factory):
            raise TypeError('task factory must be a callable or None')
        self._task_factory = factory

    def get_task_factory(self):
        return self._task_factory

    def _check_closed(self):
        if self._closed:
            raise RuntimeError('Event loop is closed')

    def _check_running(self):
        if self.is_running():
            raise RuntimeError('This event loop is already running')
        if _running_loop is not None:
            raise RuntimeError('Cannot run the event loop while another loop is running')

    def run_forever(self):
        self._check_closed()
        self._check_running()
        self._running = True
        _set_running_loop(self)
        try:
            while True:
                self._run_once()
                if self._stopping:
                    break
        finally:
            self._stopping = False
            self._running = False
            _set_running_loop(None)

    def run_until_complete(self, future):
        self._check_closed()
        self._check_running()
        new_task = not isfuture(future)
        future = ensure_future(future, loop=self)
        if new_task:
            future._log_destroy_pending = False
        future.add_done_callback(_run_until_complete_cb)
        try:
            self.run_forever()
        except:
            if new_task and future.done() and not future.cancelled():
                future.exception()
            raise
        finally:
            future.remove_done_callback(_run_until_complete_cb)
        if not future.done():
            raise RuntimeError('Event loop stopped before Future completed.')
        return future.result()

    def stop(self):
        self._stopping = True

    def close(self):
        if self.is_running():
            raise RuntimeError("Cannot close a running event loop")
        if self._closed:
            return
        self._closed = True
        self._ready.clear()
        self._scheduled.clear()

    def is_closed(self):
        return self._closed

    def is_running(self):
        return self._running

    def time(self):
        return time.monotonic()

    def get_debug(self):
        return self._debug

    def set_debug(self, enabled):
        self._debug = enabled

    async def shutdown_asyncgens(self):
        return

    async def shutdown_default_executor(self, timeout=None):
        return

    def call_later(self, delay, callback, *args, context=None):
        if delay is None:
            raise TypeError('delay must not be None')
        timer = self.call_at(self.time() + delay, callback, *args, context=context)
        return timer

    def call_at(self, when, callback, *args, context=None):
        if when is None:
            raise TypeError("when cannot be None")
        self._check_closed()
        timer = TimerHandle(when, callback, args, self, context)
        heapq.heappush(self._scheduled, timer)
        timer._scheduled = True
        return timer

    def call_soon(self, callback, *args, context=None):
        if self._closed:
            raise RuntimeError('Event loop is closed')
        if not callable(callback):
            raise TypeError('a callable object was expected by call_soon(), got ' + repr(callback))
        handle = Handle(callback, args, self, context)
        self._ready.append(handle)
        return handle

    # call_soon(callback, *args) from this module's own code (callback known to be callable)
    def _call_soon(self, callback, args, context):
        if self._closed:
            raise RuntimeError('Event loop is closed')
        handle = Handle(callback, args, self, context)
        self._ready.append(handle)
        return handle

    def call_soon_threadsafe(self, callback, *args, context=None):
        return self.call_soon(callback, *args, context=context)

    def _timer_handle_cancelled(self, handle):
        if handle._scheduled:
            self._timer_cancelled_count += 1

    def set_exception_handler(self, handler):
        if handler is not None and not callable(handler):
            raise TypeError('A callable object or None is expected, got ' + repr(handler))
        self._exception_handler = handler

    def get_exception_handler(self):
        return self._exception_handler

    def default_exception_handler(self, context):
        import sys
        message = context.get('message')
        if not message:
            message = 'Unhandled exception in event loop'
        exc = context.get('exception')
        if exc is not None:
            print(message, file=sys.stderr)
            print(type(exc).__name__ + ': ' + str(exc), file=sys.stderr)
        else:
            print(message, file=sys.stderr)

    def call_exception_handler(self, context):
        if self._exception_handler is None:
            self.default_exception_handler(context)
        else:
            self._exception_handler(self, context)

    def _run_once(self):
        sched_count = len(self._scheduled)
        if (sched_count > _MIN_SCHEDULED_TIMER_HANDLES and
                self._timer_cancelled_count / sched_count > _MIN_CANCELLED_TIMER_HANDLES_FRACTION):
            new_scheduled = []
            for handle in self._scheduled:
                if handle._cancelled:
                    handle._scheduled = False
                else:
                    new_scheduled.append(handle)
            heapq.heapify(new_scheduled)
            self._scheduled = new_scheduled
            self._timer_cancelled_count = 0
        else:
            while self._scheduled and self._scheduled[0]._cancelled:
                self._timer_cancelled_count -= 1
                handle = heapq.heappop(self._scheduled)
                handle._scheduled = False

        timeout = None
        if self._ready or self._stopping:
            timeout = 0
        elif self._scheduled:
            timeout = self._scheduled[0]._when - self.time()
            if timeout > MAXIMUM_SELECT_TIMEOUT:
                timeout = MAXIMUM_SELECT_TIMEOUT
            elif timeout < 0:
                timeout = 0
        if timeout is None:
            # Nothing can ever wake the loop (CPython would wait forever on its selector).
            raise _unsupported("an event loop with nothing left to run (it would wait forever)")
        if timeout > 0:
            # what EpollSelector.select() waits: whole milliseconds, rounded up
            time.sleep(math.ceil(timeout * 1e3) * 1e-3)

        end_time = self.time() + self._clock_resolution
        while self._scheduled:
            handle = self._scheduled[0]
            if handle._when >= end_time:
                break
            handle = heapq.heappop(self._scheduled)
            handle._scheduled = False
            self._ready.append(handle)

        ntodo = len(self._ready)
        for i in range(ntodo):
            handle = self._ready.popleft()
            if handle._cancelled:
                continue
            handle._run()


# ---------- futures.py ----------

class Future:
    _state = _PENDING
    _result = None
    _exception = None
    _loop = None
    _cancel_message = None
    _cancelled_exc = None
    _asyncio_future_blocking = False
    _log_traceback = False

    def __init__(self, *, loop=None):
        if loop is None:
            self._loop = get_event_loop()
        else:
            self._loop = loop
        self._callbacks = []

    def __repr__(self):
        if self._callbacks:
            raise _unsupported("repr of an asyncio Future with callbacks")
        if self._state == _FINISHED:
            if self._exception is not None:
                return '<' + type(self).__name__ + ' finished exception=' + repr(self._exception) + '>'
            return '<' + type(self).__name__ + ' finished result=' + _short_repr(self._result) + '>'
        return '<' + type(self).__name__ + ' ' + self._state.lower() + '>'

    def get_loop(self):
        loop = self._loop
        if loop is None:
            raise RuntimeError("Future object is not initialized.")
        return loop

    def _make_cancelled_error(self):
        if self._cancelled_exc is not None:
            exc = self._cancelled_exc
            self._cancelled_exc = None
            return exc
        if self._cancel_message is None:
            exc = CancelledError()
        else:
            exc = CancelledError(self._cancel_message)
        return exc

    def cancel(self, msg=None):
        self._log_traceback = False
        if self._state != _PENDING:
            return False
        self._state = _CANCELLED
        self._cancel_message = msg
        self._schedule_callbacks()
        return True

    def _schedule_callbacks(self):
        callbacks = self._callbacks[:]
        if not callbacks:
            return
        self._callbacks[:] = []
        loop = self._loop
        for callback, ctx in callbacks:
            loop._call_soon(callback, (self,), ctx)

    def cancelled(self):
        return self._state == _CANCELLED

    def done(self):
        return self._state != _PENDING

    def result(self):
        if self._state == _CANCELLED:
            raise self._make_cancelled_error()
        if self._state != _FINISHED:
            raise InvalidStateError('Result is not ready.')
        self._log_traceback = False
        if self._exception is not None:
            raise self._exception
        return self._result

    def exception(self):
        if self._state == _CANCELLED:
            raise self._make_cancelled_error()
        if self._state != _FINISHED:
            raise InvalidStateError('Exception is not set.')
        self._log_traceback = False
        return self._exception

    def add_done_callback(self, fn, *, context=None):
        if self._state != _PENDING:
            self._loop.call_soon(fn, self, context=context)
        else:
            self._callbacks.append((fn, context))

    def remove_done_callback(self, fn):
        filtered_callbacks = [(f, ctx) for (f, ctx) in self._callbacks if f != fn]
        removed_count = len(self._callbacks) - len(filtered_callbacks)
        if removed_count:
            self._callbacks[:] = filtered_callbacks
        return removed_count

    def set_result(self, result):
        if self._state != _PENDING:
            raise InvalidStateError(self._state + ': ' + repr(self))
        self._result = result
        self._state = _FINISHED
        self._schedule_callbacks()

    def set_exception(self, exception):
        if self._state != _PENDING:
            raise InvalidStateError(self._state + ': ' + repr(self))
        if isinstance(exception, type):
            exception = exception()
        if isinstance(exception, StopIteration):
            new_exc = RuntimeError("StopIteration interacts badly with "
                                   "generators and cannot be raised into a "
                                   "Future")
            exception = new_exc
        self._exception = exception
        self._state = _FINISHED
        self._schedule_callbacks()
        self._log_traceback = True

    def __await__(self):
        if not self.done():
            self._asyncio_future_blocking = True
            yield self
        if not self.done():
            raise RuntimeError("await wasn't used with future")
        return self.result()

    __iter__ = __await__


def _short_repr(v):
    r = repr(v)
    if len(r) > 20:
        r = r[:8] + '...' + r[-9:]
    return r


def _get_loop(fut):
    return fut.get_loop()


def _set_result_unless_cancelled(fut, result):
    if fut.cancelled():
        return
    fut.set_result(result)


# ---------- tasks.py ----------

_task_name_counter = 0
_scheduled_tasks = {}


def _next_task_name():
    global _task_name_counter
    _task_name_counter += 1
    return 'Task-' + str(_task_name_counter)


def current_task(loop=None):
    if loop is None:
        loop = get_running_loop()
    return loop._current_task


def all_tasks(loop=None):
    if loop is None:
        loop = get_running_loop()
    return {t for t in list(_scheduled_tasks.values()) if t._loop is loop and not t.done()}


def _register_task(task):
    _scheduled_tasks[id(task)] = task


def _unregister_task(task):
    _scheduled_tasks.pop(id(task), None)


def _enter_task(loop, task):
    if loop._current_task is not None:
        raise RuntimeError("Cannot enter into task while another task is being executed.")
    loop._current_task = task


def _leave_task(loop, task):
    if loop._current_task is not task:
        raise RuntimeError("Leaving task does not match the current task.")
    loop._current_task = None


class Task(Future):
    _log_destroy_pending = True

    def __init__(self, coro, *, loop=None, name=None, context=None, eager_start=False):
        super().__init__(loop=loop)
        if not iscoroutine(coro):
            self._log_destroy_pending = False
            raise TypeError("a coroutine was expected, got " + repr(coro))
        if name is None:
            self._name = _next_task_name()
        else:
            self._name = str(name)
        self._num_cancels_requested = 0
        self._must_cancel = False
        self._fut_waiter = None
        self._coro = coro
        self._context = context
        if eager_start:
            raise _unsupported("eager tasks")
        self._loop._call_soon(self._step, (), self._context)
        _register_task(self)

    def __repr__(self):
        raise _unsupported("repr of an asyncio Task")

    def get_coro(self):
        return self._coro

    def get_context(self):
        return self._context

    def get_name(self):
        return self._name

    def set_name(self, value):
        self._name = str(value)

    def set_result(self, result):
        raise RuntimeError('Task does not support set_result operation')

    def set_exception(self, exception):
        raise RuntimeError('Task does not support set_exception operation')

    def cancel(self, msg=None):
        self._log_traceback = False
        if self.done():
            return False
        self._num_cancels_requested += 1
        if self._fut_waiter is not None:
            if self._fut_waiter.cancel(msg=msg):
                return True
        self._must_cancel = True
        self._cancel_message = msg
        return True

    def cancelling(self):
        return self._num_cancels_requested

    def uncancel(self):
        if self._num_cancels_requested > 0:
            self._num_cancels_requested -= 1
            if self._num_cancels_requested == 0:
                self._must_cancel = False
        return self._num_cancels_requested

    def _step(self, exc=None):
        if self._state != _PENDING:
            raise InvalidStateError('_step(): already done')
        if self._must_cancel:
            if not isinstance(exc, CancelledError):
                exc = self._make_cancelled_error()
            self._must_cancel = False
        self._fut_waiter = None
        loop = self._loop
        if loop._current_task is not None:
            raise RuntimeError("Cannot enter into task while another task is being executed.")
        loop._current_task = self
        try:
            self._step_run_and_handle_result(exc)
        finally:
            if loop._current_task is not self:
                raise RuntimeError("Leaving task does not match the current task.")
            loop._current_task = None

    def _step_run_and_handle_result(self, exc):
        coro = self._coro
        try:
            if exc is None:
                result = coro.send(None)
            else:
                result = coro.throw(exc)
        except StopIteration as e:
            if self._must_cancel:
                self._must_cancel = False
                Future.cancel(self, msg=self._cancel_message)
            else:
                Future.set_result(self, e.value)
            _unregister_task(self)
        except CancelledError as e:
            self._cancelled_exc = e
            Future.cancel(self)
            _unregister_task(self)
        except (KeyboardInterrupt, SystemExit) as e:
            Future.set_exception(self, e)
            _unregister_task(self)
            raise
        except BaseException as e:
            Future.set_exception(self, e)
            _unregister_task(self)
        else:
            if result is None:
                # sleep(0)
                self._loop._call_soon(self._step, (), self._context)
                return
            blocking = getattr(result, '_asyncio_future_blocking', None)
            if blocking is not None:
                if _get_loop(result) is not self._loop:
                    new_exc = RuntimeError('Task got Future attached to a different loop')
                    self._loop.call_soon(self._step, new_exc, context=self._context)
                elif blocking:
                    if result is self:
                        new_exc = RuntimeError('Task cannot await on itself')
                        self._loop.call_soon(self._step, new_exc, context=self._context)
                    else:
                        result._asyncio_future_blocking = False
                        tr = type(result)
                        if tr is Future or tr is Task or tr is _GatheringFuture:
                            # add_done_callback, inlined
                            if result._state != _PENDING:
                                self._loop._call_soon(self._wakeup, (result,), self._context)
                            else:
                                result._callbacks.append((self._wakeup, self._context))
                        else:
                            result.add_done_callback(self._wakeup, context=self._context)
                        self._fut_waiter = result
                        if self._must_cancel:
                            if self._fut_waiter.cancel(msg=self._cancel_message):
                                self._must_cancel = False
                else:
                    new_exc = RuntimeError('yield was used instead of yield from in task')
                    self._loop.call_soon(self._step, new_exc, context=self._context)
            elif result is None:
                self._loop.call_soon(self._step, context=self._context)
            else:
                new_exc = RuntimeError('Task got bad yield: ' + repr(result))
                self._loop.call_soon(self._step, new_exc, context=self._context)

    def _wakeup(self, future):
        try:
            future.result()
        except BaseException as exc:
            self._step(exc)
        else:
            self._step()


def create_task(coro, *, name=None, context=None):
    loop = get_running_loop()
    if context is None:
        task = loop.create_task(coro, name=name)
    else:
        task = loop.create_task(coro, name=name, context=context)
    return task


def ensure_future(coro_or_future, *, loop=None):
    if isfuture(coro_or_future):
        if loop is not None and loop is not _get_loop(coro_or_future):
            raise ValueError('The future belongs to a different loop than '
                             'the one specified as the loop argument')
        return coro_or_future
    should_close = True
    if not iscoroutine(coro_or_future):
        if _is_awaitable(coro_or_future):
            coro_or_future = _wrap_awaitable(coro_or_future)
            should_close = False
        else:
            raise TypeError('An asyncio.Future, a coroutine or an awaitable '
                            'is required')
    if loop is None:
        loop = get_event_loop()
    try:
        return loop.create_task(coro_or_future)
    except RuntimeError:
        if should_close:
            coro_or_future.close()
        raise


async def _wrap_awaitable(awaitable):
    return await awaitable


async def sleep(delay, result=None):
    if delay <= 0:
        _brevi_async.suspend(None)
        return result
    if math.isnan(delay):
        raise ValueError("Invalid delay: NaN (not a number)")
    loop = get_running_loop()
    future = loop.create_future()
    h = loop.call_later(delay, _set_result_unless_cancelled, future, result)
    try:
        return await future
    finally:
        h.cancel()


class _GatheringFuture(Future):
    def __init__(self, children, *, loop):
        super().__init__(loop=loop)
        self._children = children
        self._cancel_requested = False

    def cancel(self, msg=None):
        if self.done():
            return False
        ret = False
        for child in self._children:
            if child.cancel(msg=msg):
                ret = True
        if ret:
            self._cancel_requested = True
        return ret


def gather(*coros_or_futures, return_exceptions=False):
    if not coros_or_futures:
        loop = get_event_loop()
        outer = loop.create_future()
        outer.set_result([])
        return outer

    nfuts = 0
    nfinished = 0
    outer = None
    children = []

    def _done_callback(fut):
        nonlocal nfinished
        nfinished += 1
        if outer is None or outer.done():
            if not fut.cancelled():
                fut.exception()
            return
        if not return_exceptions:
            if fut.cancelled():
                exc = fut._make_cancelled_error()
                outer.set_exception(exc)
                return
            else:
                exc = fut.exception()
                if exc is not None:
                    outer.set_exception(exc)
                    return
        if nfinished == nfuts:
            results = []
            for fut in children:
                if fut.cancelled():
                    res = CancelledError('' if fut._cancel_message is None else fut._cancel_message)
                else:
                    res = fut.exception()
                    if res is None:
                        res = fut.result()
                results.append(res)
            if outer._cancel_requested:
                exc = fut._make_cancelled_error()
                outer.set_exception(exc)
            else:
                outer.set_result(results)

    arg_to_fut = {}
    done_futs = []
    loop = None
    for arg in coros_or_futures:
        if arg not in arg_to_fut:
            fut = ensure_future(arg, loop=loop)
            if loop is None:
                loop = _get_loop(fut)
            if fut is not arg:
                fut._log_destroy_pending = False
            nfuts += 1
            arg_to_fut[arg] = fut
            if fut.done():
                done_futs.append(fut)
            else:
                fut.add_done_callback(_done_callback)
        else:
            fut = arg_to_fut[arg]
        children.append(fut)
    outer = _GatheringFuture(children, loop=loop)
    for fut in done_futs:
        _done_callback(fut)
    return outer


def shield(arg):
    inner = ensure_future(arg)
    if inner.done():
        return inner
    loop = _get_loop(inner)
    outer = loop.create_future()

    def _inner_done_callback(inner):
        if outer.cancelled():
            if not inner.cancelled():
                inner.exception()
            return
        if inner.cancelled():
            outer.cancel()
        else:
            exc = inner.exception()
            if exc is not None:
                outer.set_exception(exc)
            else:
                outer.set_result(inner.result())

    def _outer_done_callback(outer):
        if not inner.done():
            inner.remove_done_callback(_inner_done_callback)

    inner.add_done_callback(_inner_done_callback)
    outer.add_done_callback(_outer_done_callback)
    return outer


def _release_waiter(waiter, *args):
    if not waiter.done():
        waiter.set_result(None)


async def wait_for(fut, timeout):
    if timeout is not None and timeout <= 0:
        fut = ensure_future(fut)
        if fut.done():
            return fut.result()
        await _cancel_and_wait(fut)
        try:
            return fut.result()
        except CancelledError as exc:
            raise TimeoutError from exc
    async with timeout_(timeout):
        return await fut


async def _cancel_and_wait(fut):
    loop = get_running_loop()
    waiter = loop.create_future()

    def cb(f):
        _release_waiter(waiter, f)

    fut.add_done_callback(cb)
    try:
        fut.cancel()
        await waiter
    finally:
        fut.remove_done_callback(cb)


async def wait(fs, *, timeout=None, return_when=ALL_COMPLETED):
    if isfuture(fs) or iscoroutine(fs):
        raise TypeError("expect a list of futures, not " + type(fs).__name__)
    if not fs:
        raise ValueError('Set of Tasks/Futures is empty.')
    if return_when not in (FIRST_COMPLETED, FIRST_EXCEPTION, ALL_COMPLETED):
        raise ValueError('Invalid return_when value: ' + str(return_when))
    fs = set(fs)
    if any(iscoroutine(f) for f in fs):
        raise TypeError("Passing coroutines is forbidden, use tasks explicitly.")
    loop = get_running_loop()
    return await _wait(fs, timeout, return_when, loop)


async def _wait(fs, timeout, return_when, loop):
    waiter = loop.create_future()
    timeout_handle = None
    if timeout is not None:
        timeout_handle = loop.call_later(timeout, _release_waiter, waiter)
    counter = len(fs)

    def _on_completion(f):
        nonlocal counter
        counter -= 1
        if (counter <= 0 or
                return_when == FIRST_COMPLETED or
                return_when == FIRST_EXCEPTION and (not f.cancelled() and f.exception() is not None)):
            if timeout_handle is not None:
                timeout_handle.cancel()
            if not waiter.done():
                waiter.set_result(None)

    for f in fs:
        f.add_done_callback(_on_completion)
    try:
        await waiter
    finally:
        if timeout_handle is not None:
            timeout_handle.cancel()
        for f in fs:
            f.remove_done_callback(_on_completion)
    done, pending = set(), set()
    for f in fs:
        if f.done():
            done.add(f)
        else:
            pending.add(f)
    return done, pending


class _AsCompletedIterator:
    def __init__(self, aws, timeout):
        self._done = Queue()
        self._timeout_handle = None
        loop = get_event_loop()
        todo = {ensure_future(aw, loop=loop) for aw in set(aws)}
        for f in todo:
            f.add_done_callback(self._handle_completion)
        if todo and timeout is not None:
            self._timeout_handle = loop.call_later(timeout, self._handle_timeout)
        self._todo = todo
        self._todo_left = len(todo)

    def __aiter__(self):
        return self

    def __iter__(self):
        return self

    async def __anext__(self):
        if not self._todo_left:
            raise StopAsyncIteration
        self._todo_left -= 1
        return await self._wait_for_one()

    def __next__(self):
        if not self._todo_left:
            raise StopIteration
        self._todo_left -= 1
        return self._wait_for_one(resolve=True)

    def _handle_timeout(self):
        for f in self._todo:
            f.remove_done_callback(self._handle_completion)
            self._done.put_nowait(None)
        self._todo.clear()

    def _handle_completion(self, f):
        if not self._todo:
            return
        self._todo.remove(f)
        self._done.put_nowait(f)
        if not self._todo and self._timeout_handle is not None:
            self._timeout_handle.cancel()

    async def _wait_for_one(self, resolve=False):
        f = await self._done.get()
        if f is None:
            raise TimeoutError
        return f.result() if resolve else f


def as_completed(fs, *, timeout=None):
    if _is_awaitable(fs):
        raise TypeError("expects an iterable of awaitables, not " + type(fs).__name__)
    return _AsCompletedIterator(fs, timeout)


# ---------- runners.py ----------

class Runner:
    def __init__(self, *, debug=None, loop_factory=None):
        self._state = 'created'
        self._debug = debug
        self._loop_factory = loop_factory
        self._loop = None
        self._set_event_loop = False

    def __enter__(self):
        self._lazy_init()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def close(self):
        if self._state != 'initialized':
            return
        try:
            loop = self._loop
            _cancel_all_tasks(loop)
            loop.run_until_complete(loop.shutdown_asyncgens())
            loop.run_until_complete(loop.shutdown_default_executor(300))
        finally:
            if self._set_event_loop:
                set_event_loop(None)
            loop.close()
            self._loop = None
            self._state = 'closed'

    def get_loop(self):
        self._lazy_init()
        return self._loop

    def run(self, coro, *, context=None):
        if not iscoroutine(coro):
            raise ValueError("a coroutine was expected, got " + repr(coro))
        if _get_running_loop() is not None:
            raise RuntimeError("Runner.run() cannot be called from a running event loop")
        self._lazy_init()
        task = self._loop.create_task(coro, context=context)
        return self._loop.run_until_complete(task)

    def _lazy_init(self):
        if self._state == 'closed':
            raise RuntimeError("Runner is closed")
        if self._state == 'initialized':
            return
        if self._loop_factory is None:
            self._loop = new_event_loop()
            if not self._set_event_loop:
                set_event_loop(self._loop)
                self._set_event_loop = True
        else:
            self._loop = self._loop_factory()
        if self._debug is not None:
            self._loop.set_debug(self._debug)
        self._state = 'initialized'


def run(main, *, debug=None, loop_factory=None):
    if _get_running_loop() is not None:
        raise RuntimeError("asyncio.run() cannot be called from a running event loop")
    with Runner(debug=debug, loop_factory=loop_factory) as runner:
        return runner.run(main)


def _cancel_all_tasks(loop):
    to_cancel = all_tasks(loop)
    if not to_cancel:
        return
    for task in to_cancel:
        task.cancel()
    loop.run_until_complete(gather(*to_cancel, return_exceptions=True))
    for task in to_cancel:
        if task.cancelled():
            continue
        if task.exception() is not None:
            loop.call_exception_handler({
                'message': 'unhandled exception during asyncio.run() shutdown',
                'exception': task.exception(),
                'task': task,
            })


# ---------- locks.py ----------

class _LoopBoundMixin:
    _loop = None

    def _get_loop(self):
        loop = _get_running_loop()
        if self._loop is None:
            self._loop = loop
        if loop is not self._loop:
            raise RuntimeError('lock is bound to a different event loop')
        return loop


class _ContextManagerMixin:
    async def __aenter__(self):
        await self.acquire()
        return None

    async def __aexit__(self, exc_type, exc, tb):
        self.release()


class Lock(_ContextManagerMixin, _LoopBoundMixin):
    def __init__(self):
        self._waiters = None
        self._locked = False

    def __repr__(self):
        extra = 'locked' if self._locked else 'unlocked'
        if self._waiters:
            extra = extra + ', waiters:' + str(len(self._waiters))
        return '<asyncio.locks.Lock object at ' + hex(id(self)) + ' [' + extra + ']>'

    def locked(self):
        return self._locked

    async def acquire(self):
        if (not self._locked and (self._waiters is None or
                                  all(w.cancelled() for w in self._waiters))):
            self._locked = True
            return True
        if self._waiters is None:
            self._waiters = collections.deque()
        fut = self._get_loop().create_future()
        self._waiters.append(fut)
        try:
            try:
                await fut
            finally:
                self._waiters.remove(fut)
        except CancelledError:
            if not self._locked:
                self._wake_up_first()
            raise
        self._locked = True
        return True

    def release(self):
        if self._locked:
            self._locked = False
            self._wake_up_first()
        else:
            raise RuntimeError('Lock is not acquired.')

    def _wake_up_first(self):
        if not self._waiters:
            return
        fut = self._waiters[0]
        if not fut.done():
            fut.set_result(True)


class Event(_LoopBoundMixin):
    def __init__(self):
        self._waiters = collections.deque()
        self._value = False

    def __repr__(self):
        extra = 'set' if self._value else 'unset'
        if self._waiters:
            extra = extra + ', waiters:' + str(len(self._waiters))
        return '<asyncio.locks.Event object at ' + hex(id(self)) + ' [' + extra + ']>'

    def is_set(self):
        return self._value

    def set(self):
        if not self._value:
            self._value = True
            for fut in self._waiters:
                if not fut.done():
                    fut.set_result(True)

    def clear(self):
        self._value = False

    async def wait(self):
        if self._value:
            return True
        fut = self._get_loop().create_future()
        self._waiters.append(fut)
        try:
            await fut
            return True
        finally:
            self._waiters.remove(fut)


class Condition(_ContextManagerMixin, _LoopBoundMixin):
    def __init__(self, lock=None):
        if lock is None:
            lock = Lock()
        self._lock = lock
        self.locked = lock.locked
        self.acquire = lock.acquire
        self.release = lock.release
        self._waiters = collections.deque()

    def __repr__(self):
        extra = 'locked' if self.locked() else 'unlocked'
        if self._waiters:
            extra = extra + ', waiters:' + str(len(self._waiters))
        return '<asyncio.locks.Condition object at ' + hex(id(self)) + ' [' + extra + ']>'

    async def wait(self):
        if not self.locked():
            raise RuntimeError('cannot wait on un-acquired lock')
        fut = self._get_loop().create_future()
        self.release()
        try:
            try:
                self._waiters.append(fut)
                try:
                    await fut
                    return True
                finally:
                    self._waiters.remove(fut)
            finally:
                err = None
                while True:
                    try:
                        await self.acquire()
                        break
                    except CancelledError as e:
                        err = e
                if err is not None:
                    raise err
        except BaseException:
            self._notify(1)
            raise

    async def wait_for(self, predicate):
        result = predicate()
        while not result:
            await self.wait()
            result = predicate()
        return result

    def notify(self, n=1):
        if not self.locked():
            raise RuntimeError('cannot notify on un-acquired lock')
        self._notify(n)

    def _notify(self, n):
        idx = 0
        for fut in self._waiters:
            if idx >= n:
                break
            if not fut.done():
                idx += 1
                fut.set_result(False)

    def notify_all(self):
        self.notify(len(self._waiters))


class Semaphore(_ContextManagerMixin, _LoopBoundMixin):
    def __init__(self, value=1):
        if value < 0:
            raise ValueError("Semaphore initial value must be >= 0")
        self._waiters = None
        self._value = value

    def __repr__(self):
        extra = 'locked' if self.locked() else 'unlocked, value:' + str(self._value)
        if self._waiters:
            extra = extra + ', waiters:' + str(len(self._waiters))
        return '<asyncio.locks.' + type(self).__name__ + ' object at ' + hex(id(self)) + ' [' + extra + ']>'

    def locked(self):
        return self._value == 0 or (any(not w.cancelled() for w in (self._waiters or ())))

    async def acquire(self):
        if not self.locked():
            self._value -= 1
            return True
        if self._waiters is None:
            self._waiters = collections.deque()
        fut = self._get_loop().create_future()
        self._waiters.append(fut)
        try:
            try:
                await fut
            finally:
                self._waiters.remove(fut)
        except CancelledError:
            if fut.done() and not fut.cancelled():
                self._value += 1
            raise
        finally:
            while self._value > 0:
                if not self._wake_up_next():
                    break
        return True

    def release(self):
        self._value += 1
        self._wake_up_next()

    def _wake_up_next(self):
        if not self._waiters:
            return False
        for fut in self._waiters:
            if not fut.done():
                self._value -= 1
                fut.set_result(True)
                return True
        return False


class BoundedSemaphore(Semaphore):
    def __init__(self, value=1):
        self._bound_value = value
        super().__init__(value)

    def release(self):
        if self._value >= self._bound_value:
            raise ValueError('BoundedSemaphore released too many times')
        super().release()


# ---------- queues.py ----------

class Queue(_LoopBoundMixin):
    def __init__(self, maxsize=0):
        self._maxsize = maxsize
        self._getters = collections.deque()
        self._putters = collections.deque()
        self._unfinished_tasks = 0
        self._finished = Event()
        self._finished.set()
        self._init(maxsize)
        self._is_shutdown = False

    def _init(self, maxsize):
        self._queue = collections.deque()

    def _get(self):
        return self._queue.popleft()

    def _put(self, item):
        self._queue.append(item)

    def _wakeup_next(self, waiters):
        while waiters:
            waiter = waiters.popleft()
            if not waiter.done():
                waiter.set_result(None)
                break

    def __repr__(self):
        return '<' + type(self).__name__ + ' at ' + hex(id(self)) + ' ' + self._format() + '>'

    def __str__(self):
        return '<' + type(self).__name__ + ' ' + self._format() + '>'

    def _format(self):
        result = 'maxsize=' + repr(self._maxsize)
        if self._queue:
            result += ' _queue=' + repr(list(self._queue))
        if self._getters:
            result += ' _getters[' + str(len(self._getters)) + ']'
        if self._putters:
            result += ' _putters[' + str(len(self._putters)) + ']'
        if self._unfinished_tasks:
            result += ' tasks=' + str(self._unfinished_tasks)
        if self._is_shutdown:
            result += ' shutdown'
        return result

    def qsize(self):
        return len(self._queue)

    @property
    def maxsize(self):
        return self._maxsize

    def empty(self):
        return not self._queue

    def full(self):
        if self._maxsize <= 0:
            return False
        else:
            return self.qsize() >= self._maxsize

    async def put(self, item):
        while self.full():
            if self._is_shutdown:
                raise QueueShutDown
            putter = self._get_loop().create_future()
            self._putters.append(putter)
            try:
                await putter
            except:
                putter.cancel()
                try:
                    self._putters.remove(putter)
                except ValueError:
                    pass
                if not self.full() and not putter.cancelled():
                    self._wakeup_next(self._putters)
                raise
        return self.put_nowait(item)

    def put_nowait(self, item):
        if self._is_shutdown:
            raise QueueShutDown
        if self.full():
            raise QueueFull
        self._put(item)
        self._unfinished_tasks += 1
        self._finished.clear()
        self._wakeup_next(self._getters)

    async def get(self):
        while self.empty():
            if self._is_shutdown and self.empty():
                raise QueueShutDown
            getter = self._get_loop().create_future()
            self._getters.append(getter)
            try:
                await getter
            except:
                getter.cancel()
                try:
                    self._getters.remove(getter)
                except ValueError:
                    pass
                if not self.empty() and not getter.cancelled():
                    self._wakeup_next(self._getters)
                raise
        return self.get_nowait()

    def get_nowait(self):
        if self.empty():
            if self._is_shutdown:
                raise QueueShutDown
            raise QueueEmpty
        item = self._get()
        self._wakeup_next(self._putters)
        return item

    def task_done(self):
        if self._unfinished_tasks <= 0:
            raise ValueError('task_done() called too many times')
        self._unfinished_tasks -= 1
        if self._unfinished_tasks == 0:
            self._finished.set()

    async def join(self):
        if self._unfinished_tasks > 0:
            await self._finished.wait()

    def shutdown(self, immediate=False):
        self._is_shutdown = True
        if immediate:
            while not self.empty():
                self._get()
                if self._unfinished_tasks > 0:
                    self._unfinished_tasks -= 1
            if self._unfinished_tasks == 0:
                self._finished.set()
        while self._getters:
            getter = self._getters.popleft()
            if not getter.done():
                getter.set_result(None)
        while self._putters:
            putter = self._putters.popleft()
            if not putter.done():
                putter.set_result(None)


class PriorityQueue(Queue):
    def _init(self, maxsize):
        self._queue = []

    def _put(self, item):
        heapq.heappush(self._queue, item)

    def _get(self):
        return heapq.heappop(self._queue)


class LifoQueue(Queue):
    def _init(self, maxsize):
        self._queue = []

    def _put(self, item):
        self._queue.append(item)

    def _get(self):
        return self._queue.pop()


# ---------- timeouts.py ----------

class Timeout:
    def __init__(self, when):
        self._state = 'created'
        self._timeout_handler = None
        self._task = None
        self._when = when

    def when(self):
        return self._when

    def reschedule(self, when):
        if self._state != 'active':
            if self._state == 'created':
                raise RuntimeError("Timeout has not been entered")
            raise RuntimeError("Cannot change state of " + self._state + " Timeout")
        self._when = when
        if self._timeout_handler is not None:
            self._timeout_handler.cancel()
        if when is None:
            self._timeout_handler = None
        else:
            loop = get_running_loop()
            if when <= loop.time():
                self._timeout_handler = loop.call_soon(self._on_timeout)
            else:
                self._timeout_handler = loop.call_at(when, self._on_timeout)

    def expired(self):
        return self._state in ('expiring', 'expired')

    def __repr__(self):
        info = ''
        if self._state == 'active':
            when = round(self._when, 3) if self._when is not None else None
            info = ' when=' + str(when)
        return '<Timeout [' + self._state + ']' + info + '>'

    async def __aenter__(self):
        if self._state != 'created':
            raise RuntimeError("Timeout has already been entered")
        task = current_task()
        if task is None:
            raise RuntimeError("Timeout should be used inside a task")
        self._state = 'active'
        self._task = task
        self._cancelling = self._task.cancelling()
        self.reschedule(self._when)
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self._timeout_handler is not None:
            self._timeout_handler.cancel()
            self._timeout_handler = None
        if self._state == 'expiring':
            self._state = 'expired'
            if self._task.uncancel() <= self._cancelling and exc_type is not None:
                if issubclass(exc_type, CancelledError):
                    raise TimeoutError from exc_val
        elif self._state == 'active':
            self._state = 'finished'
        return None

    def _on_timeout(self):
        self._task.cancel()
        self._state = 'expiring'
        self._timeout_handler = None


def timeout(delay):
    loop = get_running_loop()
    return Timeout(loop.time() + delay if delay is not None else None)


timeout_ = timeout


def timeout_at(when):
    return Timeout(when)


# ---------- taskgroups.py ----------

class TaskGroup:
    def __init__(self):
        self._entered = False
        self._exiting = False
        self._aborting = False
        self._loop = None
        self._parent_task = None
        self._parent_cancel_requested = False
        self._tasks = {}
        self._errors = []
        self._base_error = None
        self._on_completed_fut = None

    def __repr__(self):
        info = ''
        if self._tasks:
            info += ' tasks=' + str(len(self._tasks))
        if self._errors:
            info += ' errors=' + str(len(self._errors))
        if self._aborting:
            info += ' cancelling'
        elif self._entered:
            info += ' entered'
        return '<TaskGroup' + info + '>'

    async def __aenter__(self):
        if self._entered:
            raise RuntimeError("TaskGroup " + repr(self) + " has already been entered")
        if self._loop is None:
            self._loop = get_running_loop()
        self._parent_task = current_task(self._loop)
        if self._parent_task is None:
            raise RuntimeError("TaskGroup " + repr(self) + " cannot determine the parent task")
        self._entered = True
        return self

    async def __aexit__(self, et, exc, tb):
        try:
            return await self._aexit(et, exc)
        finally:
            self._parent_task = None
            self._errors = None
            self._base_error = None

    async def _aexit(self, et, exc):
        self._exiting = True
        if (exc is not None and
                self._is_base_error(exc) and
                self._base_error is None):
            self._base_error = exc
        if et is not None and issubclass(et, CancelledError):
            propagate_cancellation_error = exc
        else:
            propagate_cancellation_error = None
        if et is not None:
            if not self._aborting:
                self._abort()
        while self._tasks:
            if self._on_completed_fut is None:
                self._on_completed_fut = self._loop.create_future()
            try:
                await self._on_completed_fut
            except CancelledError as ex:
                if not self._aborting:
                    propagate_cancellation_error = ex
                    self._abort()
            self._on_completed_fut = None
        if self._base_error is not None:
            raise self._base_error
        if self._parent_cancel_requested:
            if self._parent_task.uncancel() == 0:
                propagate_cancellation_error = None
        if propagate_cancellation_error is not None and not self._errors:
            raise propagate_cancellation_error
        if et is not None and not issubclass(et, CancelledError):
            self._errors.append(exc)
        if self._errors:
            if self._parent_task.cancelling():
                self._parent_task.uncancel()
                self._parent_task.cancel()
            raise BaseExceptionGroup('unhandled errors in a TaskGroup', self._errors) from None

    def create_task(self, coro, *, name=None, context=None):
        if not self._entered:
            coro.close()
            raise RuntimeError("TaskGroup " + repr(self) + " has not been entered")
        if self._exiting and not self._tasks:
            coro.close()
            raise RuntimeError("TaskGroup " + repr(self) + " is finished")
        if self._aborting:
            coro.close()
            raise RuntimeError("TaskGroup " + repr(self) + " is shutting down")
        if context is None:
            task = self._loop.create_task(coro, name=name)
        else:
            task = self._loop.create_task(coro, name=name, context=context)
        self._tasks[id(task)] = task
        task.add_done_callback(self._on_task_done)
        return task

    def _is_base_error(self, exc):
        return isinstance(exc, (SystemExit, KeyboardInterrupt))

    def _abort(self):
        self._aborting = True
        for t in list(self._tasks.values()):
            if not t.done():
                t.cancel()

    def _on_task_done(self, task):
        self._tasks.pop(id(task), None)
        if self._on_completed_fut is not None and not self._tasks:
            if not self._on_completed_fut.done():
                self._on_completed_fut.set_result(True)
        if task.cancelled():
            return
        exc = task.exception()
        if exc is None:
            return
        self._errors.append(exc)
        if self._is_base_error(exc) and self._base_error is None:
            self._base_error = exc
        if self._parent_task.done():
            self._loop.call_exception_handler({
                'message': 'Task has errored out but its parent task is already completed',
                'exception': exc,
                'task': task,
            })
            return
        if not self._aborting and not self._parent_cancel_requested:
            self._abort()
            self._parent_cancel_requested = True
            self._parent_task.cancel()
