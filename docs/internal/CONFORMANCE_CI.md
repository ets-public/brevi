# Conformance in CI (and as a release gate)

**Problem this fixes:** the conformance suite is the most independent correctness measure Brevi has, and today it is run by hand, is not in CI, and its 44 known-wrong results are allowlisted. That makes the strongest evidence invisible and lets it regress silently.

**What "pass" means:** two independent conditions.
1. `tools/conformance.py --check` must not find a wrong result that is missing from `compat/conformance_known_wrong.txt`.
2. The number of allowlisted results must not grow. Track it as a **budget that only goes down**: start at the current 88 test/mode pairs and lower it every release; raising it requires a reviewed change with a written reason.

## Gates

| Gate | Trigger | Blocking? | Runtime |
|---|---|---|---|
| PR check | PRs touching `src/**`, `Cargo.lock`, `tools/conformance.py`, the allowlist | Yes | ~10 min |
| Nightly | 03:00 UTC, every day | Reports only (failure opens a `conformance` issue) | ~10 min |
| Release gate | `workflow_call` from the release workflow, and a manual run before tagging | Yes | ~10 min |

## Files

- `.github/workflows/conformance.yml` → `.github/workflows/conformance.yml`
- Report artifact: `bench/conformance/` (uploaded, 90-day retention)
- Job summary: the whole `report.md` is appended to `$GITHUB_STEP_SUMMARY`, so the number is visible on the Actions page without download.
- Later: a small GitHub Pages job that renders the report and the budget trend. Do not block on it.

## Wiring into the release workflow

In `.github/workflows/release.yml`, add a job and make the release depend on it:

```yaml
jobs:
  conformance:
    uses: ./.github/workflows/conformance.yml

  release:
    needs: [conformance]
    # ...existing release job...
```

Also add the budget to the release checklist (`RELEASE_GATE.md`): a release does not ship with a higher known-wrong count than the previous one.

## Why budget + `--check` and not "any failure fails the job"

The allowlist exists because the project knowingly ships some divergences; failing on all of them today would block every release without improving anything. The budget makes the *direction* non-negotiable: the project can ship with 88 pairs today, but it cannot ship with 90, and it must reach 0. `--check` already catches anything new that was not consciously accepted.

## Rollout notes

- Conformance downloads and pins a CPython tag; cache `~/.cache/brevi/conformance` between runs (the tool verifies pinned hashes).
- Use a 4-core runner; the tool runs four tests at a time. On a 2-core runner, raise the timeout.
- If the runner's `python3` is not 3.13.13, pass `--python /path/to/python3.13`; the tool stops if the reference interpreter does not match the tag.
- Nightly failures open a `conformance` issue with the report attached; a failure that persists two nights is a P0.
- Keep `tools/conformance.py --check` the single gate. Do not reimplement the comparison elsewhere.

## Definition of done

- [ ] Nightly run is green or opens an issue.
- [ ] PR gate blocks new wrong results.
- [ ] Release workflow depends on the gate.
- [ ] Budget is a named env var in the workflow and is lowered in the release commit.
- [ ] The report is linked from `README.md` and from the site's compatibility page (see `../CLAIMS.md`).
