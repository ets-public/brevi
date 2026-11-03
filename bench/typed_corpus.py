"""Which path does agent-written Python take? Builds every saved Python solution (bench/solutions/*-python)
and every program in a repro folder (BREVI_REPRO, default /mnt/project-files/brevi-repro; a folder with a
main.py is built as one program), records each build's {"path":...} report, and prints the counts.

With --shadow, each solution that built is also run on its task's tests and compared with python3 (run with
two hash seeds; a test where CPython itself differs between them is skipped): `ok`, or `BAD` and the tests.

usage: BREVI=target/release/brevi python3 bench/typed_corpus.py out.json [--shadow] [--only SUBSTR]
Group the reasons with: python3 -c "import json,collections; ..." over out.json's `path` and `why`."""
import json, os, glob, shutil, subprocess, sys, tempfile, concurrent.futures as cf
from pathlib import Path
REPO = Path(__file__).resolve().parent.parent
BREVI = os.environ.get('BREVI', str(REPO/'target/release/brevi'))
def programs():
    out = []
    for f in sorted(glob.glob(str(REPO/'bench/solutions/*-python/*.py'))):
        out.append(('sol', f, None))
    R = os.environ.get('BREVI_REPRO', '/mnt/project-files/brevi-repro')
    for root, dirs, files in os.walk(R):
        dirs.sort()
        pys = sorted(x for x in files if x.endswith('.py'))
        if not pys: continue
        if 'main.py' in pys:
            out.append(('repro', os.path.join(root, 'main.py'), root))
        else:
            for x in pys:
                out.append(('repro', os.path.join(root, x), None))
    return out
SHADOW = '--shadow' in sys.argv
def shadow(f, binp, cwd, run=None):
    stem = Path(f).stem
    if stem == 'main':  # a repro folder named <model>_<task> or <run>-<model>-<task>
        stem = Path(f).parent.name.replace('-', '_').split('_', 1)[-1].split('_', 1)[-1] if '-' in Path(f).parent.name else Path(f).parent.name.split('_', 1)[-1]
    tests = REPO/'bench/tasks'/stem/'tests'
    if not tests.is_dir(): return 'no tests'
    bad = []
    for i in sorted(tests.glob('*.in')):
        data = i.read_bytes()
        try:
            p1 = subprocess.run([sys.executable, run or f], input=data, capture_output=True, timeout=20, env=dict(os.environ, PYTHONHASHSEED='1'), cwd=cwd)
            p2 = subprocess.run([sys.executable, run or f], input=data, capture_output=True, timeout=20, env=dict(os.environ, PYTHONHASHSEED='2'), cwd=cwd)
        except subprocess.TimeoutExpired:
            continue
        try:
            b = subprocess.run([binp], input=data, capture_output=True, timeout=20, cwd=cwd)
        except subprocess.TimeoutExpired:
            bad.append(i.name + ':timeout'); continue
        if (b.stdout, b.returncode != 0) in ((p1.stdout, p1.returncode != 0), (p2.stdout, p2.returncode != 0)):
            continue
        if p1.stdout != p2.stdout:
            continue  # CPython itself differs between runs
        bad.append(i.name)
    return 'ok' if not bad else 'BAD ' + ' '.join(bad[:5])
def one(p):
    kind, f, d = p
    t = tempfile.mkdtemp()
    try:
        if d:
            for x in os.listdir(d):
                if x.endswith('.py'): shutil.copy(os.path.join(d, x), t)
            main = os.path.join(t, 'main.py')
        else:
            main = os.path.join(t, os.path.basename(f))
            shutil.copy(f, main)
        env = dict(os.environ, BREVI_PATH='1')
        try:
            r = subprocess.run([BREVI, 'build', main, '-o', os.path.join(t, 'bin')], capture_output=True, text=True, timeout=300, env=env, cwd=t)
        except subprocess.TimeoutExpired:
            return dict(kind=kind, f=f, path='timeout')
        rec = None
        for line in r.stderr.splitlines():
            if line.startswith('{"path"'):
                try: rec = json.loads(line)
                except ValueError: pass
        if rec is None:
            rec = {'path': 'error' if r.returncode else 'none', 'why': r.stderr.strip().splitlines()[0][:300] if r.stderr.strip() else ''}
        rec.update(kind=kind, f=f, rc=r.returncode)
        if SHADOW and r.returncode == 0:
            rec['shadow'] = shadow(f, os.path.join(t, 'bin'), t, main)
        return rec
    finally:
        shutil.rmtree(t, ignore_errors=True)
if __name__ == '__main__':
    ps = programs()
    if '--only' in sys.argv:
        s = sys.argv[sys.argv.index('--only')+1]; ps = [p for p in ps if s in p[1]]
    with cf.ThreadPoolExecutor(4) as ex:
        res = list(ex.map(one, ps))
    json.dump(res, open(sys.argv[1], 'w'), indent=1)
    import collections
    c = collections.Counter((r['kind'], r['path']) for r in res)
    for k, v in sorted(c.items()): print(k, v)
