# Scheduling development during DeepSeek off-peak hours

**Standing rule.** All scheduled or autonomous development work in this repository that consumes DeepSeek API capacity must run during **DeepSeek off-peak hours only**. This covers scheduled agent sessions, batch AI-assisted edits or reviews, benchmark/release runs that call DeepSeek models, and any background job that spends DeepSeek tokens.

## Current window

Source: [DeepSeek API — Models & Pricing](https://api-docs.deepseek.com/quick_start/pricing), checked **12 November 2026**.

- **Peak:** 01:00–04:00 and 06:00–10:00 **UTC**, Monday–Friday, excluding Chinese public holidays.
- **Off-peak:** every other hour — including all day Saturday and Sunday, and Chinese public holidays in full.
- Off-peak rates are half the peak rates.

DeepSeek can change this. If the pricing page differs from this file, **the pricing page wins**: update this file and the rule in `AGENTS.md`, then re-check any active schedules.

## Rule

1. Before creating or editing a schedule for DeepSeek-backed development, compute the **next occurrence in UTC** and check it against the window above. Record the UTC time in the task description.
2. Store recurring schedules with an explicit timezone (prefer `TZID=Australia/Sydney` or UTC) — never a bare local time with no zone.
3. If a recurring schedule would land in peak on any occurrence, move it or split it; do not let it run through peak.
4. If the time is ambiguous, or the provider documentation cannot be checked, treat it as **peak** and defer.
5. No automated exceptions. A human may explicitly override for an urgent release or security fix; record the reason in the task/PR.

Human-initiated interactive work is outside this scheduling rule. The rule constrains what is **scheduled or autonomous**, not what a person chooses to do interactively.

## Sydney example

DeepSeek peak is expressed in UTC, so the local window moves with daylight saving:

| Local zone | Peak (Mon–Fri) | Off-peak |
|---|---|---|
| AEDT (UTC+11, early Nov–early Apr) | 12:00–15:00 and 17:00–21:00 | 00:00–12:00, 15:00–17:00, 21:00–24:00 |
| AEST (UTC+10) | 11:00–14:00 and 16:00–20:00 | 00:00–11:00, 14:00–16:00, 20:00–24:00 |

Weekends and Chinese public holidays are fully off-peak in both zones.

**Existing audit schedule:** the daily Brevi audit runs at **06:00 Australia/Sydney**. That is 19:00 UTC in AEDT or 20:00 UTC in AEST, both off-peak; it does not need to move.

## What this does not cover

- GitHub Actions that do not call DeepSeek (normal build/test CI) are unaffected.
- Release or benchmark workflows that use DeepSeek keys must be triggered at off-peak or gated so they cannot start during peak.
- The compiler itself, local builds, tests and interactive edits do not consume DeepSeek API capacity and are not scheduling-constrained by this rule.
