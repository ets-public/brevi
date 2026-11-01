"""How much plain Python does Brevi already accept?

Takes every saved single-file Python solution in solutions/*-python/, hands it unchanged to `brevi check`,
and prints the share that pass plus the most common first errors (with numbers and quotes masked so similar
errors group together). The aim is for this share to rise: every Python habit Brevi accepts is one less
difference a model has to think about.

With --full, the ones that pass are also built (`brevi check` without --fast, which runs rustc), since the fast
check does not catch type errors that only Rust sees.

usage: python3 py_as_brevi.py [--top 25] [--jobs 8] [--json out.json] [--full]
"""
import argparse, collections, concurrent.futures as cf, glob, json, os, re, shutil, subprocess, sys, tempfile
from pathlib import Path

B = Path(__file__).resolve().parent
BREVI = os.environ.get("BREVI", str(B.parent / "target" / "release" / "brevi"))


def check(path, fast=True):
    d = tempfile.mkdtemp()
    try:
        dst = Path(d) / "main.bv"
        shutil.copy(path, dst)
        r = subprocess.run([BREVI, "check"] + (["--fast"] if fast else []) + [str(dst)], capture_output=True, text=True, timeout=300)
        out = (r.stdout + r.stderr).strip()
        if r.returncode == 0:
            return path, None
        first = out.splitlines()[0] if out else "no output"
        try:
            msg = json.loads(first).get("msg", first)
        except ValueError:
            msg = first
        return path, msg
    except subprocess.TimeoutExpired:
        return path, "timeout"
    finally:
        shutil.rmtree(d, ignore_errors=True)


def run_tests(path):
    """Builds the solution and runs its task's tests: None when every test prints the expected output."""
    task = B / "tasks" / Path(path).stem / "tests"
    if not task.is_dir():
        return path, "no tests"
    d = tempfile.mkdtemp()
    try:
        dst = Path(d) / "main.bv"
        shutil.copy(path, dst)
        r = subprocess.run([BREVI, "build", str(dst), "-o", str(Path(d) / "bin")], capture_output=True, text=True, timeout=300)
        if r.returncode != 0:
            return path, "build failed"
        for i in sorted(task.glob("*.in")):
            o = i.with_suffix(".out")
            if not o.exists():
                continue
            # Only tests the Python program itself passes count.
            try:
                py = subprocess.run([sys.executable, path], stdin=open(i), capture_output=True, text=True, timeout=10).stdout
            except subprocess.TimeoutExpired:
                continue
            if py.rstrip() != o.read_text().rstrip():
                continue
            try:
                got = subprocess.run([str(Path(d) / "bin")], stdin=open(i), capture_output=True, text=True, timeout=10).stdout
            except subprocess.TimeoutExpired:
                return path, f"timeout on {i.name}"
            if got.rstrip() != o.read_text().rstrip():
                return path, f"wrong output on {i.name}"
        return path, None
    finally:
        shutil.rmtree(d, ignore_errors=True)


def norm(msg):
    msg = re.sub(r"`[^`]*`|'[^']*'|\"[^\"]*\"", "Q", msg)
    return re.sub(r"\b\d+\b", "N", msg)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--top", type=int, default=25)
    ap.add_argument("--jobs", type=int, default=os.cpu_count() or 4)
    ap.add_argument("--json")
    ap.add_argument("--full", action="store_true")
    ap.add_argument("--run", action="store_true", help="with --full: also run each built solution on its task's tests")
    a = ap.parse_args()
    # Single-file solutions only: multi-file tasks keep their files in a folder.
    files = sorted(p for p in glob.glob(str(B / "solutions" / "*-python" / "*.py")))
    with cf.ThreadPoolExecutor(a.jobs) as ex:
        res = list(ex.map(check, files))
    ok = [p for p, m in res if m is None]
    errs = collections.Counter(norm(m) for _, m in res if m is not None)
    print(f"{len(ok)} of {len(files)} Python solutions pass brevi check unchanged ({100 * len(ok) / max(1, len(files)):.1f}%)")
    if a.full:
        with cf.ThreadPoolExecutor(a.jobs) as ex:
            full = list(ex.map(lambda p: check(p, False), ok))
        built = [p for p, m in full if m is None]
        print(f"{len(built)} of {len(files)} also build ({100 * len(built) / max(1, len(files)):.1f}%); first build errors:")
        for m, n in collections.Counter(norm(m) for _, m in full if m is not None).most_common(a.top):
            print(f"{n:5}  {m[:150]}")
        if a.run:
            with cf.ThreadPoolExecutor(a.jobs) as ex:
                ran = list(ex.map(run_tests, built))
            right = [p for p, m in ran if m is None]
            tested = [p for p, m in ran if m != "no tests"]
            print(f"{len(right)} of {len(tested)} built solutions with tests give the right output on every test; wrong:")
            for p, m in ran:
                if m is not None and m != "no tests":
                    print(f"       {Path(p).parent.name}/{Path(p).name}: {m}")
    files_out = {str(Path(p).relative_to(B)): m for p, m in res}
    if a.full:
        files_out.update({str(Path(p).relative_to(B)): "build: " + m for p, m in full if m is not None})
    print("most common first errors:")
    for m, n in errs.most_common(a.top):
        print(f"{n:5}  {m[:150]}")
    if a.json:
        Path(a.json).write_text(json.dumps({"total": len(files), "pass": len(ok), "errors": errs.most_common(), "files": files_out}, indent=1))


if __name__ == "__main__":
    main()
