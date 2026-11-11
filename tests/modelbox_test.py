"""The box model-written code runs in (tools/modelbox.py) must hold: nothing of the harness is reachable from it, and
its self-check refuses a box that leaks. Needs Linux and root (as on the machines that run models' code; skipped
elsewhere, e.g. GitHub's non-root runners, where the box itself refuses to run)."""
import ast, os, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))

if not sys.platform.startswith("linux") or os.geteuid() != 0:
    print("modelbox: skipped (needs Linux and root)")
    sys.exit(0)

import modelbox  # noqa: E402

fails = []


def expect(name, ok):
    print(("ok   " if ok else "FAIL ") + name)
    if not ok:
        fails.append(name)


# A secret the box must not see, in the environment and in a file only root could read before.
os.environ["MODELBOX_TEST_TOKEN"] = "mbx-" + os.urandom(12).hex()
modelbox.SECRET_VALUES = modelbox.secret_values()
box = modelbox.Box(ROOT)
box.selftest()
probe = r"""
import os, socket
r = {}
r['uid'] = os.getuid()
r['secret_env'] = any('TOKEN' in k or 'KEY' in k for k in os.environ)
r['procs'] = len([p for p in os.listdir('/proc') if p.isdigit()])
r['run'] = os.listdir('/run')
r['home'] = os.listdir('/home')
r['root'] = sorted(os.listdir('/root'))
try:
    socket.create_connection(('1.1.1.1', 443), timeout=3); r['net'] = True
except OSError:
    r['net'] = False
try:
    open(%r, 'a'); r['repo_writable'] = True
except OSError:
    r['repo_writable'] = False
print(repr(r))
""" % str(ROOT / "README.md")
c, so, se = box.run([sys.executable, "-c", probe], timeout=120)
r = ast.literal_eval(so.strip().splitlines()[-1]) if c == 0 else {}  # output of boxed code: data only
expect("runs as nobody", r.get("uid") == modelbox.NOBODY)
expect("no secret-looking variable", r.get("secret_env") is False)
expect("no host process visible", r.get("procs", 99) <= 3)
expect("/run empty (no host sockets)", r.get("run") == [])
expect("/root shows only toolchains", set(r.get("root", ["x"])) <= set(modelbox.TOOLCHAINS))
expect("no network", r.get("net") is False)
expect("checkout not writable", r.get("repo_writable") is False)
c, so, se = box.run([sys.executable, "-c", "import os; print(os.environ.get('MODELBOX_TEST_TOKEN'))"])
expect("secret value not passed in", so.strip() == "None")
c, so, se = box.run(["sh", "-c", "echo $0", os.environ["MODELBOX_TEST_TOKEN"]])
expect("secret value redacted from output", "mbx-" not in so and "<redacted>" in so)
box.close()

# A folder passed in through a symlink is reachable by the caller's own spelling (the link is made again inside).
link = Path("/tmp") / f"modelbox-link-{os.getpid()}"
link.symlink_to(ROOT / "tests")
lbox = modelbox.Box(ROOT, ro=[link])
c, so, se = lbox.run(["ls", str(link / "modelbox_test.py")])
expect("symlinked folder reachable by its own path", c == 0)
lbox.close()
link.unlink()

# A box that leaks is refused: one with a readable file holding a secret value, and one that keeps /run.
leak = ROOT / "target" / "modelbox-leak.txt"
leak.parent.mkdir(exist_ok=True)
leak.write_text(os.environ["MODELBOX_TEST_TOKEN"])
r = subprocess.run([sys.executable, str(ROOT / "tools" / "modelbox.py"), "true"], capture_output=True, text=True, cwd=ROOT)
expect("refuses a readable secret file", r.returncode != 0 and "secret value" in r.stderr)
leak.unlink()
saved = list(modelbox.MASKED)
modelbox.MASKED.remove("/run")
modelbox.MASKED.remove("/home")
b = modelbox.Box(ROOT)
script = b.script()
b.script = lambda: script.replace("mount -t tmpfs -o mode=755 tmpfs /run\n", "")
try:
    b.selftest()
    expect("refuses a box that keeps /run and /home", False)
except SystemExit as e:
    expect("refuses a box that keeps /run and /home", "does not hold" in str(e))
modelbox.MASKED[:] = saved
b.close()
sys.exit(1 if fails else 0)
