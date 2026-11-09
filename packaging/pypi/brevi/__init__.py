"""Run AI-written Python natively and sandboxed from Python: the Brevi SDK.

    import brevi

    r = brevi.run("print(sum(range(10)))")        # sandboxed, nothing granted, default limits
    r.stdout, r.exit_code, r.path                  # '45\\n', 0, 'typed'
    r = brevi.run(code, stdin="3 4\\n", limits={"time": 5, "mem": 256}, allow_read=["data"])
    r = brevi.run(code, cases=[{"stdin": "1\\n", "expect": "2"}])   # r.ok, r.text says which case differed
    r = brevi.verify(code, inputs=["1\\n", "2\\n"])  # r.certificate: does the native build answer as CPython does?
    r.for_model()                                  # the short text to hand back to a model

It talks to one `brevi mcp` process per Session over stdio (a default Session starts on first use), so the
build cache stays warm and there is no process start per call beyond the program itself. The grants and
limits come from your code, never from the program or the model: `TOOL` is the tool definition to hand a
model, and it has no field for permissions.

Needs the Rust toolchain on PATH (https://rustup.rs), as every Brevi build does, or the published image
ghcr.io/ets-public/brevi-sandbox, which has it.
"""
import asyncio
import atexit
import json
import os
import shutil
import subprocess
import sys
import sysconfig
import threading

__all__ = ["TOOL", "Result", "BreviError", "Session", "AsyncSession", "run", "verify", "check", "handle", "binary"]

try:
    from importlib.metadata import version as _version

    __version__ = _version("brevi")
except Exception:  # not installed from the wheel (a source checkout)
    __version__ = "0"

# What the model sees. There is no field for permissions or limits on purpose: the caller sets those.
TOOL = {
    "name": "run_python",
    "description": (
        "Run a Python 3 program and get its output. It is compiled to native code, so write the plain, direct "
        "solution; do not hand-optimise it for speed. It reads `stdin` and prints to stdout. Files, the network "
        "and other processes are only available if the operator granted them; an error says what was refused."
    ),
    "input_schema": {
        "type": "object",
        "properties": {
            "code": {"type": "string", "description": "The whole program (main.py)."},
            "stdin": {"type": "string", "description": "Text passed to the program on stdin."},
        },
        "required": ["code"],
        "additionalProperties": False,
    },
}

_GRANTS = ("allow", "allow_py", "allow_net", "allow_read", "allow_write")
_PROTOCOL = "2025-06-18"


class BreviError(Exception):
    """The brevi binary is missing, would not start, or stopped answering."""


def binary():
    """The brevi executable: $BREVI_BIN, else the one this wheel installed, else brevi on PATH."""
    env = os.environ.get("BREVI_BIN")
    if env:
        return env
    exe = "brevi.exe" if os.name == "nt" else "brevi"
    schemes = [None]
    try:
        schemes.append(sysconfig.get_preferred_scheme("user"))
    except Exception:
        schemes.append(os.name + "_user")
    for scheme in schemes:
        try:
            d = sysconfig.get_path("scripts", scheme) if scheme else sysconfig.get_path("scripts")
        except Exception:
            continue
        if d and os.path.isfile(os.path.join(d, exe)):
            return os.path.join(d, exe)
    found = shutil.which("brevi")
    if found:
        return found
    raise BreviError("brevi not found: pip install brevi, or set BREVI_BIN to the brevi executable")


