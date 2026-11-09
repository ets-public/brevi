# Claims source of truth

**Rule:** one number, one source. Every public number lives here with an artifact link and a date. If a surface (homepage, docs, `llms.txt`, README, PyPI, npm, crates, MCP registry, the skill, a release note) needs a number, copy it from here. If a number has no artifact link, it is not published.

**Rule:** no absolutes. The words *exactly*, *never*, *always* and *no silent differences* are banned unless the sentence is scoped to `brevi verify` on the reader's own inputs. The honest product claim is: **"matches CPython on the programs we certify, refuses what it detects, and publishes the failures it knows about."**

**Rule:** date and hardware. Every timing gets a machine, a date, a version and a "best of N".

---

## 1. Claim register

| # | Claim as currently published | Where | Why it fails | Corrected claim | Evidence |
|---|---|---|---|---|---|
| 1 | "runs **2 to 40 times faster** on our benchmarks" | homepage | measured default is 1.7–39.5x; 40x needs `--release` | "runs **1.7x to 39.5x faster** on our 12 compute benchmarks, median **8.5x**; up to **44x** with `--release`" | `bench/compare/README.md` |
| 2 | "plain loops over ints and lists **10 to 50 times faster**" | `llms.txt`, skill, MCP prompt | false at both ends (7/12 below 10x, none at 50x) | "**1.7x to 39.5x faster**, median 8.5x, on our compute benchmarks" | same |
| 3 | "prints **exactly what CPython prints**, or Brevi refuses to build it" | compatibility | 44/311 CPython tests build but differ | "on the subset Brevi models, it matches CPython's deterministic stdout and exit status or refuses; `brevi verify` certifies this per program on your inputs" | `bench/conformance/report.md`; `brevi verify` |
| 4 | "It **never** builds a program that might answer differently" | homepage | contradicted by 44 known divergences | "When Brevi cannot match CPython for a piece of Python it detects, it refuses at build time. Divergences it does not detect yet are tracked and published: 44 tests of CPython's scored suite currently build but differ." | `bench/conformance/report.md` |
| 5 | "**591 of 591** same output as CPython" | homepage | true only for a self-tuned regression corpus | "**591 of 591 saved solutions** from our own benchmark tasks match CPython byte for byte (a regression corpus, not an independent sample)" | `bench/compare/README.md` |
| 6 | "Codon builds **36 of them with wrong output**" | homepage | 36 of the 206 Codon built, not of 591 | "Codon builds **206 of the 591** unchanged and **36 of those 206** print something different; it refuses 385" | `bench/compare/README.md` |
| 7 | "Model cost: **1.07x / 1.00x**" presented as parity | homepage | the bar allows +10%; the point estimate is worse | "within the release-test bar of ±10%: **1.07x Sonnet, 1.00x Haiku** (mean over published runs 1.05x); no saving on ordinary tasks" | `bench/weekly.json`, benchmarks page |
| 8 | "**0.58x / 0.53x** on heavy tasks" | homepage | produced by an extra prompt line; no CPython+hint control | "with the plain-solution instruction in the prompt, 0.58x/0.53x on four heavy tasks; this is a prompting effect plus a compiler capability, not measured against a CPython+hint control" | `bench/README.md` |
| 9 | "**92 to 98%** of final programs native" | compatibility | stale; v0.26.0 reports 96/96 | "**92–100%** of final programs native across recent runs; v0.26.0: 96 of 96" | README, benchmarks |
| 10 | "**One binary**" | homepage table | library programs need `libpython`; `--standalone` Linux-only; Windows library programs run CPython | "one native binary for stdlib-only programs; library programs can embed CPython with `--standalone` (Linux); on Windows library programs run on CPython for now" | compatibility, platforms |
| 11 | "**Only the caller grants access**" presented as containment | homepage | in-process policy layer; no seccomp/VM; no external audit; Linux-only kernel layer | "a capability policy layer: the program gets only what the caller grants, checked before every build path; for hostile code, use a container or VM (see threat model)" | `docs/THREAT_MODEL.md` |
| 12 | "open source under Apache-2.0" with "opens with the public release" | homepage | stale; repo is public now | "Apache-2.0 now: github.com/ets-public/brevi" | repo API |
| 13 | "Can I install with pip/npm/Homebrew? **Not yet** … nothing is published" | FAQ | pip, npm, crates are published | "pip, npm and crates.io are published; Homebrew is coming" | registries |
| 14 | "**Every number can be rerun**" | benchmarks | mostly true; competitor columns mixed sessions; model cost needs a key | "scripts and raw logs are public; the Codon/Nuitka columns were timed in another session and are not directly comparable; model-cost runs need an API key" | `bench/` |
| 15 | "Please report a difference, it is a bug" | FAQ | good, but implies rarity | keep, and add the current known count and the link | claimed |

---

## 2. Ready-to-paste homepage copy

**Hero heading**

> Brevi compiles the plain Python your agent writes into a native binary through Rust.

**Hero subheading**

> On our 12 compute benchmarks the default build runs 1.7–39.5x faster than CPython (median 8.5x). Every build is checked against CPython: when Brevi cannot match the Python it models, it refuses at build time, and `brevi verify` certifies a program's output against CPython on your own inputs. What it cannot build yet runs on `python3`, and says so in one line.

**Buttons:** Get started · Set up an agent · GitHub

**Four cards**

