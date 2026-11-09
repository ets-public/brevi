# Governance, policy and the weekly release gate

## 1. Add to `CONTRIBUTING.md`

> ### What we are optimising
>
> Brevi is trying to become the default runtime for AI-written Python. That means correctness you can check, an inner loop you don't notice, and distribution through harnesses and sandboxes — not the largest possible list of native libraries.
>
> ### Changes we will not merge before 1.0
>
> - **New byte-parity library ports** (polars, duckdb, onnxruntime, safetensors, extra jinja2 features, …) unless an alpha user has a written need and a named maintainer owns the upkeep. We maintain the current set and bridge the rest.
> - **New `.bv` language features** beyond bug fixes. The Python path is the product; the language is frozen for now.
> - **New benchmarks chosen to flatter the headline.** Benchmarks are added to answer a question, with the question written in the PR.
>
> ### Every PR
>
> - Adds a test in `tests/` that fails before and passes after.
> - Updates `CHANGELOG.md` under "Unreleased".
> - Touches no public claim without updating `docs/CLAIMS.md` and naming the artifact.
> - Does not raise the known-wrong conformance budget. A PR that adds a known-wrong entry needs a written reason and a plan to remove it.
>
> ### Public claims
>
> No absolute correctness language ("exactly", "never differs", "always"). Use "certified by `brevi verify`", "matches on our scored corpus", or a link to the conformance report. Every number links to its artifact, date and hardware.

## 2. Weekly release gate

A release ships only if **all** boxes are ticked. A missed Monday is cheaper than a broken one.

- [ ] `sh tools/ci_local.sh` passes.
- [ ] `cargo test` passes.
- [ ] Conformance CI green; known-wrong budget not raised; `compat/conformance_known_wrong.txt` updated deliberately.
- [ ] Latency job within thresholds (or a written, reviewed regression note).
- [ ] Release test run and within the bar (or a written note explaining the miss).
- [ ] No open P0: silent wrong answer, panic, sandbox escape, broken install.
- [ ] Claims audit: `docs/CLAIMS.md` matches the site, README, registries, `llms.txt`, skill, MCP registry. No absolute language anywhere.
- [ ] Install test on a clean machine/container: hosted path from zero, and local path with Rust, both to "hello world" printing.
- [ ] Release notes generated from `docs/CLAIMS.md`, not from memory.
- [ ] Versions bumped; checksums produced; artifacts signed/published; `SHA256SUMS` present.
- [ ] Metric table updated in the roadmap (ANER, conformance, known-wrong, typed rate, check p50, first-attempt, tokens-to-green).

## 3. Metric review (every release, 10 minutes)

Fill this in the release PR:

| Metric | Last release | This release | Target | Trend |
|---|---:|---:|---:|---|
| ANER (alpha traffic) | | | 60% @ 90d | |
| Conformance | 58.2% | | ≥70% @ 90d | |
| Known-wrong pairs | 88 | | 0 @ 1.0 | |
| `check --fast` p50 | | | ≤20 ms | |
| `check` p50 | 144 ms | | ≤50 ms | |
| Cold build p50 | 1.2 s | | ≤500 ms | |
| Typed rate (813 corpus) | 567/813 | | ≥80% | |
| First-attempt native | unknown | | ≥80% | |
| Tokens-to-green (errors surfaced) | unknown | | ≤1.10x | |
| Third-party integrations | 0 | | 3 @ 6mo | |

**Rule:** if a target misses two releases in a row, cut scope — do not move the target. Record the cut in the decision log.

## 4. Incident policy (regressions)

| Severity | Definition | Response |
|---|---|---|
| P0 | Silent wrong answer on a real program, sandbox escape, panic reachable from ordinary Python, broken install | Patch release within 48 h; add the program to the conformance or regression corpus; public note; postmortem in `docs/internal/incidents/`. |
| P1 | Wrong answer only in a claimed-native path but caught by `verify`, or a latency regression | Fix in the next weekly release; add a test. |
| P2 | Refusal regression, docs error, non-security limit gap | Next scheduled release. |

Every P0 adds a line to the known-wrong or regression corpus **before** the fix, so the same bug cannot return silently.

## 5. Claims audit script (cheap, monthly)

```bash
# 1. Find absolute language in public docs
grep -rniE "exactly what CPython|never (builds|differs|answers)|always matches|no silent differences" \
  site docs README.md llms.txt skills 2>/dev/null

# 2. Find speed ranges and check them against docs/CLAIMS.md
grep -rnoE "[0-9]+(\.[0-9]+)? ?(x|to|–|-)[0-9]+(\.[0-9]+)?x?" site docs README.md llms.txt | sort -u

# 3. Confirm the conformance number on the site equals the latest report
python3 - <<'PY'
import re, pathlib
report = pathlib.Path("bench/conformance/report.md").read_text()
m = re.search(r"Coverage \*\*([\d.]+)%\*\*", report)
print("report coverage:", m.group(1) if m else "MISSING")
PY
```

The audit output goes in the release PR. If a number changed, the PR that changes the docs links the artifact.