class Result:
    """What one call gave back.

    stdout, stderr: the program's own output (for a plain run; with cases or verify, stdout is the report).
    exit_code: the program's exit status, or None when it did not run to an exit (timeout, refused call).
    path: how it ran: "typed" or "dynamic" (native), "pybridge" (native, with CPython for a library),
        "cpython", or None when it did not build.
    limit: the limit that stopped it ("cpu", "mem", "time", "output", "file", ...), or None.
    certificate: verify's verdict, a dict such as {"verify": "identical", "sha256": ..., "speedup": ...}.
    ok: the call succeeded (exit code 0; every case passed; verify identical).
    text: the reply as the MCP tool gives it to a model.
    ms: milliseconds the call took inside brevi (plain runs).
    """

    def __init__(self, text, is_error, structured=None):
        s = structured or {}
        self.text = text
        self.ok = not is_error
        self.path = s.get("path")
        self.why = s.get("why")
        self.ms = s.get("ms")
        self.timed_out = bool(s.get("timed_out"))
        if "stdout" in s:
            self.stdout, self.stderr, self.exit_code = s["stdout"], s["stderr"], s.get("exit_code")
        else:
            self.stdout, self.stderr, self.exit_code = (text, "", None) if not is_error else ("", text, None)
        self.certificate = None
        for line in reversed(text.splitlines()):
            if line.startswith('{"verify":'):
                try:
                    self.certificate = json.loads(line)
                except ValueError:
                    pass
                break
        if self.certificate is not None:
            self.ok = self.ok and self.certificate.get("verify") == "identical"

    @property
    def limit(self):
        for line in self.stderr.splitlines() + self.text.splitlines():
            if line.startswith('{"e":"limit"'):
                try:
                    return json.loads(line).get("on")
                except ValueError:
                    return None
        return None

    def for_model(self, cap=8000):
        """What an agent reads back: the exit code, stdout and stderr, each cut to `cap` characters."""
        cut = lambda s: s if len(s) <= cap else s[:cap] + "\n[... %d more characters]" % (len(s) - cap)
        if self.exit_code is None:
            return cut(self.text)
        parts = ["exit code %d" % self.exit_code]
        if self.stdout:
            parts.append("stdout:\n" + cut(self.stdout))
        if self.stderr:
            parts.append("stderr:\n" + cut(self.stderr))
        return "\n".join(parts)

    def __repr__(self):
        return "Result(ok=%r, exit_code=%r, path=%r, stdout=%r)" % (self.ok, self.exit_code, self.path, self.stdout[:60])


def _arguments(defaults, code, extra):
    """The tools/call arguments: the session's grants and limits, then this call's, then the code."""
    a = {}
    sandbox = extra.pop("sandbox", defaults.get("sandbox", True))
    a["sandbox"] = bool(sandbox)
    for k in _GRANTS:
        v = extra.pop(k, defaults.get(k))
        if v:
            if not sandbox:
                raise ValueError("%s only applies under the sandbox; with sandbox=False everything is allowed" % k)
            a[k] = list(v)
    limits = extra.pop("limits", defaults.get("limits"))
    if limits:
        a["limits"] = dict(limits)
    timeout = extra.pop("timeout", defaults.get("timeout"))
    if timeout:
        a["timeout"] = int(timeout)
    if code is not None:
        a["code"] = code
    for k, v in extra.items():
        if v is not None:
            a[k] = v
    return a