> **Faster on compute.** 1.7–39.5x on our 12 benchmarks, median 8.5x; up to 44x with `--release`. The fast path is the typed build; every program says which path it took. → Benchmarks
>
> **Verified per program.** `brevi verify` runs the native build and CPython on the same inputs and prints a certificate when stdout and the exit code agree. On CPython's own scored test suite, 181 of 311 tests match exactly today; the 44 that differ are tracked in the open and burned down each release. → Conformance report
>
> **Cheap to iterate.** One-line JSON errors; an MCP server that keeps the program so a fix costs only the changed lines; `brevi check` in about 144 ms today (target: under 50 ms). On our release test, cost is within 10% of CPython (1.07x Sonnet, 1.00x Haiku). → For agents
>
> **One binary where it fits.** Stdlib-only programs build to a single native binary; library programs can embed CPython with `--standalone` (Linux). Under `--sandbox`, a program gets only what the caller grants — a policy layer, not a VM; see the threat model. → Compatibility

**Benchmark summary**

> Twelve compute-dense Python programs, best of 3, one 4-core Linux machine, 3 November 2026. Default build: median 8.5x, range 1.7–39.5x. `--release` is level with Codon where Codon builds and faster on object-, string- and set-heavy code. Nuitka matches CPython exactly and runs at about CPython's speed. Raw logs and scripts are in `bench/`.

**Conformance summary (new section, link from homepage)**

> We test Brevi against CPython's own test suite. Today 181 of 311 scored tests print exactly what CPython prints from a native build, 66 are refused at build time, and 44 build but differ (18 in output, 26 in exit status). The known divergences are listed in the repository and are worked down every release. For your program, `brevi verify` is the contract.

---

## 3. FAQ replacements

**Will my program give the same answer?**
> On the subset Brevi models, yes — or it refuses. We do not yet model all of Python: on CPython's own scored tests, 58.2% match exactly and 44 build but differ today; those are tracked in the open. For your program, run `brevi verify` with your inputs: it runs the native build and CPython, compares stdout and the exit code, and prints a certificate when they agree. That per-program check is the guarantee we stand behind.

**What happens when Brevi cannot build my file?**
> `brevi run` runs it with `python3` and prints one line saying which line stopped Brevi. `brevi build`, `brevi check` and `brevi run --native` print the errors instead. Under `--sandbox` there is no CPython fallback: a program Brevi cannot build is refused.

**Does it cost more tokens than Python?**
> About the same on ordinary tasks: 1.07x (Sonnet) and 1.00x (Haiku) in the v0.26.0 release test, whose bar is within 10%. On four heavy tasks, telling the model its code is compiled and to write the plain solution cut cost to 0.58x/0.53x — that is a prompting effect plus the compiler making plain code fast enough, and it is measured without a CPython-plus-hint control.

**How is this different from Codon, Nuitka, Cython or PyPy?**
> Codon compiles a Python-like language and is faster than CPython on math-heavy code, but changes Python's behaviour and builds 206 of our 591 saved solutions unchanged (36 of those 206 print something different; it refuses 385). Nuitka matches CPython exactly and runs at about CPython's speed; it is mature and supports arbitrary Python. Cython needs type annotations. PyPy is a JIT, not a single native binary. Brevi's bet is exact-or-refuse plus speed plus an agent-shaped interface; `brevi verify` is how you check it on your code.

**Is it safe to run code an agent wrote?**
> Brevi programs are memory-safe Rust and integers do not silently wrap. Under `--sandbox`, only the caller grants files, network, native code or CPython; a program's own `allow` lines grant nothing, and a refused program never falls back to CPython. That is a policy layer, not an operating-system jail: there is no seccomp, namespace or VM, CPython under `--allow-py` is full trust, and no outside security review has been done yet. For code you have reason to distrust, run Brevi inside a container or VM as well. See the threat model.

**Which platforms?**
> Linux, macOS and Windows on x86-64 and ARM64 (Windows x86-64 only). Library calls through CPython and `--standalone` are Linux/macOS only for now; on Windows, programs that import packages run on CPython.

**Is it free?**
> The compiler is Apache-2.0 and will stay that way. A hosted API and paid business extras are planned but not built.

**Who makes Brevi?**
> Two people, working with AI coding agents (Claude), with every change reviewed and tested before release.

---

## 4. `llms.txt` / skill line (replace the current sentence)

> Brevi compiles plain Python to a native binary through Rust. On our 12 compute benchmarks the default build runs 1.7–39.5x faster than CPython (median 8.5x). Write the plain, direct solution; do not hand-optimise it. When Brevi cannot match CPython it refuses at build time, and `brevi verify` checks a program against CPython on your inputs.

---

## 5. Surfaces to update in the same commit

- [ ] Homepage (`site/`) — hero, cards, benchmark section, conformance section
- [ ] `/docs/benchmarks` — range, median, competitor session caveat
- [ ] `/docs/compatibility` — "In numbers" table; add conformance; fix 92–98%
- [ ] `/docs/faq` — the answers above
- [ ] `/docs/install` — remove "private until the public release"; publication status
- [ ] `/docs/architecture` — replace "as of v0.24" with the current version, or remove the claim
- [ ] `/llms.txt` and `/llms-full.txt`
- [ ] `README.md`
- [ ] PyPI description, npm description, crates description, MCP registry entry
- [ ] `skills/brevi/SKILL.md` and editor rules
- [ ] GitHub release notes template (pull from here, not from memory)

**Sign-off:** every row above needs a named owner and a date. A claim without an owner drifts again within a month.
