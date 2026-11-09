"""Checks `brevi mcp`: plain Python by default, the kept program, patch and edit with a check, show, run
with cases, for Python and for Brevi code; then the protocol (versions, bad input, errors), argument checks,
long output, timeouts, cancel, exit on EOF, the memory limit and the caller's resource limits.
usage: python3 tests/mcp_test.py [BREVI]"""
import json, subprocess, sys

B = sys.argv[1] if len(sys.argv) > 1 else "target/release/brevi"
n = 0
p = None

def start():
    global p
    if p:
        p.stdin.close()
        p.wait()
    p = subprocess.Popen([B, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)

def rpc(method, params):
    global n
    n += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": n, "method": method, "params": params}) + "\n")
    p.stdin.flush()
    return json.loads(p.stdout.readline())["result"]

def call(tool, **args):
    r = rpc("tools/call", {"name": tool, "arguments": args})
    if "ax_" in r["content"][0]["text"]:
        print(f"FAIL {tool}: internal name in {r['content'][0]['text']!r}")
        global fails
        fails += 1
    return r["content"][0]["text"], r["isError"]

fails = 0
def expect(label, got, want_in, err=None):
    global fails
    text, is_err = got
    ok = want_in in text and (err is None or is_err == err)
    if not ok:
        fails += 1
        print(f"FAIL {label}: want {want_in!r} (error={err}), got {text!r} (error={is_err})")

# Plain Python, the default.
start()
init = rpc("initialize", {"protocolVersion": "2025-06-18"})["instructions"]
if "plain Python" not in init or "guide once" in init or "hand-optimise" not in init:
    fails += 1
    print(f"FAIL instructions: {init!r}")
expect("named main.py on a fresh server", call("check", file="main.py", code="print(1)\n"), "OK", False)
start()
dog = "class Dog:\n    def __init__(self, name):\n        self.name = name\n\n    def speak(self):\n        return self.name + ' says woof'\n\nit = Dog('Rex')\nprint(type(it).__name__)\nprint(it.speak())\n"
expect("python class", call("run", code=dog), "Dog\nRex says woof", False)
expect("python check", call("check"), "OK", False)
expect("python patch", call("patch", patch="def helper():\n    return 2\n"), "added def helper\nOK", False)
expect("undefined name", call("check", code="nums = [1, 2, 3]\ntotal = sum(nums)\nprint(totl(nums))\n"), "name 'totl' is not defined", True)
expect("undefined name hint", call("check"), "did you mean total?", True)
expect("undefined name run", call("run"), "name 'totl' is not defined", True)
expect("python patch fixes it", call("edit", **{"from": 3, "to": 3, "text": "print(total)"}), "OK", False)
expect("python run", call("run"), "6", False)
expect("unknown method", call("check", code="x = 5\nprint(x.uppr())\n"), "did you mean .upper()?", True)
expect("text plus number", call("check", code='nums = [1]\nprint("n=" + len(nums))\n'), "cannot add str and int", True)
expect("python reset", call("check", code="print(1)\n"), "OK", False)
expect("python module", call("check", file="util.py", code="def inc(x):\n    return x + 1\n"), "OK", None)
expect("python import", call("run", code="from util import inc\nprint(inc(41))\n"), "42", False)
expect("python def patch", call("patch", file="util.py", patch="def inc(x):\n    return x + 2\n"), "replaced def inc", None)
expect("python cases", call("run", cases=[{"stdin": "", "expect": "43\n"}]), "passed 1/1", False)
# verify: the native build against CPython. CPython cannot be sandboxed, so the default sandbox refuses it.
start()
sq = "import sys\nn = int(sys.stdin.read() or 3)\nprint(sum(i * i for i in range(n)))\n"
expect("verify needs sandbox false", call("verify", code=sq), "sandbox false", True)
expect("verify identical", call("verify", sandbox=False, inputs=["5", "100"], args=["x"]), '"verify":"identical","file":"main.py"', False)
expect("verify names inputs", call("verify", sandbox=False, inputs=["5"]), '"input":"input1"', False)
expect("verify unstable", call("verify", sandbox=False, code="import time\nprint(time.time_ns())\n"), "pass once true", True)
expect("verify not built", call("verify", sandbox=False, code='exec("print(1)")\n'), '"verify":"not built"', True)
expect("verify Brevi code", call("verify", sandbox=False, code="fn main():\n    print(1)\n"), "no CPython answer", True)
# Verify by default: run with sandbox false checks plain Python against CPython (docs/VERIFY_BY_DEFAULT.md).
start()
sq = "import sys\nn = int(sys.stdin.read())\nprint(sum(i * i for i in range(n)))\n"
expect("run verifies by default", call("run", code=sq, sandbox=False, stdin="5"), '30\n{"verify":"identical","brevi_ms":', False)
expect("run verify false", call("run", sandbox=False, stdin="5", verify=False), "30", False)
if '"verify"' in call("run", sandbox=False, stdin="5", verify=False)[0] or '"verify"' in call("run", stdin="5")[0]:
    fails += 1
    print("FAIL run: verify false and the sandbox must not verify")
expect("run verify refuses the clock", call("run", sandbox=False, code="import time\nprint(time.time() > 0)\n"), '{"verify":"refused","why":"reads the clock"}', False)
expect("run verify refuses writes", call("run", sandbox=False, code="open('x.txt', 'w').write('x')\nprint(1)\n"), '"why":"writes files"', False)
def start_with(**env):
    global p
    if p:
        p.stdin.close()
        p.wait()
    p = subprocess.Popen([B, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=dict(os.environ, **env))
import os
start_with(BREVI_PYTHON=os.path.abspath("tests/verify/fakepy.sh"))
d = call("run", code=sq, sandbox=False, stdin="5")
expect("run verify differs gives CPython's answer", d, '30\none more line\n{"verify":"differs","on":"stdout","line":2', False)
expect("run verify differs says whose bug", d, "runs with CPython for the rest of the session", False)
expect("run after differs uses CPython", call("run", sandbox=False, stdin="5"), '"path":"cpython","why":"Brevi gave a wrong answer', False)
start_with(BREVI_PYTHON=os.path.abspath("tests/verify/upperpy.sh"))
expect("cases Brevi bug", call("run", code="print(input())\n", sandbox=False, cases=[{"stdin": "x\n", "expect": "X\n"}]), "this is a Brevi bug, not your code", True)
expect("cases after Brevi bug use CPython", call("run", sandbox=False, cases=[{"stdin": "x\n", "expect": "X\n"}]), "passed 1/1", False)
start()
expect("cases sandbox note", call("run", code="print(input())\n", cases=[{"stdin": "x\n", "expect": "X\n"}]), "not checked against CPython", True)
# Every build says which path it took: structuredContent always, the text's last line only off the typed path.
start()
def path_of(tool, **args):
    r = rpc("tools/call", {"name": tool, "arguments": args})
    return r["content"][0]["text"], r.get("structuredContent")
def expect_path(label, got, path, text_has=None, fn=None):
    global fails
    text, sc = got
    ok = isinstance(sc, dict) and sc.get("path") == path and (fn is None or sc.get("fn") == fn)
    ok = ok and (text_has is None or text_has in text) and (('{"path":' in text) == (path != "typed"))
    if not ok:
        fails += 1
        print(f"FAIL {label}: want path {path} fn {fn}, got {sc!r} with text {text!r}")
expect_path("path typed check", path_of("check", code="print(sum(range(4)))\n"), "typed", "OK")
expect_path("path typed run", path_of("run"), "typed", "6")
expect_path("path dynamic", path_of("run", code="def show(xs):\n    print(set(xs))\n\nshow([2, 'a'])\n"), "dynamic", None, "show")
expect_path("path dynamic check", path_of("check"), "dynamic", "OK\n", "show")
expect_path("path cpython", path_of("run", code="def g():\n    return eval('1 + 2')\n\nprint(g())\n", sandbox=False), "cpython", "3", "g")
expect_path("path pybridge", path_of("run", code="import colorsys\nprint(colorsys.rgb_to_hsv(1.0, 0.0, 0.0)[0])\n", sandbox=False), "pybridge", "0.0")
expect_path("path cases", path_of("run", code="print(input())\n", cases=[{"stdin": "x\n", "expect": "x\n"}]), "typed", "passed 1/1")
r = rpc("tools/call", {"name": "check", "arguments": {"code": "print(totl)\n"}})
if "structuredContent" in r:
    fails += 1
    print(f"FAIL path on a failed build: {r!r}")

# The default sandbox holds Python too: a file read, a Python library or a write is refused, never run.
start()
expect("sandbox read", call("run", code="with open('/etc/hostname') as f:\n    print(f.read().strip())\n"), "needs allow read", True)
expect("sandbox library", call("run", code="import os\nprint(os.getpid())\n"), "needs allow native", True)
# Brevi's own os.listdir reads a folder: it needs allow read, not CPython
expect("sandbox listdir", call("run", code="import os\nprint(os.listdir('.'))\n"), "needs allow read", True)
esc = call("run", code="import sys\nsys.modules['os'].system('echo escaped')\n")
if "escaped" in esc[0].replace("echo escaped", "") or not esc[1]:
    fails += 1
    print(f"FAIL sandbox sys.modules: {esc!r}")
expect("sandbox write", call("run", code="open('x.txt', 'w').write('x')\n"), "needs allow write", True)
# Path text and seeded random need nothing: Brevi's own os.path, PurePath and random run in the sandbox.
expect("sandbox pure paths", call("run", code="import os\nfrom pathlib import PurePath\nprint(os.path.join('a', 'b.txt'), PurePath('/x') / 'y.py')\n"), "a/b.txt /x/y.py", False)
expect("sandbox random", call("run", code="import random\nrandom.seed(1)\nprint(random.randint(1, 6))\n"), "2", False)
expect("sandbox random one error", call("run", code="import random\nprint(random.betavariate(1, 2))\n"), "random.betavariate needs allow native", True)
# The code cannot grant itself anything: only the caller's allow / allow_py arguments do.
expect("sandbox comment allow refused", call("run", code="# allow read\nwith open('/etc/hostname') as f:\n    print(len(f.read()) > 0)\n"), "grants nothing under --sandbox", True)
expect("sandbox allow argument", call("run", code="with open('/etc/hostname') as f:\n    print(len(f.read()) > 0)\n", allow=["read"]), "True", False)
expect("sandbox allow read not write", call("run", code="open('x.txt', 'w').write('x')\n", allow=["read"]), "needs allow write", True)
expect("sandbox allow_py", call("run", code="import os\nprint(os.getpid() > 0)\n", allow_py=["os"]), "True", False)
# CPython is held to the file grants by the kernel (Linux Landlock): without allow read it lists nothing.
expect("sandbox allow_py held to grants", call("run", code="import os\nprint(len(list(os.scandir('.'))) >= 0)\n", allow_py=["os"]), "Permission denied", True)
expect("sandbox allow_py with read", call("run", code="import os\nprint(len(list(os.scandir('.'))) >= 0)\n", allow_py=["os"], allow=["read"]), "True", False)
expect("sandbox allow_py other", call("run", code="import os\nprint(os.getpid())\n", allow_py=["json"]), "allow_py", True)
# allow_read / allow_write: files only inside the granted folders (relative to where brevi mcp started)
expect("sandbox allow_read folder", call("run", code="with open('/etc/hostname') as f:\n    print(f.read())\n", allow_read=["tests/sandbox"]), "outside the folders granted to read", True)
expect("sandbox allow_read inside", call("run", code="with open(" + repr(__import__('os').path.abspath('tests/sandbox/data.txt')) + ") as f:\n    print(len(f.read()) > 0)\n", allow_read=["tests/sandbox"]), "True", False)
expect("sandbox allow_read missing folder", call("run", code="print(1)\n", allow_read=["no/such/folder"]), "allow_read no/such/folder", True)
expect("sandbox bad allow", call("run", code="print(1)\n", allow=["network"]), "allow takes read, write, net or native", True)
# requests and httpx are Brevi's own: they need the network granted (allow net, or allow_net hosts), not native.
expect("sandbox net refused", call("run", code="import httpx\nprint(1)\n"), "needs allow net", True)
expect("sandbox bad allow_net", call("run", code="print(1)\n", allow_net=["http://x.com"]), "takes host names", True)
expect("sandbox off", call("run", code="with open('/etc/hostname') as f:\n    print(len(f.read()) > 0)\n", sandbox=False), "True", False)
# Resource limits: the caller's limits stop the program at the first one it reaches, and the reply names it.
spin = "n = 0\nwhile True:\n    n += 1\n"
expect("limits cpu", call("run", code=spin, limits={"cpu": 1}), '"on":"cpu"', True)
expect("limits time", call("run", code=spin, limits={"time": 0.5}), '"on":"time"', True)
expect("limits advice", call("run", code=spin, limits={"time": 0.5}), "raise it in limits: time=", True)
loud = "i = 0\nwhile True:\n    print('line', i)\n    i += 1\n"
expect("limits output", call("run", code=loud, limits={"output": 1}), '"on":"output"', True)
expect("limits output in cases", call("run", code=loud, limits={"output": 1}, cases=[{"stdin": "", "expect": "x"}]), '"on":"output"', True)
expect("limits cpu in cases", call("run", code=spin, limits={"cpu": 1}, cases=[{"stdin": "", "expect": "x"}]), '"on":"cpu"', True)
expect("limits pass", call("run", code="print(sum(range(10)))\n", limits={"cpu": 2, "time": 5, "output": 64}), "45", False)
expect("limits pass in cases", call("run", limits={"cpu": 2, "time": 5}, cases=[{"stdin": "", "expect": "45\n"}]), "passed 1/1", False)
expect("limits test", call("test", code="fn main():\n  print(1)\n\ntest \"spins\":\n  var n = 0\n  while true:\n    n = n + 1\n", limits={"time": 1}), '"on":"time"', True)
expect("limits bad name", call("run", code="print(1)\n", limits={"cpus": 1}), "limits has \"cpus\"", True)
expect("limits bad value", call("run", code="print(1)\n", limits={"cpu": -1}), "limits cpu must be a number above 0", True)
expect("limits verify", call("verify", sandbox=False, code=spin, limits={"cpu": 1}), '"verify":"limit"', True)

# Brevi code, recognised by its fn items.
start()
expect("no program", call("check"), "no program yet", True)
prog = "fn double(x: int) -> int = x * 3\n\nfor l in lines():\n  print(double(l.int()))\n"
expect("check keeps code", call("check", code=prog), "OK", False)
expect("cases fail", call("run", cases=[{"stdin": "2\n", "expect": "4\n"}, {"stdin": "0\n", "expect": "0\n"}]), 'case 1: line 1 want "4" got "6"', True)
expect("patch then check", call("patch", patch="fn double(x: int) -> int = x * 2\n"), "replaced fn double\nOK", False)
expect("cases pass", call("run", cases=[{"stdin": "2\n3\n", "expect": "4\n6\n"}]), "passed 1/1", False)
expect("edit with error", call("edit", **{"from": 4, "to": 4, "text": "  print(doubel(l.int()))"}), "lines 4..4 now 1 line", None)
expect("show", call("show", **{"from": 4, "to": 4}), "4   print(", False)
expect("edit fix", call("edit", **{"from": 4, "to": 4, "text": "  print(double(l.int()) + 1)"}), "OK", False)
expect("run stdin", call("run", stdin="5\n"), "11", False)
expect("edit range", call("edit", **{"from": 9, "to": 9, "text": "x"}), "outside", True)
expect("other file", call("check", file="util.bv", code="fn inc(x: int) -> int = x + 1\n"), "OK", False)
expect("use other file", call("edit", **{"from": 1, "to": 0, "text": 'use "util.bv"'}), "OK", False)
expect("old style still works", call("run", code='print("hi")\n'), "hi", False)
p.stdin.close()
p.wait()

# Protocol and robustness: the stdio transport must survive anything a client sends, and every call must end.
import os, time
def raw_start(*flags):
    return subprocess.Popen([B, "mcp", *flags], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
def raw_send(q, data):
    q.stdin.write((data if isinstance(data, bytes) else json.dumps(data).encode()) + b"\n")
    q.stdin.flush()
def raw_recv(q):
    line = q.stdout.readline()
    return json.loads(line) if line else None
def tool(i, name, **args):
    return {"jsonrpc": "2.0", "id": i, "method": "tools/call", "params": {"name": name, "arguments": args}}
def text_of(r):
    return r["result"]["content"][0]["text"]
def want(label, ok, got):
    global fails
    if not ok:
        fails += 1
        print(f"FAIL {label}: got {str(got)[:300]!r}")
def procs_in(d):
    # Processes whose working directory is d (Linux): what a stopped call must not leave behind.
    n = 0
    for pid in os.listdir("/proc") if os.path.isdir("/proc") else []:
        try:
            if pid.isdigit() and os.readlink(f"/proc/{pid}/cwd") == d and open(f"/proc/{pid}/stat").read().split(") ")[1][0] != "Z":
                n += 1
        except OSError:
            pass
    return n

q = raw_start()
work = os.path.realpath(os.path.join(__import__("tempfile").gettempdir(), f"brevi-mcp-{q.pid}"))
raw_send(q, {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}})
r = raw_recv(q)["result"]
want("version echoed", r["protocolVersion"] == "2025-06-18", r)
raw_send(q, {"jsonrpc": "2.0", "method": "notifications/initialized"})
raw_send(q, {"jsonrpc": "2.0", "id": 2, "method": "initialize", "params": {"protocolVersion": "1999-01-01"}})
want("unknown version gets the newest", raw_recv(q)["result"]["protocolVersion"] == "2025-11-25", "")
# 2026-07-28: no handshake; each request carries its envelope, and results name the server (as mcp 2.3.0 does)
env = {"io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientInfo": {"name": "t", "version": "1"},
       "io.modelcontextprotocol/clientCapabilities": {}}
raw_send(q, {"jsonrpc": "2.0", "id": 20, "method": "server/discover", "params": {"_meta": env}})
r = raw_recv(q)["result"]
want("discover", r["supportedVersions"] == ["2026-07-28"] and r["resultType"] == "complete" and r["cacheScope"] == "private" and r["ttlMs"] == 0
     and r["capabilities"] == {"tools": {"listChanged": False}} and "plain Python" in r["instructions"]
     and r["_meta"]["io.modelcontextprotocol/serverInfo"]["name"] == "brevi", r)
raw_send(q, {"jsonrpc": "2.0", "id": 21, "method": "tools/list", "params": {"_meta": env}})
r = raw_recv(q)["result"]
want("modern tools/list", len(r["tools"]) == 8 and r["resultType"] == "complete" and "ttlMs" in r and "_meta" in r, r)
raw_send(q, {"jsonrpc": "2.0", "id": 22, "method": "tools/list", "params": {"_meta": dict(env, **{"io.modelcontextprotocol/protocolVersion": "2099-01-01"})}})
r = raw_recv(q)["error"]
want("modern unknown version", r == {"code": -32022, "message": "Unsupported protocol version", "data": {"supported": ["2026-07-28"], "requested": "2099-01-01"}}, r)
raw_send(q, {"jsonrpc": "2.0", "id": 23, "method": "tools/list", "params": {"_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28"}}})
r = raw_recv(q)["error"]
want("modern envelope without capabilities", r["code"] == -32602 and "clientCapabilities" in r["message"], r)
raw_send(q, {"jsonrpc": "2.0", "id": 24, "method": "server/discover"})
want("discover without envelope", raw_recv(q)["error"]["code"] == -32602, "")
raw_send(q, {"jsonrpc": "2.0", "id": 25, "method": "tools/call", "params": {"name": "run", "arguments": {"code": "print(6 * 7)\n"}, "_meta": env}})
r = raw_recv(q)
want("modern tools/call", "42" in text_of(r) and r["result"]["resultType"] == "complete" and "_meta" in r["result"], r)
raw_send(q, {"jsonrpc": "2.0", "id": "s", "method": "tools/list"})
r = raw_recv(q)
want("notification has no reply", r["id"] == "s", r)
tools = r["result"]["tools"]
want("eight tools with closed schemas", len(tools) == 8 and all(t["inputSchema"].get("additionalProperties") is False for t in tools), tools)
for bad, code in [(b"{not json", -32700), (b"\xff\xfe", -32700), (b"[" * 100000, -32700), (b'[{"jsonrpc":"2.0","id":9,"method":"ping"}]', -32600), (b"42", -32600)]:
    raw_send(q, bad)
    r = raw_recv(q)
    want(f"bad input {bad[:12]!r}", r and r["id"] is None and r["error"]["code"] == code, r)
raw_send(q, {"jsonrpc": "2.0", "id": 3, "method": "no/such"})
want("unknown method", raw_recv(q)["error"]["code"] == -32601, "")
raw_send(q, tool(4, "nope"))
want("unknown tool", raw_recv(q)["error"]["code"] == -32602, "")
raw_send(q, tool(5, "run", stdn="x"))
r = raw_recv(q)
want("misspelt argument", r["result"]["isError"] and "stdin" in text_of(r), r)
raw_send(q, tool(6, "run", code="print(1)\n", timeout="abc"))
want("mistyped argument", "whole number" in text_of(raw_recv(q)), "")
raw_send(q, tool(7, "check", file="../escape.py", code="x = 1\n"))
want("bad file name", "not allowed" in text_of(raw_recv(q)), "")
raw_send(q, tool(8, "run", code="print('h\u00e9llo \U0001F600')\n", stdin=None))
r = raw_recv(q)
want("bad file name not kept; unicode both ways", text_of(r) == "h\u00e9llo \U0001F600\n" and not r["result"]["isError"], r)
raw_send(q, tool(9, "edit", **{"from": "1", "to": 1, "text": "print('\\x41\\u00e9')"}))
raw_send(q, tool(10, "run"))
raw_recv(q)
want("numbers as text; escapes", text_of(raw_recv(q)) == "A\u00e9\n", "")
raw_send(q, tool(11, "run", code="for i in range(40000):\n    print('line', i)\n"))
r = raw_recv(q)
t = text_of(r)
want("long output is cut, not stuck", not r["result"]["isError"] and "cut" in t and t.rstrip().endswith("line 39999") and len(t) < 30000, t[-200:])
# A program that never ends: stopped at its timeout with everything it started; the server stays usable.
t0 = time.time()
raw_send(q, tool(12, "run", code="i = 0\nwhile True:\n    i += 1\n", timeout=2))
r = raw_recv(q)
want("timeout", "timeout" in text_of(r) and time.time() - t0 < 15, r)
time.sleep(0.3)
want("nothing left running after a timeout", procs_in(work) == 0, procs_in(work))
# Cancel: ping is answered while a call runs, the cancelled call gets no reply, the next call works.
raw_send(q, tool(13, "run", code="while True:\n    pass\n", timeout=60))
time.sleep(1.0)
t0 = time.time()
raw_send(q, {"jsonrpc": "2.0", "id": 14, "method": "ping"})
r = raw_recv(q)
want("ping during a call", r["id"] == 14 and time.time() - t0 < 1, r)
raw_send(q, {"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 13}})
raw_send(q, tool(15, "run", code="print(7)\n"))
r = raw_recv(q)
want("cancelled call has no reply; next call works", r["id"] == 15 and text_of(r) == "7\n", r)
want("nothing left running after a cancel", procs_in(work) == 0, procs_in(work))
raw_send(q, tool(16, "run", code="import sys\nprint(eval(sys.stdin.read()))\n", sandbox=False, cases=[{"stdin": "6*7", "expect": 42}]))
want("cases for Python only CPython runs", "passed 1/1" in text_of(raw_recv(q)), "")
# The client goes away mid-call: the server stops the program, removes its folder and exits.
raw_send(q, tool(17, "run", code="while True:\n    pass\n", timeout=60))
time.sleep(1.0)
q.stdin.close()
t0 = time.time()
try:
    q.wait(10)
except subprocess.TimeoutExpired:
    q.kill()
want("exit on EOF", q.returncode == 0 and time.time() - t0 < 5, q.returncode)
want("work folder removed", not os.path.exists(work), work)
want("nothing left after exit", procs_in(work) == 0, procs_in(work))
want("stdout only JSON-RPC, stderr quiet", q.stdout.read() == b"" and q.stderr.read() == b"", "")
if sys.platform.startswith("linux"):
    q = raw_start("--mem-mb", "300")
    raw_send(q, tool(1, "run", code="xs = []\nwhile True:\n    xs.append([0] * 1000000)\n", timeout=60))
    t = text_of(raw_recv(q))
    want("memory limit", "memory" in t or "signal" in t, t[-300:])
    q.stdin.close()
    q.wait(10)

print("mcp: all ok" if not fails else f"mcp: {fails} failed")
sys.exit(1 if fails else 0)
