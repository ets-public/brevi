# `x or ""` on a JSON value and json.load(open(p)) build typed (#195).
import json


def key(topic):
    return (topic or "").lower()


d = json.loads('{"topics": ["A B", "", null], "items": [{"name": "Al", "n": 3, "tag": "ab"}, {"name": null, "n": 0}]}')
print([key(t) for t in d["topics"]], key(None))
for it in d["items"]:
    name = it.get("name") or "anon"
    n = it.get("n") or 0
    print(name, n + 1, (it.get("tag") or "").upper(), it.get("name") or it.get("tag") or "-")
cfg = json.load(open("tests/typed/json_or.json"))
print(len(cfg), cfg["w"] or "none")
import re
for t in d["topics"][:2]:
    print(bool(re.search(r"\bb", t, re.I)), re.sub(r"[^a-z]", "", (t or "").lower()), re.findall(r"[A-Z]", t))
