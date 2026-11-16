#!/usr/bin/env python3
"""Keeps Brevi's public security statements present, and sandbox-as-boundary claims out.

Every page in REQUIRED must carry the four statements (Adam, 11-16, after an outside review):
  No external security audit yet.
  The sandbox is a policy layer, not an isolation boundary.
  Run untrusted code in a container or VM.
  `--allow-py` and `--allow native` are full trust.
Pages in FUNDING must also carry the audit-funding statement word for word.

Every page in SCANNED (docs, sites, READMEs, skills, package metadata) is read sentence by sentence; a sentence
that puts the sandbox next to isolation, jail or containment words, or calls code "hostile"/"malicious", without a
negation ("not", "no", "never", "do not", ...) fails. Reword the sentence; there is no allowlist.

Run: python3 tools/security_claims.py   (CI: toolchains.yml "versions" job and tools/ci_local.sh)
"""
import glob, html, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

STATEMENTS = {
    "audit": "no external security audit yet",
    "policy": "policy layer, not an isolation boundary",
    "container": "run untrusted code in a container or vm",
    "trust": "--allow-py and --allow native are full trust",
}
FUNDING_TEXT = ("we have not had an external audit; we are seeking funding for one. "
                "until then, do not run hostile code under the sandbox")

REQUIRED = [
    "README.md", "SECURITY.md", "docs/THREAT_MODEL.md",
    "site/index.html", "site/llms.txt", "site/docs/usage.md", "site/docs/faq.md", "site/docs/agents.md",
    "skills/brevi/SKILL.md", "plugins/brevi/skills/brevi/SKILL.md",
    "packaging/pypi/README.md", "packaging/npm/README.md", "packaging/crates/README.md",
    "templates/README.md", "integrations/README.md",
]
FUNDING = [
    "README.md", "SECURITY.md", "docs/THREAT_MODEL.md", "docs/ROADMAP.md",
    "site/index.html", "site/docs/usage.md", "site/docs/faq.md", "site/docs/roadmap.md",
]
SCANNED_GLOBS = [
    "*.md", "docs/**/*.md", "site/**/*.html", "site/**/*.md", "site/**/*.txt",
    "skills/**/*.md", "plugins/**/*.md", "plugins/**/*.json", "packaging/**/*.md", "packaging/**/*.json",
    "packaging/**/*.toml", "templates/**/*.md", "integrations/**/*.md", "devcontainer/**/*.json",
    "server.json", "Cargo.toml",
]
# History, not claims: past changelog entries say what a release did in its own words, and the security-testing
# log records attacks tried against the sandbox.
SKIPPED = {"CHANGELOG.md", "docs/SECURITY_TESTING.md"}
SKIPPED_DIRS = ("site/out", "site/node_modules", "packaging/npm/node_modules", "site/docs/img")

SANDBOX = re.compile(r"sandbox|\bbrevi\b|capability check|allow lines?\b|runtime|program|\bcode\b|\bruns?\b|\beach\b")
BOUNDARY = re.compile(
    r"\bisolat\w*|\bjail\w*|\bcontain(s|ed|ment)? (hostile|malicious|untrusted|adversarial)"
    r"|escape[- ]proof|airtight|bullet[- ]?proof|\bsecure sandbox|\bsecurity boundary"
    r"|\bsafe(ly)?\b.{0,30}\b(untrusted|hostile|malicious)\b|\b(untrusted|hostile|malicious)\b.{0,40}\bsafe(ly)?\b"
    r"|\b(keeps?|protects?) (your|the) (machine|host|system|computer)")
HOSTILE = re.compile(r"\b(hostile|malicious|adversarial) (code|programs?|scripts?)\b")
NEGATION = re.compile(r"\b(not|no|never|nor|without|cannot|until|before)\b|n't\b|\bdo not\b")


def norm(text):
    text = re.sub(r"<!--.*?-->", " ", text, flags=re.S)
    text = re.sub(r"</?[a-zA-Z][^<>]*>", " ", text)
    text = html.unescape(text)
    text = text.replace("`", "").replace("*", "").replace(" ", " ")
    return re.sub(r"\s+", " ", text).lower()


def sentences(text):
    text = re.sub(r"<!--.*?-->", " ", text, flags=re.S)
    text = re.sub(r"</?[a-zA-Z][^<>]*>", "\n", text)
    for line in html.unescape(text).split("\n"):
        for cell in line.split("|"):
            for s in re.split(r"(?<=[.!?])\s+", cell):
                s = re.sub(r"\s+", " ", s.replace("`", "").replace("*", "")).strip().lower()
                if s:
                    yield s


def files():
    seen = set()
    for g in SCANNED_GLOBS:
        for p in glob.glob(os.path.join(ROOT, g), recursive=True):
            rel = os.path.relpath(p, ROOT).replace(os.sep, "/")
            if rel in SKIPPED or rel.startswith(SKIPPED_DIRS) or rel.startswith("changelog.d/") or rel in seen:
                continue
            seen.add(rel)
            yield rel


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
        return f.read()


# The check checks itself on every run, so a regex change that stops catching these fails CI at once.
MUST_FAIL = [
    "Each program isolated",
    "Every program runs in its own sandbox, isolated from the host.",
    "Brevi's sandbox safely runs untrusted code.",
    "The sandbox is a security boundary for agent code.",
    "Run hostile code under --sandbox.",
    "The sandbox keeps your machine safe from agent code.",
    "It contains malicious programs.",
]
MUST_PASS = [
    "The sandbox is a policy layer, not an isolation boundary.",
    "Until then, do not run hostile code under the sandbox.",
    "Run untrusted code in a container or VM.",
    "--allow-py and --allow native are full trust.",
    "It is not built to hold code that is trying to break out.",
]


def claim(s):
    return not NEGATION.search(s) and bool((SANDBOX.search(s) and BOUNDARY.search(s)) or HOSTILE.search(s))


def self_test():
    bad = [s for s in MUST_FAIL if not claim(norm(s))] + [s for s in MUST_PASS if claim(norm(s))]
    for s in bad:
        print("security_claims: self-test misjudges %r" % s)
    return not bad


def main():
    if not self_test():
        return 1
    errors = []
    for rel in REQUIRED:
        t = norm(read(rel))
        for key, phrase in STATEMENTS.items():
            if phrase not in t:
                errors.append("%s: missing the statement %r" % (rel, phrase))
    for rel in FUNDING:
        if FUNDING_TEXT not in norm(read(rel)):
            errors.append("%s: missing the audit-funding statement %r" % (rel, FUNDING_TEXT))
    n = 0
    for rel in files():
        n += 1
        for s in sentences(read(rel)):
            if claim(s):
                errors.append("%s: reads as a sandbox-as-boundary claim: %r" % (rel, s[:200]))
    for e in errors:
        print("security_claims: " + e)
    if errors:
        print("security_claims: %d problem(s). The sandbox is a policy layer, not an isolation boundary; "
              "say so, or reword (SECURITY.md, 'Where Brevi stands')." % len(errors))
        return 1
    print("security_claims: %d files scanned, %d pages carry the security statements" % (n, len(REQUIRED)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
