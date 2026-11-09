# Execution playbook (internal)

**Purpose:** make the roadmap survive contact with weekly releases. Two humans + Claude, seven workstreams, one north-star metric. The failure mode is not lack of ideas; it is parallel work, skipped Phase 0, and scope creep from library ports.

## 1. Roles

| Role | Owns | Does not own |
|---|---|---|
| **Maintainer (you)** | Compiler semantics, `typed_guard.rs`, the exactness contract, final sign-off on every public claim | CI plumbing, benchmark maintenance |
| **Second human** | Conformance suite, CI gates, release gate, security triage, benchmark integrity | Compiler internals (unless they choose to) |
| **Claude (agent)** | Tests, fuzzing, benchmark scripts, docs consistency, migrations of known-wrong fixes, changelog drafts | Semantic sign-off, public claims, production releases |
| **External (later)** | Security audit, independent reproduction | — |

If the second human is not yet committed to the semantic surface, make that explicit now: the single most valuable review is `typed_guard.rs` + conformance triage. Infrastructure review is second.

## 2. Cadence

- **Daily:** CI (tests, toolchain matrix, conformance nightly once `CONFORMANCE_CI.md` lands).
- **Weekly (Monday):** release only if the release gate passes (`RELEASE_GATE.md`). A missed release is cheaper than a broken one.
- **Per release:** update the metric table in the roadmap (ANER, conformance, known-wrong, typed rate, check p50, first-attempt, tokens-to-green). If a target misses two releases in a row, **cut scope**, not the target.
- **Monthly:** claims audit — diff `docs/CLAIMS.md` against the live site, README, PyPI/npm/crates descriptions, `llms.txt`, skill and MCP registry.
- **Quarterly (or at phase gates):** re-read the roadmap and kill workstreams that are not moving the north star. A roadmap whose numbers do not move is a wish list.

## 3. Phase 0 — the first four weeks

### Week 1 — tell the truth and instrument
- [ ] Land conformance CI (`CONFORMANCE_CI.md`) (nightly + budget).
- [ ] Apply `../CLAIMS.md` to every surface; remove all absolute correctness claims and the "10–50x" line.
- [ ] Commit `bench/latency.py` (see `LATENCY.md`); record p50/p95 baselines in the repo.
- [ ] Recruit 10 alpha users (see `ALPHA.md`).

### Week 2 — fuzz and burn down
- [ ] Fuzz harness (generator + differential runner). Triage the 5 crash/timeout cases and the 2 signals; a signal is a P0 until explained.
- [ ] Fix the top 5 known-wrong entries; publish the burn-down in `docs/CLAIMS.md`.
- [ ] Write the verify-by-default design doc (`VERIFY_BY_DEFAULT.md`): certificate schema, fallback rules, MCP default.

### Week 3 — latency spike
- [ ] Measure `brevi check --fast` without `rustc`; set the real baseline.
- [ ] Daemon spike; write the go/no-go criteria for a Cranelift/JIT dev backend.
- [ ] Design the SDK + `python3` shim; build the first base-image recipe.

### Week 4 — alpha and evidence
- [ ] Private alpha live with opt-in metrics (explicit consent; no default telemetry).
- [ ] Real-task benchmark v1 plan (one public agent benchmark subset, with/without Brevi).
- [ ] Publish the metric dashboard: conformance, known-wrong, p50 check, typed rate, first-attempt, tokens-to-green.
- [ ] Start conversations with two harness vendors and two sandbox platforms.

## 4. How decisions get made

Keep a decision log (append-only, one row per decision):

| Date | Decision | Options considered | Rationale | Revisit |
|---|---|---|---|---|
| 2026-11-09 | Verify-by-default is the trust contract | more benchmarks vs conformance CI vs per-run verify | only per-run verify is a guarantee a harness can rely on | at public beta |
| 2026-11-09 | Freeze new byte-parity library ports | keep adding polars/duckdb/onnx vs freeze | maintenance surface outgrows two humans; low adoption pull | if an alpha user proves a must-have |

Rules:
- **No new library port before 1.0** without an alpha user's written justification and a maintenance owner.
- **No public number** that is not in `docs/CLAIMS.md` with an artifact link.
- **No absolute correctness language** anywhere: "exact", "never differs", "always" are banned; use "certified by `brevi verify`" or "matches on our scored corpus".
- **A regression outranks a feature**: a silent wrong answer or a panic stops the release and gets a patch release.

## 5. Definition of "done" for Phase 0

- Conformance runs in CI and is linked from the README.
- Every public claim matches an artifact.
- p50/p95 check/build/run baselines exist in the repo and are tracked per release.
- 10 alpha users are running weekly.
- Verify-by-default has a design and an owner.
- The stop-doing list is in `CONTRIBUTING.md`.

If any of these is missing after four weeks, the roadmap's phases slip — and that is acceptable; shipping a false claim is not.
