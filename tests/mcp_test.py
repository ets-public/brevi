"""Checks `brevi mcp`: the kept program, patch and edit with a check, show, and run with cases.
usage: python3 tests/mcp_test.py [BREVI]"""
import json, subprocess, sys

B = sys.argv[1] if len(sys.argv) > 1 else "target/release/brevi"
p = subprocess.Popen([B, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
n = 0

def call(tool, **args):
    global n
    n += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": n, "method": "tools/call", "params": {"name": tool, "arguments": args}}) + "\n")
    p.stdin.flush()
    r = json.loads(p.stdout.readline())["result"]
    return r["content"][0]["text"], r["isError"]

fails = 0
def expect(label, got, want_in, err=None):
    global fails
    text, is_err = got
    ok = want_in in text and (err is None or is_err == err)
    if not ok:
        fails += 1
        print(f"FAIL {label}: want {want_in!r} (error={err}), got {text!r} (error={is_err})")

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