class _Base:
    def __init__(self, sandbox=True, allow=None, allow_py=None, allow_net=None, allow_read=None,
                 allow_write=None, limits=None, timeout=None, cwd=None, brevi=None):
        """Defaults for every call of this session; each call may override them.

        sandbox: True (the default) holds the program to the grants below; False runs it with every right a
            python3 process has (and CPython for anything Brevi cannot build).
        allow: any of "read", "write", "net", "native". allow_py: Python libraries CPython may run (needs
            "native"). allow_net: hosts it may reach. allow_read / allow_write: folders, relative to cwd.
        limits: {"cpu": s, "mem": MB, "time": s, "output": KB, "file": MB, ...} on the program's run.
        timeout: seconds for build and run together (default 20, at most 600).
        cwd: the folder brevi mcp starts in (allow_read and allow_write are relative to it).
        brevi: the brevi executable (default: binary()).
        """
        self.defaults = {"sandbox": sandbox, "allow": allow, "allow_py": allow_py, "allow_net": allow_net,
                         "allow_read": allow_read, "allow_write": allow_write, "limits": limits, "timeout": timeout}
        self.cwd = cwd
        self.brevi = brevi
        self._id = 0

    def _request(self, method, params):
        self._id += 1
        return {"jsonrpc": "2.0", "id": self._id, "method": method, "params": params}

    @staticmethod
    def _result(msg):
        if "error" in msg:
            raise BreviError("brevi mcp: %s" % msg["error"].get("message"))
        r = msg["result"]
        text = "".join(c.get("text", "") for c in r.get("content", []) if c.get("type") == "text")
        return Result(text, bool(r.get("isError")), r.get("structuredContent"))

    @staticmethod
    def _run_args(defaults, code, stdin, args, cases, extra):
        extra.update({"stdin": stdin or None, "args": list(args) if args else None, "cases": cases or None})
        return _arguments(defaults, code, extra)

    @staticmethod
    def _verify_args(defaults, code, inputs, args, once, limits, timeout):
        a = {"sandbox": False, "code": code}
        if inputs:
            a["inputs"] = list(inputs)
        if args:
            a["args"] = list(args)
        if once:
            a["once"] = True
        limits = limits or defaults.get("limits")
        if limits:
            a["limits"] = dict(limits)
        timeout = timeout or defaults.get("timeout")
        if timeout:
            a["timeout"] = int(timeout)
        return a


