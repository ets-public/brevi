"""The box that code written by a model runs in: no credential, process, socket or network of the host is reachable.

Everything a model's work can make run (a compiler it edited, the programs that compiler builds, its snippets,
the solutions bench/e2e.py checks) runs here, never as the harness that started it. The harness holds API keys in
its environment, the session has token files on disk, and the egress proxy adds credentials for some hosts, so
model-written code that ran as the harness could read or use any of them (an env dump in a DeepSeek run once sent
a key to DeepSeek's API, 11-10).

How the box is built (Linux, run as root; anything else is refused, never run unboxed):
- `unshare --mount --pid --net --ipc --uts --fork --kill-child`: no other process is visible (so not the harness's
  /proc/PID/environ), no network (so no proxy), no shared memory of the host, and the whole box dies with its
  command on a timeout.
- It runs as `nobody`, with no capabilities, no supplementary groups and no_new_privs, in an allowlisted
  environment (locale, PATH, the toolchain homes, its own HOME/TMPDIR, the caller's BREVI_* settings). A
  secret-looking variable name is refused outright.
- What it sees on disk: /root, /home, /mnt, /tmp, /var/tmp, /run and /dev/shm are replaced by empty folders. Back
  in them come only the toolchains (/root/.cargo, .rustup, .local, read-only), the caller's read-only folders and
  its writable ones (the checkout, the box's own folder, a build cache). Of those, only what the caller makes
  nobody's (chown_tree) is writable.
- A self-check (selftest) runs before the caller's first command and the run refuses to start if the box leaks:
  wrong uid, a secret-looking variable, the harness process, a token file, an unexpected folder, a unix socket,
  the network, or a readable file holding the value of any of the harness's secret variables.

Anything that comes back out of the box (output a model will read, logs) still goes through redact(), as a
second line. Usage: `python3 tools/modelbox.py CMD...` runs one command boxed from the current checkout (for
reviewing a model's branch: its compiler and the programs it builds never run outside the box either).
"""
import os, re, shlex, shutil, subprocess, sys, tempfile
from pathlib import Path

NOBODY = 65534
SECRET_NAME = re.compile(r"KEY|TOKEN|SECRET|PASSW|CREDENTIAL|AUTH|COOKIE", re.I)
TOOLCHAINS = [".cargo", ".rustup", ".local"]
# the folders replaced by an empty one inside the box (what comes back is listed by the caller)
MASKED = ["/root", "/home", "/mnt", "/tmp", "/var/tmp", "/run", "/dev/shm"]
ENV_KEEP = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8", "TERM": "dumb", "CARGO_NET_OFFLINE": "true",
            "CARGO_HOME": "/root/.cargo", "RUSTUP_HOME": "/root/.rustup", "PYTHONDONTWRITEBYTECODE": "1"}
BOX_PATH = "/root/.cargo/bin:/root/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
# Folders the self-check's file scan skips: system files only root can change (programs, libraries, kernel views).
SCAN_SKIP = ("/proc", "/sys", "/dev", "/usr", "/nix", "/lib", "/lib64", "/bin", "/sbin", "/snap", "/boot",
             "/var/lib/dpkg", "/var/cache/apt", "/opt/pw-browsers")


def secret_values():
    """The values the box must never let out: every secret-looking variable of this process, and the session's
    token file. Longest first, so redact() replaces a value before any value it contains."""
    # (a value that is a path, like CLAUDE_SESSION_INGRESS_TOKEN_FILE's, is not a secret: the file's content is)
    vals = {v for k, v in os.environ.items() if SECRET_NAME.search(k) and len(v) >= 12 and not v.startswith("/")}
    tok = os.environ.get("CLAUDE_SESSION_INGRESS_TOKEN_FILE")
    try:
        t = Path(tok).read_text().strip() if tok else ""
        if len(t) >= 12:
            vals.add(t)
    except OSError:
        pass
    return sorted(vals, key=len, reverse=True)


SECRET_VALUES = secret_values()


def redact(text):
    for v in SECRET_VALUES:
        text = text.replace(v, "<redacted>")
    return text


def chown_tree(p):
    """Make a folder the box's to write: it and everything under it become nobody's (links are not followed)."""
    p = Path(p)
    if not p.exists() or p.stat().st_uid == NOBODY:  # already the box's: files under it were made in the box
        return
    os.lchown(p, NOBODY, NOBODY)
    for d, dirs, files in os.walk(p):
        for n in dirs + files:
            os.lchown(os.path.join(d, n), NOBODY, NOBODY)


