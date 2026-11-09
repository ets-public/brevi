"""The Python SDK (packaging/pypi/brevi) and the brevi-python shim, against the built binary.

    BREVI=./target/release/brevi python3 tests/sdk_test.py
"""
import asyncio, os, subprocess, sys, tempfile

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
os.environ["BREVI_BIN"] = os.path.abspath(os.environ.get("BREVI", os.path.join(ROOT, "target/release/brevi")))
sys.path.insert(0, os.path.join(ROOT, "packaging/pypi"))
import brevi  # noqa: E402

fails = 0


def expect(label, ok, got):
    global fails
    if not ok:
        fails += 1
        print(f"FAIL {label}: {got!r}")


# The README example.
r = brevi.run("print(sum(range(10)))")
expect("readme run", (r.stdout, r.exit_code, r.path, r.ok) == ("45\n", 0, "typed", True), r)
r = brevi.run("import sys\nprint('out')\nprint('err', file=sys.stderr)\nsys.exit(3)\n")
expect("stdout and stderr apart", (r.stdout, r.stderr, r.exit_code, r.ok) == ("out\n", "err\n", 3, False), (r.stdout, r.stderr, r.exit_code))
r = brevi.run("a, b = map(int, input().split())\nprint(a * b)\n", stdin="3 4\n")
expect("stdin", r.stdout == "12\n", r)
r = brevi.run("import sys\nprint(sys.argv[1:])\n", args=["x", "y"])
expect("args", r.stdout == "['x', 'y']\n", r)

# The sandbox: nothing is granted unless the caller grants it, and the code cannot grant itself anything.
r = brevi.run("with open('/etc/hostname') as f:\n    print(f.read())\n")
expect("sandbox refuses a read", not r.ok and "needs allow read" in r.text, r.text)
r = brevi.run("# allow read\nwith open('/etc/hostname') as f:\n    print(len(f.read()) > 0)\n")
expect("an allow line grants nothing", not r.ok and "needs allow read" in r.text, r.text)
with tempfile.TemporaryDirectory() as d:
    open(os.path.join(d, "in.txt"), "w").write("hello\n")
    with brevi.Session(cwd=d, allow_read=["."]) as s:
        # The program runs in its own build folder, so it names granted files by their full path.
        r = s.run(f"with open({os.path.join(d, 'in.txt')!r}) as f:\n    print(f.read().strip())\n")
        expect("session grant", r.stdout == "hello\n", r.text)
        r = s.run(f"open({os.path.join(d, 'out.txt')!r}, 'w').write('x')\n")
        expect("read is not write", not r.ok and "needs allow write" in r.text, r.text)

# Limits.
r = brevi.run("while True:\n    pass\n", limits={"time": 1})
expect("time limit", r.limit == "time" and not r.ok, (r.limit, r.text))

# Cases and verify.
r = brevi.run("print(int(input()) * 2)\n", cases=[{"stdin": "1\n", "expect": "2"}, {"stdin": "5\n", "expect": "10"}])
expect("cases pass", r.ok and "passed 2/2" in r.text, r.text)
r = brevi.run("print(int(input()) * 2)\n", cases=[{"stdin": "1\n", "expect": "3"}])
expect("cases fail", not r.ok, r.text)
r = brevi.verify("print(int(input()) + 1)\n", inputs=["1\n", "2\n"])
expect("verify identical", r.ok and r.certificate and r.certificate.get("verify") == "identical", r.text)

# Errors come back as Results, not exceptions; a build error names the line.
r = brevi.check("print(totl)\n")
expect("check error", not r.ok and '"line":1' in r.text, r.text)
expect("check ok", brevi.check("print(1)\n").ok, "")

# The tool a harness hands a model, and the reply for it.
expect("tool has no grant field", set(brevi.TOOL["input_schema"]["properties"]) == {"code", "stdin"}, brevi.TOOL)
reply = brevi.handle({"code": "print(input()[::-1])", "stdin": "abc\n"})
expect("handle", reply.startswith("exit code 0") and "cba" in reply, reply)
try:
    brevi.run("print(1)", sandbox=False, allow=["read"])
    expect("grant without sandbox is an error", False, "no error")
except ValueError:
    pass


async def main():
    async with brevi.AsyncSession() as s:
        rs = [await s.run(f"print({i} * {i})") for i in range(3)]
        expect("async run", [r.stdout for r in rs] == ["0\n", "1\n", "4\n"], rs)


asyncio.run(main())

# The shim: `brevi python` (and the binary under the name brevi-python or python3) is python3 for every
# invocation, native when it can be.
py = [os.environ["BREVI_BIN"], "python"]
sh = lambda *a, stdin=None, env=None: subprocess.run(py + list(a), input=stdin, capture_output=True, text=True,
                                                     env={**os.environ, **(env or {})}, timeout=120)
cp = lambda *a, stdin=None: subprocess.run([sys.executable] + list(a), input=stdin, capture_output=True, text=True, timeout=60)
with tempfile.TemporaryDirectory() as d:
    progs = {
        "args.py": "import sys\nprint(sum(int(x) for x in sys.argv[1:]), len(sys.argv))\n",
        "argv0.py": "import sys\nprint(sys.argv)\n",
        "err.py": "print(totl)\n",
        "exit.py": "import sys\nprint('bye')\nsys.exit(4)\n",
        "eval.py": "print(eval('1 + 2'))\n",
    }
    for name, src in progs.items():
        p = os.path.join(d, name)
        open(p, "w").write(src)
        got, want = sh(p, "1", "2"), cp(p, "1", "2")
        expect(f"shim {name}", (got.stdout, got.stderr, got.returncode) == (want.stdout, want.stderr, want.returncode),
               (got.stdout, got.stderr, got.returncode, want.stdout, want.stderr))
    got = sh("-c", "import sys\nprint(sys.argv[1:])", "a")
    expect("shim -c", got.stdout == "['a']\n" and got.returncode == 0, got)
    got = sh(stdin="print(6 * 7)\n")
    expect("shim stdin script", got.stdout == "42\n", got)
    got = sh("-c", "import sys; print(sys.version_info[0])")
    expect("shim -c on CPython-only", got.stdout == "3\n", got)
    got = sh("-V")
    expect("shim flags go to CPython", got.stdout.startswith("Python 3"), got)
    got = sh("-c", "import sys; print(sys.executable)", env={"BREVI_SHIM": "0"})
    expect("BREVI_SHIM=0", got.returncode == 0 and "python" in got.stdout, got)

if fails:
    print(f"{fails} SDK check(s) failed")
    sys.exit(1)
print("sdk: all checks passed")
