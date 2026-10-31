import sys, re

text = sys.stdin.read().split("\n")
k = 0
while k < len(text) and text[k].strip() != "---":
    k += 1
ini = text[:k]
queries = text[k + 1:]

secs = {}
cur = "default"
declared = set()
for line in ini:
    s = line.strip()
    if not s or s[0] in ";#":
        continue
    if s.startswith("[") and s.endswith("]"):
        cur = s[1:-1].strip()
        declared.add(cur)
        secs.setdefault(cur, {})
        continue
    if "=" in s:
        key, val = s.split("=", 1)
        secs.setdefault(cur, {})[key.strip()] = val.strip()

ref_re = re.compile(r"\$\{([^}]*)\}")

class Cycle(Exception):
    pass

def lookup(sec, name):
    if "." in name:
        a, b = name.split(".", 1)
        if a in secs and b in secs[a]:
            return (a, b)
        return None
    if sec in secs and name in secs[sec]:
        return (sec, name)
    if name in secs.get("default", {}):
        return ("default", name)
    return None

def resolve(sec, key, stack):
    if (sec, key) in stack:
        raise Cycle()
    stack.add((sec, key))
    val = secs[sec][key]
    def rep(m):
        t = lookup(sec, m.group(1).strip())
        if t is None:
            return ""
        return resolve(t[0], t[1], stack)
    out = ref_re.sub(rep, val)
    stack.discard((sec, key))
    return out

def get(sec, key):
    try:
        return resolve(sec, key, set())
    except Cycle:
        return "<cycle>"

for q in queries:
    q = q.strip()
    if not q:
        continue
    if q == "sections":
        print(" ".join(sorted(n for n, d in secs.items() if d)))
        continue
    if "." not in q:
        print("<missing>")
        continue
    sec, key = q.split(".", 1)
    if key == "*":
        if sec not in secs:
            print("<missing>")
        else:
            for kk in sorted(secs[sec]):
                print("%s=%s" % (kk, get(sec, kk)))
    elif sec in secs and key in secs[sec]:
        print(get(sec, key))
    else:
        print("<missing>")
