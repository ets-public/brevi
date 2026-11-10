"""Finds credentials in files before they are committed, uploaded or sent anywhere (the repository is public).

  python3 tools/secret_scan.py              every tracked file (CI runs this)
  python3 tools/secret_scan.py FILE|DIR...  those files (session logs, transcripts, e2e work folders)

Two checks: the shapes of common credentials (Anthropic, OpenAI-style/DeepSeek, GitHub, AWS, Google, Slack,
Replicate tokens, private keys), and, when this process has them, the exact values of every
secret-looking environment variable and the session token file (tools/modelbox.py's list), so a key in any
format is caught on the machine that holds it. Prints file:line and the kind, never the value. Exit 1 on a hit.
A line that must hold a credential-shaped example ends with `secret-scan: allow`.
"""
import os, re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SECRET_NAME = re.compile(r"KEY|TOKEN|SECRET|PASSW|CREDENTIAL|AUTH|COOKIE", re.I)


def secret_values():
    """This machine's secret values (the same list as tools/modelbox.py): every secret-looking variable that is
    not a path, and the session token file."""
    vals = {v for k, v in os.environ.items() if SECRET_NAME.search(k) and len(v) >= 12 and not v.startswith("/")}
    tok = os.environ.get("CLAUDE_SESSION_INGRESS_TOKEN_FILE")
    try:
        t = Path(tok).read_text().strip() if tok else ""
        if len(t) >= 12:
            vals.add(t)
    except OSError:
        pass
    return sorted(vals, key=len, reverse=True)


SECRET_VALUES = secret_values()

SHAPES = [
    ("Anthropic key", r"sk-ant-[A-Za-z0-9_-]{20,}"),
    ("OpenAI-style key (OpenAI, DeepSeek)", r"\bsk-(?:proj-)?[A-Za-z0-9]{32,}\b"),
    ("GitHub token", r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,}\b|\bgithub_pat_[A-Za-z0-9_]{60,}"),
    ("AWS access key id", r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"),
    ("Google API key", r"\bAIza[0-9A-Za-z_-]{35}\b"),
    ("Google OAuth token", r"\bya29\.[0-9A-Za-z_-]{20,}"),
    ("Slack token", r"\bxox[abposr]-[0-9A-Za-z-]{10,}"),
    ("Replicate token", r"\br8_[A-Za-z0-9]{30,}\b"),
    ("private key", r"-----BEGIN (?:RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY-----"),
]
SHAPE_RE = [(k, re.compile(p)) for k, p in SHAPES]
ALLOW = "secret-scan: allow"
# files that hold a credential-shaped test fixture on purpose
ALLOW_FILES = {"tests/http/key.pem"}  # self-signed key for the local HTTPS test server (tests/http/server.py)


def scan_text(text, where):
    hits = []
    for n, line in enumerate(text.splitlines(), 1):
        if ALLOW in line:
            continue
        for kind, rx in SHAPE_RE:
            if rx.search(line):
                hits.append(f"{where}:{n}: {kind}")
        for v in SECRET_VALUES:
            if v in line:
                hits.append(f"{where}:{n}: the value of one of this machine's secret variables")
                break
    return hits


def files(args):
    if not args:
        out = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, text=True, check=True).stdout
        return [ROOT / p for p in out.split("\0") if p and p not in ALLOW_FILES]
    res = []
    for a in args:
        p = Path(a)
        res += [q for q in p.rglob("*") if q.is_file()] if p.is_dir() else [p]
    return res


def main():
    hits = []
    fs = files(sys.argv[1:])
    for f in fs:
        try:
            b = f.read_bytes()
        except OSError:
            continue
        if b"\0" in b[:8192]:
            continue  # binary
        hits += scan_text(b.decode("utf-8", "replace"), f.relative_to(ROOT) if f.is_relative_to(ROOT) else f)
    for h in hits:
        print(h)
    print(f"{len(fs)} files scanned: " + ("no credentials" if not hits else f"{len(hits)} hit(s)"))
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
