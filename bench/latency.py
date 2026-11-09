"""Loop latency: how long an agent waits for Brevi on each edit (docs/METRICS.md, "Check latency").

Times the commands an agent runs in its edit loop on bench/latency/ (a 3-file, ~200-line program), N runs each,
and prints p50/p95 wall-clock in ms from exec to exit. Before each timed edit the EDIT line in main.py gets a new
value, so the result cache and the generated-Rust cache both miss, as they do after a real edit.

  check_fast_edit  `brevi check --fast` after an edit (front end only: parse, inference, guard; no rustc)
  check_edit       `brevi check` after an edit (front end + rustc --emit=metadata)
  check_cached     `brevi check` again with nothing changed (result cache)
  build_edit       `brevi build` after an edit (debug build, the default)
  run_edit         `brevi run` after an edit (the quick opt-level 0 tier; docs/LATENCY.md, "Tiered builds")
  run_cached       `brevi run` again with nothing changed
  python_run       `python3 main.py`, for scale

usage: BREVI=target/release/brevi python3 bench/latency.py [--runs 20] [--only NAME,...] [--dyn] [--json out.json]
--dyn sets BREVI_DYN=1 so the same program takes the dynamic build, the path most agent code takes today.
The fixture is copied to a temp folder, so the repo is never edited. Report the machine with the numbers."""
import json, os, platform, shutil, subprocess, sys, tempfile, time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BREVI = os.path.abspath(os.environ.get('BREVI', str(REPO / 'target/release/brevi')))
MODES = ['check_fast_edit', 'check_edit', 'check_cached', 'build_edit', 'run_edit', 'run_cached', 'python_run']


def arg(name, default):
    return sys.argv[sys.argv.index(name) + 1] if name in sys.argv else default


def pct(xs, p):
    s = sorted(xs)
    k = (len(s) - 1) * p / 100
    lo = int(k)
    hi = min(lo + 1, len(s) - 1)
    return s[lo] + (s[hi] - s[lo]) * (k - lo)


def timed(cmd, cwd, env):
    t = time.perf_counter()
    p = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True)
    ms = (time.perf_counter() - t) * 1000
    return ms, p


def main():
    runs = int(arg('--runs', '20'))
    only = arg('--only', ','.join(MODES)).split(',')
    work = Path(tempfile.mkdtemp(prefix='brevi-latency-'))
    prog = work / 'prog'
    shutil.copytree(REPO / 'bench/latency', prog, ignore=shutil.ignore_patterns('__pycache__'))
    # A private cache so other runs on the machine neither help nor hurt, warmed once (runtime rlib, rustc).
    env = dict(os.environ, BREVI_CACHE=str(work / 'cache'), BREVI_PATH='1')
    if '--dyn' in sys.argv:
        env['BREVI_DYN'] = '1'
    main_py = prog / 'main.py'
    src = main_py.read_text()
    edit = [0]

    def bump():
        edit[0] += 1
        main_py.write_text(src.replace('EDIT = 0', f'EDIT = {edit[0]}', 1))

    warm = subprocess.run([BREVI, 'run', 'main.py'], cwd=prog, env=env, capture_output=True)
    if warm.returncode != 0:
        sys.exit(f'the fixture does not run under {BREVI}:\n{warm.stdout.decode()}{warm.stderr.decode()}')
    path = next((l for l in warm.stderr.decode().splitlines() + warm.stdout.decode().splitlines() if '"path"' in l), '')
    res = {}
    for mode in only:
        cmd = {
            'check_fast_edit': [BREVI, 'check', '--fast', 'main.py'],
            'check_edit': [BREVI, 'check', 'main.py'],
            'check_cached': [BREVI, 'check', 'main.py'],
            'build_edit': [BREVI, 'build', 'main.py', '-o', str(work / 'out')],
            'run_edit': [BREVI, 'run', 'main.py'],
            'run_cached': [BREVI, 'run', 'main.py'],
            'python_run': [sys.executable, 'main.py'],
        }[mode]
        times = []
        if mode.endswith('_cached'):
            bump()
            subprocess.run(cmd, cwd=prog, env=env, capture_output=True)
        for _ in range(runs):
            # An agent reads the output before its next edit; by then the background opt-level 1 build of a
            # tiered run has finished. Wait for it, so one edit's background build is not timed in the next.
            settle(work / 'cache' / 'bin')
            if mode.endswith('_edit'):
                bump()
            ms, p = timed(cmd, prog, env)
            if p.returncode != 0:
                sys.exit(f'{mode} failed:\n{p.stdout.decode()}{p.stderr.decode()}')
            times.append(ms)
        res[mode] = {'p50_ms': round(pct(times, 50), 1), 'p95_ms': round(pct(times, 95), 1), 'runs': runs}
        print(f'{mode:<16} p50 {res[mode]["p50_ms"]:>8.1f} ms   p95 {res[mode]["p95_ms"]:>8.1f} ms', flush=True)
    ver = subprocess.run([BREVI, '--version'], capture_output=True, text=True).stdout.strip()
    rustc = subprocess.run(['rustc', '--version'], capture_output=True, text=True).stdout.strip()
    meta = {'brevi': ver, 'rustc': rustc, 'python': platform.python_version(), 'machine': platform.machine(),
            'cpu': cpu(), 'cpus': os.cpu_count(), 'date': time.strftime('%Y-%m-%d'), 'path': path.strip(),
            'fixture_lines': sum(len(f.read_text().splitlines()) for f in (REPO / 'bench/latency').glob('*.py'))}
    print(json.dumps(meta))
    if '--json' in sys.argv:
        Path(arg('--json', '')).write_text(json.dumps({'meta': meta, 'results': res}, indent=1) + '\n')
    shutil.rmtree(work, ignore_errors=True)


def settle(bins, limit=30):
    end = time.time() + limit
    while time.time() < end and bins.is_dir() and any(p.name.startswith('.tmp-tier-') for p in bins.iterdir()):
        time.sleep(0.05)


def cpu():
    try:
        for l in open('/proc/cpuinfo'):
            if l.startswith('model name'):
                return l.split(':', 1)[1].strip()
    except OSError:
        pass
    return platform.processor()


if __name__ == '__main__':
    main()