class Session(_Base):
    """One `brevi mcp` process. Calls are taken one at a time (it is safe to share between threads)."""

    def __init__(self, **defaults):
        super().__init__(**defaults)
        self._proc = None
        self._lock = threading.Lock()

    def _start(self):
        exe = self.brevi or binary()
        try:
            self._proc = subprocess.Popen([exe, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                          stderr=subprocess.DEVNULL, cwd=self.cwd, bufsize=0)
        except OSError as e:
            raise BreviError("cannot start %s mcp: %s" % (exe, e))
        self._send(self._request("initialize", {"protocolVersion": _PROTOCOL, "capabilities": {},
                                                "clientInfo": {"name": "brevi-python-sdk", "version": __version__}}))
        self._recv()
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def _send(self, msg):
        try:
            self._proc.stdin.write((json.dumps(msg) + "\n").encode())
            self._proc.stdin.flush()
        except (OSError, ValueError) as e:
            self._proc = None
            raise BreviError("brevi mcp stopped: %s" % e)

    def _recv(self):
        while True:
            line = self._proc.stdout.readline()
            if not line:
                self._proc = None
                raise BreviError("brevi mcp stopped without answering")
            msg = json.loads(line)
            if msg.get("id") == self._id:
                return msg

    def call(self, tool, arguments):
        """Calls one MCP tool (check, run, test, verify, patch, edit, show, guide) and returns its Result."""
        with self._lock:
            if self._proc is None or self._proc.poll() is not None:
                self._start()
            self._send(self._request("tools/call", {"name": tool, "arguments": arguments}))
            return self._result(self._recv())

    def run(self, code, stdin="", args=None, cases=None, **grants):
        """Builds `code` (plain Python) natively and runs it. Keyword arguments override the session's grants,
        limits, sandbox and timeout for this call."""
        return self.call("run", self._run_args(self.defaults, code, stdin, args, cases, grants))

    def check(self, code, fast=False, **grants):
        """Builds without running: ok, or the errors as one JSON line each in .text."""
        return self.call("check", _arguments(self.defaults, code, {"fast": fast or None, **grants}))

    def verify(self, code, inputs=None, args=None, once=False, limits=None, timeout=None):
        """Runs the native build and CPython on each input and compares them; .certificate is the verdict.
        Runs CPython on the code, which no sandbox holds, so only verify code you would run with python3."""
        return self.call("verify", self._verify_args(self.defaults, code, inputs, args, once, limits, timeout))

    def handle(self, tool_input):
        """Runs a `run_python` tool call from a model (TOOL) and returns the text to send back."""
        return self.run(tool_input.get("code", ""), stdin=tool_input.get("stdin", "")).for_model()

    def close(self):
        with self._lock:
            p, self._proc = self._proc, None
        if p is not None:
            try:
                p.stdin.close()
                p.wait(timeout=10)
            except Exception:
                p.kill()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()


class AsyncSession(_Base):
    """Session for asyncio harnesses: the same calls, awaited. Calls are taken one at a time."""

    def __init__(self, **defaults):
        super().__init__(**defaults)
        self._proc = None
        self._lock = asyncio.Lock()

    async def _start(self):
        exe = self.brevi or binary()
        try:
            self._proc = await asyncio.create_subprocess_exec(exe, "mcp", stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                                              stderr=subprocess.DEVNULL, cwd=self.cwd, limit=1 << 26)
        except OSError as e:
            raise BreviError("cannot start %s mcp: %s" % (exe, e))
        await self._send(self._request("initialize", {"protocolVersion": _PROTOCOL, "capabilities": {},
                                                      "clientInfo": {"name": "brevi-python-sdk", "version": __version__}}))
        await self._recv()
        await self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    async def _send(self, msg):
        try:
            self._proc.stdin.write((json.dumps(msg) + "\n").encode())
            await self._proc.stdin.drain()
        except (OSError, ValueError) as e:
            self._proc = None
            raise BreviError("brevi mcp stopped: %s" % e)

    async def _recv(self):
        while True:
            line = await self._proc.stdout.readline()
            if not line:
                self._proc = None
                raise BreviError("brevi mcp stopped without answering")
            msg = json.loads(line)
            if msg.get("id") == self._id:
                return msg

    async def call(self, tool, arguments):
        async with self._lock:
            if self._proc is None or self._proc.returncode is not None:
                await self._start()
            await self._send(self._request("tools/call", {"name": tool, "arguments": arguments}))
            return self._result(await self._recv())

    async def run(self, code, stdin="", args=None, cases=None, **grants):
        return await self.call("run", self._run_args(self.defaults, code, stdin, args, cases, grants))

    async def check(self, code, fast=False, **grants):
        return await self.call("check", _arguments(self.defaults, code, {"fast": fast or None, **grants}))

    async def verify(self, code, inputs=None, args=None, once=False, limits=None, timeout=None):
        return await self.call("verify", self._verify_args(self.defaults, code, inputs, args, once, limits, timeout))

    async def handle(self, tool_input):
        return (await self.run(tool_input.get("code", ""), stdin=tool_input.get("stdin", ""))).for_model()

    async def close(self):
        p, self._proc = self._proc, None
        if p is not None:
            try:
                p.stdin.close()
                await asyncio.wait_for(p.wait(), 10)
            except Exception:
                p.kill()

    async def __aenter__(self):
        return self

    async def __aexit__(self, *exc):
        await self.close()


_default = None
_default_lock = threading.Lock()


def _session():
    global _default
    with _default_lock:
        if _default is None:
            _default = Session()
            atexit.register(_default.close)
        return _default


def run(code, stdin="", args=None, cases=None, **grants):
    """Session.run on the default session (sandboxed; nothing granted unless you pass it)."""
    return _session().run(code, stdin=stdin, args=args, cases=cases, **grants)


def check(code, fast=False, **grants):
    """Session.check on the default session."""
    return _session().check(code, fast=fast, **grants)


def verify(code, inputs=None, args=None, once=False, limits=None, timeout=None):
    """Session.verify on the default session."""
    return _session().verify(code, inputs=inputs, args=args, once=once, limits=limits, timeout=timeout)


def handle(tool_input):
    """Session.handle on the default session: run a model's `run_python` call, return the reply text."""
    return _session().handle(tool_input)


def _shim():
    """The `brevi-python` command the wheel installs: the brevi binary acting as python3 (see `brevi python`)."""
    exe = binary()
    args = [exe, "python"] + sys.argv[1:]
    if os.name == "nt":
        sys.exit(subprocess.call(args))
    os.execv(exe, args)