class Box:
    """root: the checkout (readable; write only what you chown_tree). rw: other folders the box may write (made
    nobody's). ro: folders it may read. All three appear at their own paths inside."""

    def __init__(self, root, rw=(), ro=()):
        if not sys.platform.startswith("linux") or os.geteuid() != 0 or not shutil.which("unshare") or not shutil.which("setpriv"):
            sys.exit("refusing to run model-written code: the box needs Linux, root, unshare and setpriv "
                     "(it never runs unboxed)")
        self.root = Path(root).resolve()
        self.dir = Path(tempfile.mkdtemp(prefix="modelbox-"))  # the box's home and temp folder
        (self.dir / "tmp").mkdir()
        self.stage = Path("/run") / f"modelbox-stage-{os.getpid()}-{self.dir.name}"
        self.rw = [self.dir] + [Path(x).resolve() for x in rw if x]
        for d in self.rw:
            d.mkdir(parents=True, exist_ok=True)
            chown_tree(d)
        os.chmod(self.dir, 0o700)
        self.ro = [Path(x).resolve() for x in ro if x and Path(x).is_dir()]
        # Folders are bound in at their resolved paths; a symlink on the way to one (such as /mnt/project-files ->
        # /mnt/attach/project-files) is made again inside, so the caller's own spelling of the path still works.
        self.links = {}
        for x in [root, *rw, *ro]:
            if not x:
                continue
            cur = Path("/")
            for part in Path(os.path.abspath(x)).parts[1:]:
                cur = cur / part
                if cur.is_symlink():
                    self.links[str(cur)] = os.readlink(cur)
        self.stage.mkdir(mode=0o700, exist_ok=True)
        self.checked = False

    def mkdir(self, prefix):
        """A fresh folder for one command (the box can write it)."""
        d = Path(tempfile.mkdtemp(prefix=prefix, dir=self.dir / "tmp"))
        os.chown(d, NOBODY, NOBODY)
        return d

    def env(self, extra=None):
        env = dict(ENV_KEEP, PATH=BOX_PATH, HOME=str(self.dir), TMPDIR=str(self.dir / "tmp"))
        env.update({k: v for k, v in os.environ.items() if k.startswith("BREVI_") and not SECRET_NAME.search(k)})
        env.update(extra or {})
        bad = [k for k in env if SECRET_NAME.search(k)]
        if bad:
            raise RuntimeError(f"refusing to pass {bad} into the box")
        return env

    def script(self):
        q = lambda p: shlex.quote(str(p))
        s = self.stage
        # parents before children, so a folder inside another one comes back on top of it
        back = sorted({(p, "rw") for p in [self.root] + self.rw} | {(p, "ro") for p in self.ro}, key=lambda x: len(x[0].parts))
        lines = ["set -e", "mount --make-rprivate /", f"mount -t tmpfs -o mode=700 tmpfs {q(s)}",
                 f"mkdir {q(s)}/root && mount --rbind /root {q(s)}/root"]
        for i, (p, _) in enumerate(back):
            lines.append(f"mkdir {q(s)}/b{i} && mount --rbind {q(p)} {q(s)}/b{i}")
        for m in MASKED:
            if m == "/run":
                continue  # the stage lives in /run: masked last
            lines.append(f"if [ -d {m} ]; then mount -t tmpfs -o mode={'1777' if m in ('/tmp', '/var/tmp', '/dev/shm') else '755'} tmpfs {m}; fi")
        for t in TOOLCHAINS:
            lines.append(f"if [ -d {q(s)}/root/{t} ]; then mkdir /root/{t} && mount --rbind {q(s)}/root/{t} /root/{t} "
                         f"&& mount -o remount,bind,ro /root/{t}; fi")
        for i, (p, mode) in enumerate(back):
            if str(p).startswith("/run/") or str(p) == "/run":
                raise RuntimeError(f"{p}: a folder under /run cannot be passed into the box")
            lines.append(f"mkdir -p {q(p)} && mount --bind {q(s)}/b{i} {q(p)}" + (f" && mount -o remount,bind,ro {q(p)}" if mode == "ro" else ""))
        lines.append(f"umount -l {q(s)}")
        lines.append("mount -t tmpfs -o mode=755 tmpfs /run")
        for link, target in sorted(self.links.items()):
            if any(link == m or link.startswith(m + "/") for m in MASKED):  # outside the masked folders it is still there
                lines.append(f"[ -e {q(link)} ] || [ -L {q(link)} ] || {{ mkdir -p {q(os.path.dirname(link))} && ln -s {q(target)} {q(link)}; }}")
        lines.append(f'exec setpriv --reuid={NOBODY} --regid={NOBODY} --clear-groups --no-new-privs --inh-caps=-all --bounding-set=-all -- "$@"')
        return "\n".join(lines)

    def argv(self, cmd, env=None):
        return (["unshare", "--mount", "--pid", "--net", "--ipc", "--uts", "--fork", "--kill-child", "--mount-proc", "--",
                 "sh", "-c", self.script(), "box", "env", "-i"] + [f"{k}={v}" for k, v in self.env(env).items()] + list(cmd))

    def run(self, cmd, timeout=900, env=None, cwd=None, capture=True):
        """Run one command in the box. Returns (code, stdout, stderr) with secret values redacted (capture=False:
        output goes straight to this process's, and stdout/stderr come back empty)."""
        if not self.checked:
            self.selftest()
        try:
            r = subprocess.run(self.argv(cmd, env), cwd=cwd or self.root, capture_output=capture, text=True,
                               timeout=timeout, env={"PATH": BOX_PATH})
        except subprocess.TimeoutExpired:
            return -1, "", f"timed out after {timeout}s"
        return r.returncode, redact(r.stdout or ""), redact(r.stderr or "")

    def selftest(self):
        """Refuse to go on unless the box holds (see the module's notes). Run before the first command."""
        allowed = sorted({str(p) for p in [self.root, *self.rw, *self.ro]})
        report = self.mkdir("selftest-") / "readable.txt"
        probe = f"""
import os, re, socket, stat
bad = []
if os.getuid() != {NOBODY} or os.geteuid() != {NOBODY}: bad.append('runs as uid %d' % os.geteuid())
if [k for k in os.environ if re.search({SECRET_NAME.pattern!r}, k, re.I)]: bad.append('a secret-looking variable')
if os.path.exists('/proc/{os.getpid()}'): bad.append('the harness process is visible')
if not set(os.listdir('/root')) <= {set(TOOLCHAINS) | {p.split('/')[2] for p in allowed if p.startswith('/root/')}!r}:
    bad.append('/root shows ' + str(sorted(os.listdir('/root'))))
if os.listdir('/run'): bad.append('/run shows ' + str(os.listdir('/run')))
if os.listdir('/dev/shm'): bad.append('/dev/shm is not empty')
for p in {[os.environ.get("CLAUDE_SESSION_INGRESS_TOKEN_FILE", ""), "/root/.claude", "/root/.ccr", "/home/claude/.claude"]!r}:
    if p and os.path.exists(p): bad.append(p + ' is visible')
try:
    socket.create_connection(('1.1.1.1', 443), timeout=3); bad.append('the network is reachable')
except OSError:
    pass
allowed = {allowed!r}
def inside(d):
    return any(d == a or d.startswith(a + '/') or a.startswith(d + '/') for a in allowed)
readable = []
for top in ('/root', '/home', '/mnt', '/tmp', '/var', '/run', '/etc', '/opt', '/srv', '/media'):
    for d, dirs, files in os.walk(top, onerror=lambda e: None):
        if d.startswith({SCAN_SKIP!r}):
            dirs[:] = []
            continue
        if top in ('/home', '/mnt', '/tmp', '/run') and not inside(d) and d != top:
            bad.append(d + ' is visible')
            dirs[:] = []
            continue
        for n in files:
            p = os.path.join(d, n)
            try:
                st = os.lstat(p)
            except OSError:
                continue
            if stat.S_ISSOCK(st.st_mode): bad.append('socket ' + p)
            elif stat.S_ISREG(st.st_mode) and st.st_size < 64 << 20 and os.access(p, os.R_OK): readable.append(p)
open({str(report)!r}, 'w').write('\\n'.join(readable))
print('BOX OK' if not bad else 'BOX LEAKS: ' + '; '.join(bad[:20]))
"""
        self.checked = True
        c, so, se = self.run(["python3", "-c", probe], timeout=300)
        if c != 0 or so.strip() != "BOX OK":
            self.checked = False
            sys.exit("refusing to run: the box for model-written code does not hold: " + (so + se).strip()[:800])
        # Every file the box could read, checked here (outside) for the value of any secret: none may hold one.
        vals = [v.encode() for v in SECRET_VALUES]
        hits = []
        for p in report.read_text().split("\n") if report.exists() else []:
            try:
                b = Path(p).read_bytes()
            except OSError:
                continue
            if any(v in b for v in vals):
                hits.append(p)
        if hits:
            self.checked = False
            sys.exit("refusing to run: the box can read files that hold a secret value: " + ", ".join(hits[:10]))

    def close(self):
        shutil.rmtree(self.dir, ignore_errors=True)
        try:
            self.stage.rmdir()
        except OSError:
            pass


def main():
    if len(sys.argv) < 2:
        sys.exit("usage: modelbox.py CMD... (runs CMD in the box, from the current checkout)")
    here = Path.cwd().resolve()
    top = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True).stdout.strip()
    root = Path(top) if top else here
    (root / "target").mkdir(exist_ok=True)
    chown_tree(root / "target")
    box = Box(root, rw=[root / "target", os.environ.get("BREVI_CACHE")], ro=[x for x in os.environ.get("DS_BOX_RO", "").split(":") if x])
    try:
        return box.run(sys.argv[1:], timeout=None, cwd=here, capture=False)[0]
    finally:
        box.close()


if __name__ == "__main__":
    sys.exit(main())
