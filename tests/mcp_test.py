"""Checks `brevi mcp`: plain Python by default, the kept program, patch and edit with a check, show, and run
with cases, for Python and for Brevi code.
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
# The default sandbox holds Python too: a file read, a Python library or a write is refused, never run.
start()
expect("sandbox read", call("run", code="with open('/etc/hostname') as f:\n    print(f.read().strip())\n"), "needs allow read", True)
expect("sandbox library", call("run", code="import os\nprint(os.listdir('.'))\n"), "needs allow native", True)
esc = call("run", code="import sys\nsys.modules['os'].system('echo escaped')\n")
if "escaped" in esc[0].replace("echo escaped", "") or not esc[1]:
    fails += 1
    print(f"FAIL sandbox sys.modules: {esc!r}")
expect("sandbox write", call("run", code="open('x.txt', 'w').write('x')\n"), "needs allow write", True)
expect("sandbox off", call("run", code="with open('/etc/hostname') as f:\n    print(len(f.read()) > 0)\n", sandbox=False), "True", False)

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
print("mcp: all ok" if not fails else f"mcp: {fails} failed")
sys.exit(1 if fails else 0)
