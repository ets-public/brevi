#!/usr/bin/env python3
"""Keeps every place that documents --limit in step with LIMIT_PLATFORMS in src/proc.rs."""
import glob, json, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OSES = ["linux", "macos", "windows"]
NAME = {"linux": "Linux", "macos": "macOS", "windows": "Windows"}
errors = []


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
        return f.read()


def table():
    src = read("src/proc.rs")
    m = re.search(r"LIMIT_PLATFORMS:[^=]*=\s*\[(.*?)\n\];", src, re.S)
    if not m:
        sys.exit("limits_check: LIMIT_PLATFORMS not found in src/proc.rs")
    t = {}
    for name, oses in re.findall(r'\("(\w+)",\s*&\[([^\]]*)\]\)', m.group(1)):
        t[name] = re.findall(r'"(\w+)"', oses)
    if len(t) != 10 or any(o not in OSES for v in t.values() for o in v):
        sys.exit("limits_check: cannot parse LIMIT_PLATFORMS: %r" % t)
    return t


def named(desc):
    """Platforms a description names, from a trailing '(Linux and macOS)'-style note."""
    m = re.search(r"\(((?:Linux|macOS|Windows)(?:(?:,| and|, and) (?:Linux|macOS|Windows))*)\)\s*$", desc)
    if not m:
        return None
    return {o for o in OSES if NAME[o] in m.group(1)}


def check_mcp(t):
    src = read("src/mcp.rs")
    found = 0
    for m in re.finditer(r'"limits":\{"type":"object","properties":(\{.*?\}\}),"additionalProperties"', src):
        try:
            props = json.loads(m.group(1))
        except ValueError as e:
            errors.append("src/mcp.rs: limits schema is not valid JSON: %s" % e)
            continue
        found += 1
        if set(props) != set(t):
            errors.append("src/mcp.rs: limits properties %s differ from LIMIT_PLATFORMS %s" % (sorted(props), sorted(t)))
        for name, p in props.items():
            if name not in t:
                continue
            desc, want = p.get("description", ""), set(t[name])
            got = named(desc)
            if want == set(OSES):
                if got is not None:
                    errors.append("src/mcp.rs: limit %s is enforced everywhere; remove the platform note from %r" % (name, desc))
            elif got != want:
                errors.append("src/mcp.rs: limit %s description %r must end with the platforms %s" % (name, desc, " and ".join(NAME[o] for o in OSES if o in want)))
    if found == 0:
        errors.append("src/mcp.rs: no limits schema found")


def check_matrix(t):
    path = "docs/SUPPORT_MATRIX.md"
    if not os.path.exists(os.path.join(ROOT, path)):
        errors.append(path + " is missing")
        return
    text = read(path)
    m = re.search(r"^## Resource limits by platform\n(.*?)(?=^## |\Z)", text, re.S | re.M)
    if not m:
        errors.append(path + ": no '## Resource limits by platform' section")
        return
    rows = {}
    for r in re.finditer(r"^\|\s*`(\w+)`\s*\|\s*(\w+|-)\s*\|\s*(\w+|-)\s*\|\s*(\w+|-)\s*\|", m.group(1), re.M):
        rows[r.group(1)] = {o for o, c in zip(OSES, r.groups()[1:]) if c == "yes"}
    for name, oses in t.items():
        if name not in rows:
            errors.append("%s: no row for %s" % (path, name))
        elif rows[name] != set(oses):
            errors.append("%s: %s row says %s, code says %s" % (path, name, sorted(rows[name]), sorted(oses)))
    for name in rows:
        if name not in t:
            errors.append("%s: row for unknown limit %s" % (path, name))


def check_prose(t):
    files = ["README.md", "docs/THREAT_MODEL.md"] + sorted(os.path.relpath(p, ROOT) for p in glob.glob(os.path.join(ROOT, "site/docs/*.md")))
    refused = {o: {n for n, v in t.items() if o not in v} for o in OSES}
    for rel in files:
        text = read(rel)
        # "on Windows `--limit cpu`, `file` and `fds` are refused": the named limits must all be refused there.
        for m in re.finditer(r"on (Windows|macOS|Linux)\s+((?:`(?:--limit )?\w+`(?:,| and|, and)? ?)+)\s*(?:is|are) refused", text):
            osn = {v: k for k, v in NAME.items()}[m.group(1)]
            for n in re.findall(r"`(?:--limit )?(\w+)`", m.group(2)):
                if n in t and n not in refused[osn]:
                    errors.append("%s: says %s is refused on %s, but the code enforces it there" % (rel, n, m.group(1)))
        # "`mem` on macOS" style, inside a refusal list.
        for m in re.finditer(r"refused[^.\n]*?:\s*([^.\n]*)", text):
            for n, osn in re.findall(r"`(\w+)`[^`;.]*?\bon (Windows|macOS|Linux)\b", m.group(1)):
                o = {v: k for k, v in NAME.items()}[osn]
                if n in t and n not in refused[o]:
                    errors.append("%s: says %s is refused on %s, but the code enforces it there" % (rel, n, osn))
        # "`threads` (Linux)" / "Linux only": claims the limit is enforced elsewhere.
        for m in re.finditer(r"`(\w+)` \((Linux|macOS|Windows)\)", text):
            n, osn = m.groups()
            o = {v: k for k, v in NAME.items()}[osn]
            if n in t and set(t[n]) != {o} and set(t[n]) != set(OSES):
                errors.append("%s: `%s` (%s) but the code enforces it on %s" % (rel, n, osn, ", ".join(t[n])))


t = table()
check_mcp(t)
check_matrix(t)
check_prose(t)
if errors:
    print("limits_check: the --limit platform claims disagree with LIMIT_PLATFORMS in src/proc.rs:")
    for e in errors:
        print("  - " + e)
    sys.exit(1)
print("limits_check: ok (%d limits)" % len(t))
