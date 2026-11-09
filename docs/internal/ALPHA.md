# Private alpha — one page

**Goal:** 10 teams using Brevi weekly for six weeks, before any public launch, to produce the real-agent traffic that the roadmap's metrics need (ANER, refusal histograms, first-attempt rate, tokens-to-green).

## What you get

- Early access to weekly builds and the hosted endpoint (no Rust toolchain needed).
- A direct channel to the maintainer; your bugs jump the queue.
- `brevi verify` certificates for the programs you run — the native build and CPython checked on your inputs.
- Your use case shapes the typed-path priorities (we publish the refusal histogram we build from alpha traffic, not from our own tasks).

## What we need

- Use Brevi on at least a few real tasks per week, in whatever harness you already use (or our sandbox image).
- Opt in to sharing: (a) whether each program built natively, which path it took, and how long check/build/run took; (b) the *sanitised* source of programs that fail, as a regression corpus. Nothing else. No default telemetry, no account, no phone-home.
- A 20-minute call every two weeks; one short written report at the end.
- Tolerance for breakage: weekly releases, occasional wrong answers (reported immediately), CLI changes.

## What we will not do

- Read your data, prompts, or non-Brevi programs.
- Share your name or programs without permission.
- Use your programs for anything except the test corpus and bug fixes.

## Consent text (paste into the signup)

> I agree to let Brevi use (1) build/run statistics from my alpha usage and (2) source code of failing programs I explicitly submit, for the purpose of fixing Brevi and building its regression corpus. I can withdraw and have my data deleted at any time by emailing support@brevi-lang.org. No telemetry is collected by default.

## Invitation template

> Subject: Brevi alpha — 10 teams, six weeks
>
> Hi — I'm building Brevi, a compiler that turns the plain Python an AI agent writes into a native binary through Rust, with a per-program check against CPython (`brevi verify`). It's fast on compute-heavy code (median ~8x on our benchmarks) and it refuses rather than guesses when it can't match Python.
>
> I'm looking for 10 teams running agent-written Python to use it weekly for six weeks, tell me where it breaks, and let me use sanitised failing programs as a regression corpus. You get the hosted endpoint (no Rust install), weekly builds, and a direct line to me.
>
> If you're interested, reply and I'll send the 10-minute setup. If not, no follow-up.

## Alpha exit criteria (what you are deciding at week 6)

- 10 teams active; ≥5 running weekly.
- ≥300 real agent programs observed; ≥95% with a known path and match status.
- A refusal histogram with the top 10 causes owned.
- ≥3 verified bugs from alpha traffic fixed.
- A go/no-go: public beta only if install ≤60 s, check p50 ≤50 ms, verify-by-default usable, and the claims audit is live.
