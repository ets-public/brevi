"""Checks the templates without any platform account.

    python3 templates/test.py            # the static checks only (no Docker needed; CI runs these)
    python3 templates/test.py --docker   # also runs programs in the brevi-sandbox image, as every platform does
                                         # (build it first: docker build -t brevi-sandbox templates/image)
"""
import ast, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from brevi_tool import Grants, command  # noqa: E402

fails = 0


def check(ok, what):
    global fails
    print(("ok   " if ok else "FAIL ") + what)
    fails += not ok


# 1. The Cloudflare image is the shared image plus the shim: the bodies must not drift apart.
image = open(os.path.join(HERE, "image", "Dockerfile")).read()
cf = open(os.path.join(HERE, "cloudflare", "Dockerfile")).read()
body = image[image.index("FROM debian"):]
check(body in cf and "sandbox-shim" in cf, "cloudflare/Dockerfile holds image/Dockerfile's body")
# E2B, Daytona and Modal build the image from its text alone, so it must not COPY or ADD files.
check(not re.search(r"^(COPY|ADD) ", image, re.M), "image/Dockerfile has no COPY or ADD")
# The shim's tag must match the @cloudflare/sandbox version the Worker uses.
pkg = open(os.path.join(HERE, "cloudflare", "package.json")).read()
ver = re.search(r'"@cloudflare/sandbox": "([^"]+)"', pkg).group(1)
check(f"cloudflare/sandbox:{ver}" in cf, f"shim image tag matches @cloudflare/sandbox {ver}")

# 2. Every Python file parses.
for root, _, files in os.walk(HERE):
    for f in files:
        if f.endswith(".py"):
            p = os.path.join(root, f)
            try:
                ast.parse(open(p).read(), p)
                check(True, f"parses: {os.path.relpath(p, HERE)}")
            except SyntaxError as e:
                check(False, f"parses: {os.path.relpath(p, HERE)}: {e}")

# 3. The command: grants and limits come from Grants, quoted for the shell.
g = Grants(allow=["read"], allow_net=["api.github.com"], limits={"cpu": 2, "time": "500ms"})
check(command(g) == "brevi run --sandbox --allow read --allow-net api.github.com --limit cpu=2,time=500ms main.py < stdin.txt",
      "command() builds the brevi run line")
check(command(Grants(limits={})) == "brevi run --sandbox main.py < stdin.txt", "command() with nothing granted")

# 4. Programs in the real image, through the Docker template (what each platform runs).
if "--docker" in sys.argv:
    import importlib.util
    spec = importlib.util.spec_from_file_location("docker_box", os.path.join(HERE, "docker", "sandbox.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    cases = [
        ("reads stdin", "import sys\nprint(sum(map(int, sys.stdin.read().split())))", "1 2 3", lambda r: r.exit_code == 0 and r.stdout == "6\n"),
        ("dynamic program", "import json\nfrom collections import Counter\nprint(json.dumps(Counter('abca').most_common(1)))", "", lambda r: r.stdout == '[["a", 2]]\n'),
        ("file read refused", "print(open('/etc/hostname').read())", "", lambda r: r.exit_code == 1 and "needs allow read" in r.stdout + r.stderr),
        ("network refused", "import urllib.request\nprint(urllib.request.urlopen('https://example.com').status)", "", lambda r: r.exit_code != 0 and "allow" in r.stdout + r.stderr),
        ("subprocess refused", "import subprocess\nprint(subprocess.run(['id']).returncode)", "", lambda r: r.exit_code != 0),
        ("memory limit", "x = []\nwhile True: x.append('a' * 10**6)", "", lambda r: r.exit_code == 124 and r.limit == "mem"),
        ("time limit", "while True: pass", "", lambda r: r.exit_code == 124 and r.limit in ("time", "cpu")),
        ("output limit", "while True: print('x' * 1000)", "", lambda r: r.exit_code == 124 and r.limit == "output"),
    ]
    limits = {"cpu": 3, "mem": 256, "time": 5, "output": 64, "file": 4}
    with mod.Sandbox(Grants(limits=limits)) as box:
        for name, code, stdin, ok in cases:
            r = box.run(code, stdin)
            check(ok(r), f"docker: {name}" + ("" if ok(r) else f": exit {r.exit_code} stdout {r.stdout[:200]!r} stderr {r.stderr[-300:]!r}"))
    with mod.Sandbox(Grants(allow=["read"])) as box:
        r = box.run("print(open('/etc/hostname').read() != '')")
        check(r.stdout == "True\n", "docker: file read granted by the caller")

print("all passed" if not fails else f"{fails} failed")
sys.exit(1 if fails else 0)
