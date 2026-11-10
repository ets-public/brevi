# Runs a server script on Brevi's mcp copy (src/dynpy/py/_brevi_mcp.py) under CPython, for quick differential
# checks before a native build: python3 tests/mcp_sdk/devrun.py SERVER.py
# The compiler's part (each tool function's __brevi_sig__, model annotations as descriptors) is done here
# from CPython's own annotations; _brevi_stdio is os-level stdio.
import inspect
import os
import select
import sys
import types
import typing

HERE = os.path.dirname(os.path.abspath(__file__))
PY = os.path.join(HERE, "..", "..", "src", "dynpy", "py")
DEV = sys.argv.pop(1) if len(sys.argv) > 2 and sys.argv[1].startswith("--dev=") else None

# Brevi's copies come before site-packages: a small directory of links to them (pydantic, the mcp files)
tmp = os.path.join(os.environ.get("TMPDIR", "/tmp"), "brevi-mcp-dev")
os.makedirs(tmp, exist_ok=True)
for f in ("pydantic.py", "_brevi_mcp.py", "_brevi_mcp_models.py", "_brevi_mcp_types.py"):
    p = os.path.join(tmp, f)
    if not os.path.exists(p):
        os.symlink(os.path.abspath(os.path.join(PY, f)), p)
sys.path.insert(0, tmp)

# _brevi_async / _brevi_stdio
ba = types.ModuleType("_brevi_async")
ba.iscoroutine = inspect.iscoroutine
sys.modules["_brevi_async"] = ba
st = types.ModuleType("_brevi_stdio")
_saved = []


def claim():
    sys.stdout.flush()
    i = os.dup(0)
    o = os.dup(1)
    n = os.open(os.devnull, os.O_RDONLY)
    os.dup2(n, 0)
    os.close(n)
    os.dup2(2, 1)
    _saved[:] = [i, o]
    return (i, o)


def release():
    i, o = _saved
    os.dup2(i, 0)
    os.dup2(o, 1)


def write(fd, b):
    while b:
        k = os.write(fd, b)
        b = b[k:]


st.claim = claim
st.release = release
st.read = os.read
st.write = write
st.poll = None
sys.modules["_brevi_stdio"] = st

# _brevi_srv: an HTTP server's sockets (non-blocking), by fd
import socket  # noqa: E402

sv = types.ModuleType("_brevi_srv")
_socks = {}


def _listen(host, port):
    s = socket.create_server((host, port))
    s.setblocking(False)
    _socks[s.fileno()] = s
    return (s.fileno(), s.getsockname()[1])


def _accept(fd):
    try:
        c, peer = _socks[fd].accept()
    except BlockingIOError:
        return None
    c.setblocking(False)
    _socks[c.fileno()] = c
    return (c.fileno(), "%s:%d" % (peer[0], peer[1]))


def _read(fd, n):
    try:
        return _socks[fd].recv(n)
    except BlockingIOError:
        return None


def _write(fd, b):
    try:
        return _socks[fd].send(b)
    except BlockingIOError:
        return 0


def _close(fd):
    s = _socks.pop(fd, None)
    if s is not None:
        s.close()


sv.listen = _listen
sv.accept = _accept
sv.read = _read
sv.write = _write
sv.close = _close
sv.poll = None
sv.random = os.urandom
sys.modules["_brevi_srv"] = sv

import pydantic  # noqa: E402  (Brevi's)

assert pydantic.__file__.startswith(tmp), pydantic.__file__


def desc(t):
    if t is inspect.Parameter.empty:
        return None
    if isinstance(t, str):
        raise SystemExit("devrun: string annotation %r" % t)
    if t is None or t is type(None):
        return ("none",)
    if t in (int, float, str, bool, bytes):
        return (t.__name__,)
    if t is typing.Any:
        return ("any", "Any")
    if t is object:
        return ("any", "object")
    if t is list:
        return ("list", ("any", "Any"), "list", True)
    if t is dict:
        return ("dict", ("any", "Any"), ("any", "Any"), "dict", True)
    if t is tuple:
        return ("tuplevar", ("any", "Any"), "tuple", True)
    o = typing.get_origin(t)
    a = typing.get_args(t)
    if o is typing.Annotated:
        return ("ann", desc(a[0]), tuple(a[1:]))
    if o in (typing.Union, types.UnionType):
        out = []
        for x in a:
            d = desc(x)
            if d[0] == "union":
                out.extend(d[1])
            else:
                out.append(d)
        return ("union", tuple(out))
    if o is typing.Literal:
        return ("literal", tuple(a))
    if o is list:
        return ("list", desc(a[0]), "list", False)
    if o is set:
        return ("set", desc(a[0]), "set", False)
    if o is dict:
        return ("dict", desc(a[0]), desc(a[1]), "dict", False)
    if o is tuple:
        if len(a) == 2 and a[1] is Ellipsis:
            return ("tuplevar", desc(a[0]), "tuple", False)
        return ("tuple", tuple(desc(x) for x in a), "tuple")
    if o is not None and isinstance(o, type):
        return ("cls", o)
    if isinstance(t, type):
        return ("cls", t)
    raise SystemExit("devrun: annotation %r" % (t,))


