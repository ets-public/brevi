"""Brevi's temp folders are private to the user (src/privdir.rs, main.rs tmp_root): a folder under Brevi's name
that another local user made first in a shared temp folder is never used, nor one that is a link or is open to
others. Brevi then builds in ~/.cache/brevi/tmp, and with no home folder it stops with the reason.

Needs two users, so Linux and root: the other user's folders are made here, as root, and brevi runs as nobody in
tools/modelbox.py. Skipped elsewhere, except that GitHub's Linux job runs it under sudo with BREVI_ROOT_TESTS=1,
where a skip fails. BREVI: the brevi to test."""
import os, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))

if not sys.platform.startswith("linux") or os.geteuid() != 0:
    if os.environ.get("BREVI_ROOT_TESTS") == "1":  # CI runs it as root (toolchains.yml): a skip there is a failure
        sys.exit("privtmp: FAIL: BREVI_ROOT_TESTS=1 but this is not Linux as root, so the test cannot run")
    print("privtmp: skipped (needs Linux and root)")
    sys.exit(0)

import modelbox  # noqa: E402

BREVI = str(Path(os.environ.get("BREVI", ROOT / "target/release/brevi")).resolve())
OTHER = 4242  # another local user: not nobody, not root
ME = modelbox.NOBODY
fails = []


def expect(name, ok, detail=""):
    print(("ok   " if ok else "FAIL ") + name + ("" if ok else f": {detail}"[:600]))
    if not ok:
        fails.append(name)


box = modelbox.Box(ROOT)
cache = box.dir / "cache"  # one cache for every case; each case runs a program of its own, so each one builds


def fnv(path):  # main.rs build_dir_path's hash of a source file's path
    h = 1469598103934665603
    for b in os.path.realpath(path).encode():
        h = ((h ^ b) * 1099511628211) % (1 << 64)
    return f"{h:x}"


def case(name, plant, with_home=True):
    """A shared (sticky, 1777) temp folder in which `plant` makes what another user left at brevi-UID; brevi runs
    a program of this case's with TMPDIR there. Returns (exit code, output, the planted path)."""
    tmp = box.mkdir(f"shared-{name}-")
    os.chown(tmp, 0, 0)  # as /tmp: root's and sticky, so only its owner can remove what another user made in it
    os.chmod(tmp, 0o1777)
    at = tmp / f"brevi-{ME}"
    prog = box.mkdir(f"prog-{name}-") / "a.py"
    if name == "build-folder":  # brevi up to 0.28.1 built in TMPDIR/brevi-HASH of the source path
        at = tmp / f"brevi-{fnv(prog)}"
    plant(at)
    seen[at] = stamp(at.parent / "theirs" if name == "link" else at)
    prog.write_text(f"case = {name!r}\nprint(sum(range(5)))\n")
    os.chown(prog, ME, ME)
    h = box.mkdir(f"home-{name}-")  # a home of this case's own, so each case shows where it built
    homes[name] = h
    cmd = ["env"] + ([f"HOME={h}"] if with_home else ["-u", "HOME"]) + [f"TMPDIR={tmp}", f"BREVI_CACHE={cache}", BREVI, "run", str(prog)]
    code, out, err = box.run(cmd, timeout=900)
    return code, out + err, at


def squat(at):  # made first by the other user, open to everyone
    at.mkdir()
    os.chown(at, OTHER, OTHER)
    os.chmod(at, 0o777)


def link(at):  # a link of the other user's to a folder of theirs
    d = at.parent / "theirs"
    d.mkdir()
    os.chown(d, OTHER, OTHER)
    os.chmod(d, 0o777)
    at.symlink_to(d)
    os.lchown(at, OTHER, OTHER)


def loose(at):  # this user's name and owner, but open to others (someone may already have put files in it)
    at.mkdir()
    os.chown(at, ME, ME)
    os.chmod(at, 0o777)


def stamp(p):  # a folder's entries and the last time any was made or removed in it
    return (sorted(os.listdir(p)), os.stat(p).st_mtime_ns) if os.path.isdir(p) else None


seen, homes = {}, {}


def private(p):
    st = os.lstat(p) if os.path.lexists(p) else None
    return st is not None and st.st_uid == ME and st.st_mode & 0o7777 == 0o700


def untouched(at, p):  # nothing was made in it (or made and removed again) since it was planted
    return stamp(p) == seen[at]


code, out, _ = case("clean", lambda at: None)
expect("a clean temp folder: runs", code == 0 and out.strip() == "10", out)

for name, plant, where in [("build-folder", squat, None), ("squat", squat, None), ("link", link, "theirs"), ("loose", loose, None)]:
    code, out, at = case(name, plant)
    target = at.parent / where if where else at
    expect(f"{name}: runs", code == 0 and out.strip() == "10", out)
    expect(f"{name}: nothing written in the other user's folder", untouched(at, target), (seen[at], stamp(target)))
    if name == "build-folder":  # brevi-UID is free: the private folder is made there
        expect(f"{name}: built in TMPDIR/brevi-UID", private(at.parent / f"brevi-{ME}"), "")
    else:
        expect(f"{name}: built in ~/.cache/brevi/tmp instead", private(homes[name] / ".cache/brevi/tmp"), "")

code, out, at = case("nohome", squat, with_home=False)
expect("no home folder: stops with the reason", code != 0 and "no private temp folder" in out and "belongs to another user" in out, out)
expect("no home folder: nothing written in the other user's folder", untouched(at, at), (seen[at], stamp(at)))

box.close()
if fails:
    sys.exit(f"privtmp: {len(fails)} failed: {', '.join(fails)}")
print("privtmp: all passed")
