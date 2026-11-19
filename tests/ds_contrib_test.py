"""tools/ds_contrib.py's spend cap and lane limit must hold (Adam 11-17: a run never spends past --budget, which is at
most $2; a lane gets its first run and one resume, then goes to Opus). Runs against a fake DeepSeek API on
localhost; no key, network or build needed."""
import datetime, http.server, json, os, subprocess, sys, tempfile, threading
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
os.environ["DEEPSEEK_API_KEY"] = "sk-test-not-a-key"
import ds_contrib as ds  # noqa: E402

fails = []


def expect(name, ok):
    print(("ok   " if ok else "FAIL ") + name)
    if not ok:
        fails.append(name)


def raises_stop(f):
    try:
        f()
    except ds.BudgetStop:
        return True
    return False


OFF = datetime.datetime(2026, 11, 14, 12, tzinfo=datetime.timezone.utc)  # a Saturday: off-peak all day
PEAK = datetime.datetime(2026, 11, 17, 7, tzinfo=datetime.timezone.utc)  # Tuesday 07:00 UTC: peak
expect("test clocks: OFF is off-peak, PEAK is peak", not ds.is_peak(OFF) and ds.is_peak(PEAK))
expect("a call that ends in peak is priced at peak", ds.peak_during(OFF.replace(day=17, hour=5, minute=50), PEAK))

# --- the meter on its own ---
m = ds.Meter("deepseek-v4-pro", 1.00)
n = m.plan(20000, False)
expect("a fresh run asks for the full max_tokens", n == ds.MAX_TOKENS)
m.spent = 0.99
expect("a call whose worst case does not fit is refused before it is sent", raises_stop(lambda: m.plan(20000, False)))
m.spent = 0.97
n = m.plan(20000, False)
expect("near the cap, max_tokens shrinks so the worst case still fits",
       ds.MIN_TOKENS <= n < ds.MAX_TOKENS and m.spent + m.worst(20000, n, False) <= m.budget)
expect("peak prices double the worst case", abs(m.worst(1000, 1000, True) - 2 * m.worst(1000, 1000, False)) < 1e-12)

for bad in (None, {}, {"prompt_tokens": 10}, {"prompt_tokens": "10", "completion_tokens": 5},
            {"prompt_tokens": 10, "completion_tokens": -1},
            {"prompt_tokens": 10, "completion_tokens": 5, "prompt_cache_hit_tokens": 4, "prompt_cache_miss_tokens": 4}):
    m = ds.Meter("deepseek-v4-pro", 1.00)
    expect(f"unreadable usage {bad!r} stops the run and is charged its worst case",
           raises_stop(lambda: m.charge(bad, 5000, 8000, False)) and abs(m.spent - m.worst(5000, 8000, False)) < 1e-12)
m = ds.Meter("deepseek-v4-pro", 1.00)
expect("more output than max_tokens stops the run (the bound broke)",
       raises_stop(lambda: m.charge({"prompt_tokens": 100, "completion_tokens": 9000}, 5000, 8000, False)))
m = ds.Meter("deepseek-v4-pro", 1.00)
m.charge({"prompt_tokens": 1000, "completion_tokens": 1000, "prompt_cache_hit_tokens": 600,
          "prompt_cache_miss_tokens": 400}, 5000, 8000, False)
expect("a good reply is charged at the API's numbers (off-peak, cache hits cheaper)",
       abs(m.spent - (400 * 0.66 + 600 * 0.022 + 1000 * 1.98) / 1e6) < 1e-12)
m2 = ds.Meter("deepseek-v4-pro", 1.00)
m2.charge({"prompt_tokens": 1000, "completion_tokens": 1000, "prompt_cache_hit_tokens": 600,
           "prompt_cache_miss_tokens": 400}, 5000, 8000, True)
expect("the same reply in peak costs twice as much", abs(m2.spent - 2 * m.spent) < 1e-12)


# --- chat() against a fake API ---
class Fake(http.server.BaseHTTPRequestHandler):
    script = []  # what to answer, one entry per request: "usage", "nousage", "empty", "500", "429"
    seen = []

    def log_message(self, *a):
        pass

    def do_POST(self):
        body = self.rfile.read(int(self.headers["Content-Length"]))
        req = json.loads(body)
        Fake.seen.append(req["max_tokens"])
        kind = Fake.script.pop(0) if Fake.script else "usage"
        if kind in ("500", "429"):
            self.send_response(int(kind)); self.end_headers(); self.wfile.write(b"{}"); return
        # The worst a real reply can do: every request byte a prompt token, every allowed token used.
        usage = {"prompt_tokens": len(body), "completion_tokens": req["max_tokens"]}
        out = {"choices": [{"message": {"content": "x"}}], "usage": usage}
        if kind == "nousage":
            del out["usage"]
        if kind == "empty":
            out = {}
        data = json.dumps(out).encode()
        self.send_response(200); self.send_header("Content-Length", str(len(data))); self.end_headers()
        self.wfile.write(data)


srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Fake)
threading.Thread(target=srv.serve_forever, daemon=True).start()
ds.BASE_URL = f"http://127.0.0.1:{srv.server_address[1]}"
os.environ["no_proxy"] = os.environ["NO_PROXY"] = "127.0.0.1"
no_sleep = lambda s: None  # noqa: E731
msgs = [{"role": "user", "content": "hi " * 2000}]


def run_until_stop(budget, clock):
    ds.now_utc = lambda: clock
    m = ds.Meter("deepseek-v4-pro", budget)
    calls = 0
    try:
        while calls < 500:
            ds.chat("deepseek-v4-pro", msgs + [{"role": "user", "content": "y" * 3000 * calls}], [], "max", m, no_sleep)
            calls += 1
    except ds.BudgetStop:
        pass
    return m, calls


for budget, clock, label in ((2.00, OFF, "off-peak"), (2.00, PEAK, "peak"), (0.30, OFF, "small budget")):
    Fake.script = []
    m, calls = run_until_stop(budget, clock)
    expect(f"worst-case replies ({label}): {calls} calls stop at ${m.spent:.4f}, never past ${budget:.2f}",
           calls > 0 and m.spent <= budget and m.spent > budget - m.worst(30000, ds.MIN_TOKENS, clock is PEAK))

ds.now_utc = lambda: OFF
Fake.script = ["nousage"]
m = ds.Meter("deepseek-v4-pro", 2.00)
expect("a reply with no usage stops the run (fail closed)",
       raises_stop(lambda: ds.chat("deepseek-v4-pro", msgs, [], "max", m, no_sleep)) and m.spent > 0)

Fake.script = ["500", "empty", "429"]
m = ds.Meter("deepseek-v4-pro", 2.00)
ds.chat("deepseek-v4-pro", msgs, [], "max", m, no_sleep)
nb = len(json.dumps({"model": "deepseek-v4-pro", "messages": msgs, "tools": [], "max_tokens": ds.MAX_TOKENS,
                     "reasoning_effort": "max"}).encode())
expect("a 5xx and an empty 200 are charged their worst case, a 429 is not",
       m.spent >= 2 * m.worst(nb, ds.MAX_TOKENS, False) * 0.99 and len(Fake.seen) >= 4)

Fake.script = ["500"] * 5
m = ds.Meter("deepseek-v4-pro", 0.20)
expect("retries after lost attempts stop once their worst cases use up the budget",
       raises_stop(lambda: ds.chat("deepseek-v4-pro", msgs, [], "max", m, no_sleep)) and m.spent <= 0.20)
srv.shutdown()

# --- the command line: budget cap, lane limit, ledger ---
CLI = [sys.executable, str(ROOT / "tools" / "ds_contrib.py")]


def cli(*args, ledger):
    env = {**os.environ, "DEEPSEEK_API_KEY": "sk-test-not-a-key"}
    return subprocess.run(CLI + list(args) + ["--allow-peak", "--ledger", str(ledger)], capture_output=True, text=True,
                          env=env, cwd=ROOT)


with tempfile.TemporaryDirectory() as d:
    led = Path(d) / "deepseek" / "ledger.jsonl"
    for b in ("2.01", "5", "0", "-1", "nan"):
        r = cli("some_test", "--budget", b, ledger=led)
        expect(f"--budget {b} is refused", r.returncode == 2 and "per-run cap" in r.stderr)
    led.parent.mkdir()
    led.write_text("".join(json.dumps({"event": "start", "lane": "some_test", "run": i}) + "\n" for i in (1, 2)))
    r = cli("some_test", ledger=led)
    expect("a third run of a lane is refused: hand it to Opus", r.returncode == 1 and "hand it to Opus" in r.stderr)
    led.write_text(json.dumps({"event": "start", "lane": "t", "check": "python3 x.py", "run": 1}) + "\n"
                   + json.dumps({"event": "start", "lane": "t-renamed", "check": "python3 x.py", "run": 2}) + "\n")
    r = cli("--task", str(led), "--id", "t-again", "--check", "python3 x.py", ledger=led)
    expect("a task lane renamed with --id but the same --check still counts", "hand it to Opus" in r.stderr)
    expect("ledger_runs counts only this lane",
           len(ds.ledger_runs(led, "other")) == 0 and len(ds.ledger_runs(led, "t")) == 1)
    led.write_text("not json\n")
    r = cli("some_test", ledger=led)
    expect("an unreadable ledger refuses the run", r.returncode == 1 and "can't be read or written" in r.stderr)
    r = cli("some_test", ledger=Path(d) / "no" / "such" / "ledger.jsonl")
    expect("a ledger on missing storage refuses the run", r.returncode == 1 and "can't be read or written" in r.stderr)

if fails:
    sys.exit(f"{len(fails)} ds_contrib check(s) failed")
print("ds_contrib: spend cap and lane limit hold")
