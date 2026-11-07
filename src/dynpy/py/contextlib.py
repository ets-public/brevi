# Brevi's copy of contextlib (CPython 3.13's, the parts programs use most): contextmanager, asynccontextmanager,
# suppress, nullcontext, closing, aclosing, ExitStack and AsyncExitStack. What is not here (redirect_stdout,
# chdir, ContextDecorator, ...) is refused when a program is built (src/caps.rs), and CPython runs it.
import sys


class AbstractContextManager:
    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        return None


class AbstractAsyncContextManager:
    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc_value, traceback):
        return None


class _GeneratorContextManager(AbstractContextManager):
    def __init__(self, func, args, kwds):
        self.gen = func(*args, **kwds)
        self.func, self.args, self.kwds = func, args, kwds
        doc = getattr(func, "__doc__", None)
        self.__doc__ = doc

    def __call__(self, func):
        def inner(*args, **kwds):
            with self.func(*self.args, **self.kwds):
                return func(*args, **kwds)
        return inner

    def __enter__(self):
        del self.args, self.kwds, self.func
        try:
            return next(self.gen)
        except StopIteration:
            raise RuntimeError("generator didn't yield") from None

    def __exit__(self, typ, value, traceback):
        if typ is None:
            try:
                next(self.gen)
            except StopIteration:
                return False
            else:
                try:
                    raise RuntimeError("generator didn't stop")
                finally:
                    self.gen.close()
        else:
            if value is None:
                value = typ()
            try:
                self.gen.throw(value)
            except StopIteration as exc:
                return exc is not value
            except RuntimeError as exc:
                if exc is value:
                    exc.__traceback__ = traceback
                    return False
                if isinstance(value, StopIteration) and exc.__cause__ is value:
                    value.__traceback__ = traceback
                    return False
                raise
            except BaseException as exc:
                if exc is not value:
                    raise
                exc.__traceback__ = traceback
                return False
            try:
                raise RuntimeError("generator didn't stop after throw()")
            finally:
                self.gen.close()


class _AsyncGeneratorContextManager(AbstractAsyncContextManager):
    def __init__(self, func, args, kwds):
        self.gen = func(*args, **kwds)
        self.func, self.args, self.kwds = func, args, kwds
        self.__doc__ = getattr(func, "__doc__", None)

    async def __aenter__(self):
        del self.args, self.kwds, self.func
        try:
            return await self.gen.__anext__()
        except StopAsyncIteration:
            raise RuntimeError("generator didn't yield") from None

    async def __aexit__(self, typ, value, traceback):
        if typ is None:
            try:
                await self.gen.__anext__()
            except StopAsyncIteration:
                return False
            else:
                try:
                    raise RuntimeError("generator didn't stop")
                finally:
                    await self.gen.aclose()
        else:
            if value is None:
                value = typ()
            try:
                await self.gen.athrow(value)
            except StopAsyncIteration as exc:
                return exc is not value
            except RuntimeError as exc:
                if exc is value:
                    exc.__traceback__ = traceback
                    return False
                if isinstance(value, (StopIteration, StopAsyncIteration)) and exc.__cause__ is value:
                    value.__traceback__ = traceback
                    return False
                raise
            except BaseException as exc:
                if exc is not value:
                    raise
                exc.__traceback__ = traceback
                return False
            try:
                raise RuntimeError("generator didn't stop after athrow()")
            finally:
                await self.gen.aclose()


def contextmanager(func):
    def helper(*args, **kwds):
        return _GeneratorContextManager(func, args, kwds)
    helper.__name__ = getattr(func, "__name__", "helper")
    helper.__doc__ = getattr(func, "__doc__", None)
    helper.__wrapped__ = func
    return helper


def asynccontextmanager(func):
    def helper(*args, **kwds):
        return _AsyncGeneratorContextManager(func, args, kwds)
    helper.__name__ = getattr(func, "__name__", "helper")
    helper.__doc__ = getattr(func, "__doc__", None)
    helper.__wrapped__ = func
    return helper


class closing(AbstractContextManager):
    def __init__(self, thing):
        self.thing = thing

    def __enter__(self):
        return self.thing

    def __exit__(self, *exc_info):
        self.thing.close()


class aclosing(AbstractAsyncContextManager):
    def __init__(self, thing):
        self.thing = thing

    async def __aenter__(self):
        return self.thing

    async def __aexit__(self, *exc_info):
        await self.thing.aclose()


