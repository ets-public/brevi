"""Exact token count from the Anthropic count_tokens endpoint (Sonnet tokenizer).

Reads text on stdin (or files given as arguments) and prints the token count.
Needs BENCH_ANTHROPIC_KEY. Results are cached by content hash in tokcache.json.
The endpoint counts a whole user message, so the fixed message overhead is subtracted.
"""
import hashlib, json, os, sys, time, urllib.request, fcntl

MODEL = os.environ.get("TOK_MODEL", "claude-sonnet-5-5")
CACHE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "tokcache.json")

def _api(text):
    body = json.dumps({"model": MODEL, "messages": [{"role": "user", "content": text}]}).encode()
    req = urllib.request.Request("https://api.anthropic.com/v1/messages/count_tokens", body, {
        "x-api-key": os.environ["BENCH_ANTHROPIC_KEY"], "anthropic-version": "2023-06-01",
        "content-type": "application/json"})
    for i in range(5):
        try:
            with urllib.request.urlopen(req, timeout=30) as r: return json.load(r)["input_tokens"]
        except Exception as e:
            if i == 4: raise
            time.sleep(2 ** i)

def _load():
    try: return json.load(open(CACHE))
    except (OSError, ValueError): return {}

def count(text):
    if not text.strip(): return 0
    key = MODEL + ":" + hashlib.sha256(text.encode()).hexdigest()
    cache = _load()
    if key not in cache:
        overhead = cache.get(MODEL + ":overhead")
        if overhead is None:
            overhead = cache[MODEL + ":overhead"] = _api("a") - 1
        n = _api(text) - overhead
        with open(CACHE + ".lock", "w") as lk:
            fcntl.flock(lk, fcntl.LOCK_EX)
            cache = _load(); cache[key] = n; cache[MODEL + ":overhead"] = overhead
            tmp = CACHE + ".%d" % os.getpid(); json.dump(cache, open(tmp, "w")); os.replace(tmp, CACHE)
    return cache[key]

if __name__ == "__main__":
    if len(sys.argv) > 1: print(sum(count(open(f).read()) for f in sys.argv[1:]))
    else: print(count(sys.stdin.read()))