def ann_hook(cls):
    a = cls.__dict__.get("__annotations__")
    if a is None:
        a = getattr(cls, "__annotate__", None)
        a = a(1) if callable(a) and cls.__dict__.get("__annotate__") else None
    if a is None:
        return ()
    if isinstance(a, tuple):
        return a
    return tuple((k, desc(v)) for k, v in a.items())


pydantic._ann_hook = ann_hook

import _brevi_mcp as M  # noqa: E402

_orig_sig = M._sig


def sig(fn):
    try:
        return _orig_sig(fn)
    except M.InvalidSignature:
        pass
    f = fn
    bound = False
    if inspect.ismethod(f):
        bound = True
    s = inspect.signature(f)
    hints = typing.get_type_hints(f, include_extras=True)
    params = []
    kinds = {inspect.Parameter.POSITIONAL_ONLY: 0, inspect.Parameter.POSITIONAL_OR_KEYWORD: 0, inspect.Parameter.VAR_POSITIONAL: 1,
             inspect.Parameter.KEYWORD_ONLY: 2, inspect.Parameter.VAR_KEYWORD: 3}
    for p in s.parameters.values():
        d = desc(hints[p.name]) if p.name in hints else None
        params.append((p.name, d, M._NO if p.default is inspect.Parameter.empty else p.default, kinds[p.kind]))
    has_ret = "return" in hints
    ret = desc(hints["return"]) if has_ret else None
    return (params, ret, has_ret, inspect.iscoroutinefunction(f), f.__doc__)


M._sig = sig


async def _wait(fd):
    import asyncio

    loop = asyncio.get_running_loop()
    fut = loop.create_future()

    def ready():
        loop.remove_reader(fd)
        if not fut.done():
            fut.set_result(1)

    loop.add_reader(fd, ready)
    return await fut


M._stdio_wait = _wait


async def _srv_wait(fd, events):
    import asyncio

    loop = asyncio.get_running_loop()
    fut = loop.create_future()
    add, rm = (loop.add_reader, loop.remove_reader) if events == 1 else (loop.add_writer, loop.remove_writer)

    def ready():
        rm(fd)
        if not fut.done():
            fut.set_result(events)

    add(fd, ready)
    try:
        return await fut
    finally:
        rm(fd)


M._srv_wait = _srv_wait

# the SDK's module names, all as Brevi's copy
for name in ("mcp", "mcp.types", "mcp_types", "mcp.server", "mcp.server.mcpserver", "mcp.server.lowlevel", "mcp.server.stdio",
             "mcp.shared", "mcp.shared.exceptions", "mcp.server.mcpserver.exceptions", "mcp.server.mcpserver.prompts",
             "mcp.server.mcpserver.prompts.base", "mcp.server.mcpserver.utilities", "mcp.server.mcpserver.utilities.types",
             "mcp.server.mcpserver.resources", "mcp.server.mcpserver.tools", "mcp.server.transport_security", "mcp.server.subscriptions",
             "mcp.shared.subscriptions"):
    m = types.ModuleType(name)
    m.__dict__.update({k: v for k, v in vars(M).items() if not k.startswith("__")})
    m.__dict__.update({k: v for k, v in vars(M.T).items() if not k.startswith("__") and name in ("mcp.types", "mcp_types", "mcp")})
    m.__path__ = []
    sys.modules[name] = m

script = sys.argv[1]
sys.argv = sys.argv[1:]
code = compile(open(script).read(), script, "exec")
g = {"__name__": "__main__", "__file__": script}
exec(code, g)
