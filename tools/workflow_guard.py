"""Fails CI when a GitHub Actions workflow opens a way in for untrusted code or text (the repository is public).

Rules, for every file in .github/workflows/:
- No `pull_request_target`, `workflow_run` or `issue_comment` trigger: they run with the repository's secrets and a
  write token on events anyone can cause, which is how most Actions takeovers start.
- A workflow that runs on `pull_request` (code from forks) uses no `secrets.`: fork PRs never get them, and a
  same-repo PR must not either, since the PR's own code runs in that job.
- A workflow that runs on `issues` (text anyone can write) gates its jobs on the author (`author_association`).
- Every third-party action is pinned to a full commit SHA (a tag or branch can be moved to new code by whoever
  controls that repository; Dependabot keeps the SHAs current).
- No `${{ }}` expression in a `run:` script expands text a person outside the project can choose (event payloads,
  dispatch inputs, branch names): pass it through `env:` and quote "$VAR", so it cannot become shell code.
Standard library only (it runs before any setup step).
"""
import re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BANNED_TRIGGERS = ("pull_request_target", "workflow_run", "issue_comment")
# expression contexts safe to expand in a script: set by the workflow itself or by GitHub, not by an outsider
SAFE_EXPR = re.compile(r"^\s*!?\s*(matrix\.|steps\.[\w-]+\.outputs\.|needs\.[\w-]+\.(outputs|result)|runner\.|env\.|"
                       r"github\.(event_name|sha|run_id|run_number|run_attempt|repository|repository_owner|workspace|server_url|job|ref)\b(?!_)|"
                       r"hashFiles\(|success\(\)|failure\(\)|always\(\)|cancelled\(\))")


def triggers(text):
    """Names of the events under the top-level `on:` (block or inline list form)."""
    m = re.search(r"^on:\s*(.*)$", text, re.M)
    if not m:
        return set()
    if m.group(1).strip():
        return set(re.findall(r"[a-z_]+", m.group(1)))
    out = set()
    for line in text[m.end():].splitlines()[1:] if text[m.end():].startswith("\n") else text[m.end():].splitlines():
        if line.strip() and not line.startswith(" "):
            break
        k = re.match(r"^  ([a-z_]+):", line)
        if k:
            out.add(k.group(1))
    return out


def run_scripts(text):
    """(line number, script text) of every `run:` step."""
    lines = text.splitlines()
    out = []
    for i, line in enumerate(lines):
        m = re.match(r"^(\s*)(?:- )?run:\s*(.*)$", line)
        if not m:
            continue
        indent = len(m.group(1)) + (2 if line.lstrip().startswith("- ") else 0)
        body = [m.group(2)]
        if m.group(2).strip() in ("|", ">", "|-", ">-"):
            body = []
            for nxt in lines[i + 1:]:
                if nxt.strip() and len(nxt) - len(nxt.lstrip()) <= indent:
                    break
                body.append(nxt)
        out.append((i + 1, "\n".join(body)))
    return out


def check(path):
    text = path.read_text()
    name = path.relative_to(ROOT)
    errs = []
    on = triggers(text)
    for t in BANNED_TRIGGERS:
        if t in on:
            errs.append(f"{name}: the `{t}` trigger runs with secrets on events anyone can cause")
    if "pull_request" in on and re.search(r"secrets\.", text):
        errs.append(f"{name}: runs on pull_request (the PR's code) and uses secrets")
    if "issues" in on and "author_association" not in text:
        errs.append(f"{name}: runs on issues (anyone's text) without gating on author_association")
    for i, line in enumerate(text.splitlines(), 1):
        m = re.search(r"uses:\s*([^\s#]+)", line)
        if m and not m.group(1).startswith(("./", "docker://")) and not re.search(r"@[0-9a-f]{40}$", m.group(1)):
            errs.append(f"{name}:{i}: `{m.group(1)}` is not pinned to a commit SHA")
    for ln, script in run_scripts(text):
        for e in re.findall(r"\$\{\{(.*?)\}\}", script):
            if not SAFE_EXPR.match(e):
                errs.append(f"{name}:{ln}: `${{{{{e.strip()}}}}}` is expanded into a shell script; pass it through env: instead")
    return errs


def main():
    files = sorted((ROOT / ".github" / "workflows").glob("*.y*ml"))
    errs = [e for f in files for e in check(f)]
    for e in errs:
        print(e)
    print(f"{len(files)} workflows checked: " + ("ok" if not errs else f"{len(errs)} problem(s)"))
    return 1 if errs else 0


if __name__ == "__main__":
    sys.exit(main())
