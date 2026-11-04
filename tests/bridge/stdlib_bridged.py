# Standard-library modules the dynamic build lacks run in CPython; the program around them stays native.
import argparse
import hashlib
import logging
import shlex
import sqlite3
import sys
from urllib.parse import urlencode, urlparse, quote


def slug(s):
    out = []
    for ch in s.lower():
        if ch.isalnum():
            out.append(ch)
        elif out and out[-1] != "-":
            out.append("-")
    return "".join(out).strip("-")


logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s", stream=sys.stdout)
log = logging.getLogger("pipe")
ap = argparse.ArgumentParser()
ap.add_argument("--n", type=int, default=3)
args = ap.parse_args([])
db = sqlite3.connect(":memory:")
db.execute("create table posts (id integer primary key, title text, score real)")
rows = [(i, f"Post number {i}!", i * 1.5) for i in range(1, args.n + 2)]
db.executemany("insert into posts values (?, ?, ?)", rows)
db.commit()
for pid, title, score in db.execute("select id, title, score from posts where score > ? order by score desc", (2,)):
    log.info("%s %s %.1f", pid, slug(title), score)
    print(pid, hashlib.sha256(title.encode()).hexdigest()[:8])
total = db.execute("select sum(score) from posts").fetchone()[0]
print("total", total, type(total).__name__)
u = urlparse("https://example.com/a/b?x=1#frag")
print(u.scheme, u.netloc, u.path, u.query, u.fragment)
print(urlencode({"q": "brevi lang", "page": 2}), quote("a b/c"))
print(shlex.split('post "hello world" --tag=x'), shlex.quote("it's"))
db.close()