class suppress(AbstractContextManager):
    def __init__(self, *exceptions):
        self._exceptions = exceptions

    def __enter__(self):
        pass

    def __exit__(self, exctype, excinst, exctb):
        if exctype is None:
            return False
        if issubclass(exctype, self._exceptions):
            return True
        return False


class nullcontext(AbstractContextManager, AbstractAsyncContextManager):
    def __init__(self, enter_result=None):
        self.enter_result = enter_result

    def __enter__(self):
        return self.enter_result

    def __exit__(self, *excinfo):
        pass

    async def __aenter__(self):
        return self.enter_result

    async def __aexit__(self, *excinfo):
        pass


class _BaseExitStack:
    def __init__(self):
        self._exit_callbacks = []

    def pop_all(self):
        new_stack = type(self)()
        new_stack._exit_callbacks = self._exit_callbacks
        self._exit_callbacks = []
        return new_stack

    def push(self, exit):
        _cb_type = type(exit)
        try:
            exit_method = _cb_type.__exit__
        except AttributeError:
            self._exit_callbacks.append((True, exit))
        else:
            self._exit_callbacks.append((True, lambda *a: exit_method(exit, *a)))
        return exit

    def enter_context(self, cm):
        cls = type(cm)
        try:
            _enter = cls.__enter__
            _exit = cls.__exit__
        except AttributeError:
            raise TypeError("'%s' object does not support the context manager protocol" % cls.__name__) from None
        result = _enter(cm)
        self._exit_callbacks.append((True, lambda *a: _exit(cm, *a)))
        return result

    def callback(self, callback, *args, **kwds):
        def _exit_wrapper(exc_type, exc, tb):
            callback(*args, **kwds)
        _exit_wrapper.__wrapped__ = callback
        self._exit_callbacks.append((True, _exit_wrapper))
        return callback


class ExitStack(_BaseExitStack, AbstractContextManager):
    def __enter__(self):
        return self

    def __exit__(self, *exc_details):
        exc = exc_details[1]
        received_exc = exc is not None
        suppressed_exc = False
        pending_raise = False
        while self._exit_callbacks:
            is_sync, cb = self._exit_callbacks.pop()
            try:
                if exc is None:
                    exc_details = None, None, None
                else:
                    exc_details = type(exc), exc, exc.__traceback__
                if cb(*exc_details):
                    suppressed_exc = True
                    pending_raise = False
                    exc = None
            except BaseException as new_exc:
                if exc is not None and new_exc is not exc and new_exc.__context__ is None:
                    new_exc.__context__ = exc
                pending_raise = True
                exc = new_exc
        if pending_raise:
            raise exc
        return received_exc and suppressed_exc

    def close(self):
        self.__exit__(None, None, None)


class AsyncExitStack(_BaseExitStack, AbstractAsyncContextManager):
    async def enter_async_context(self, cm):
        cls = type(cm)
        try:
            _enter = cls.__aenter__
            _exit = cls.__aexit__
        except AttributeError:
            raise TypeError("'%s' object does not support the asynchronous context manager protocol" % cls.__name__) from None
        result = await _enter(cm)
        self._exit_callbacks.append((False, lambda *a: _exit(cm, *a)))
        return result

    def push_async_exit(self, exit):
        _cb_type = type(exit)
        try:
            exit_method = _cb_type.__aexit__
        except AttributeError:
            self._exit_callbacks.append((False, exit))
        else:
            self._exit_callbacks.append((False, lambda *a: exit_method(exit, *a)))
        return exit

    def push_async_callback(self, callback, *args, **kwds):
        async def _exit_wrapper(exc_type, exc, tb):
            await callback(*args, **kwds)
        self._exit_callbacks.append((False, _exit_wrapper))
        return callback

    async def aclose(self):
        await self.__aexit__(None, None, None)

    async def __aenter__(self):
        return self

    async def __aexit__(self, *exc_details):
        exc = exc_details[1]
        received_exc = exc is not None
        suppressed_exc = False
        pending_raise = False
        while self._exit_callbacks:
            is_sync, cb = self._exit_callbacks.pop()
            try:
                if exc is None:
                    exc_details = None, None, None
                else:
                    exc_details = type(exc), exc, exc.__traceback__
                if is_sync:
                    cb_suppress = cb(*exc_details)
                else:
                    cb_suppress = await cb(*exc_details)
                if cb_suppress:
                    suppressed_exc = True
                    pending_raise = False
                    exc = None
            except BaseException as new_exc:
                if exc is not None and new_exc is not exc and new_exc.__context__ is None:
                    new_exc.__context__ = exc
                pending_raise = True
                exc = new_exc
        if pending_raise:
            raise exc
        return received_exc and suppressed_exc
