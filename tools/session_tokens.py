#!/usr/bin/env python3
"""Token use per Claude Code session, read from the transcripts Claude Code keeps on disk.

Every call re-sends the whole conversation, so tokens ~ calls x context per call. This prints both,
counting each request once (a request's usage is copied onto each of its content records).

  session_tokens.py [--last N]            the N most recent sessions on this machine (default 10)
  session_tokens.py --record [--pr N]... [--task TEXT] [--out DIR]
                                          save this session's totals (sub-agents included) as one JSON
                                          file in DIR (default /mnt/project-files/tokens/); threads run
                                          this before they resolve
  session_tokens.py --report [DIR]        sum the saved records: tokens per merged PR, weighted by price

"weighted" prices each kind of token relative to fresh input: cache read 0.1, cache write 1.25 (5-minute
cache) or 2 (1-hour cache), output 5. It is the cost signal; the raw sum is dominated by cheap cache reads.
Records hold only counts, the session id, the branch and PR numbers: no transcript text.
"""
import argparse, datetime, glob, json, os, statistics, sys

ROOT = os.path.expanduser("~/.claude/projects")
DEFAULT_OUT = "/mnt/project-files/tokens"
KINDS = ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens", "output_tokens")


def weighted(u):
    cw = u.get("cache_creation") or {}
    w1h = int(cw.get("ephemeral_1h_input_tokens") or 0)
    w5m = int(u.get("cache_creation_input_tokens") or 0) - w1h
    return (int(u.get("input_tokens") or 0) + 0.1 * int(u.get("cache_read_input_tokens") or 0)
            + 1.25 * w5m + 2 * w1h + 5 * int(u.get("output_tokens") or 0))


def summarize(paths):
    """Totals over the transcript files of one session (main file plus sub-agent files)."""
    seen, tot, ctx, branch, w = set(), dict.fromkeys(KINDS, 0), [], "", 0.0
    for path in paths:
        for line in open(path, encoding="utf-8", errors="ignore"):
            try:
                rec = json.loads(line)
            except ValueError:
                continue
            branch = rec.get("gitBranch") or branch
            msg = rec.get("message") if isinstance(rec.get("message"), dict) else {}
            u, rid = msg.get("usage"), rec.get("requestId")
            if not isinstance(u, dict) or not rid or rid in seen:
                continue
            seen.add(rid)
            for k in KINDS:
                tot[k] += int(u.get(k) or 0)
            ctx.append(sum(int(u.get(k) or 0) for k in KINDS[:3]))
            w += weighted(u)
    return {"calls": len(seen), **tot, "weighted": round(w), "first_context": ctx[0] if ctx else 0,
            "median_context": round(statistics.median(ctx)) if ctx else 0, "max_context": max(ctx, default=0),
            "branch": branch}


def session_files(main):
    sub = os.path.join(main[:-len(".jsonl")], "subagents")
    return [main] + sorted(glob.glob(os.path.join(sub, "*.jsonl")))


def all_sessions():
    return sorted((p for p in glob.glob(f"{ROOT}/*/*.jsonl")), key=os.path.getmtime)


def cmd_last(n):
    print(f"{'session':36}  {'calls':>5}  {'median ctx':>10}  {'max ctx':>8}  {'output':>8}  {'weighted':>10}")
    for p in all_sessions()[-n:]:
        s = summarize(session_files(p))
        if s["calls"]:
            print(f"{os.path.basename(p)[:36]:36}  {s['calls']:5}  {s['median_context']:10,}  {s['max_context']:8,}"
                  f"  {s['output_tokens']:8,}  {s['weighted']:10,}")


def cmd_record(prs, task, out):
    sid = os.environ.get("CLAUDE_CODE_SESSION_ID", "")
    mains = [p for p in all_sessions() if sid and os.path.basename(p).startswith(sid)] or all_sessions()[-1:]
    if not mains:
        sys.exit("no Claude Code transcript found under " + ROOT)
    main = mains[-1]
    s = summarize(session_files(main))
    s.update(session=os.path.basename(main)[:-len(".jsonl")], prs=prs, task=task,
             recorded=datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))
    os.makedirs(out, exist_ok=True)
    dest = os.path.join(out, f"{s['recorded'][:10]}-{s['session']}.json")
    with open(dest, "w") as f:
        json.dump(s, f, indent=1)
    print(f"{dest}: {s['calls']} calls, median context {s['median_context']:,}, weighted {s['weighted']:,}")


def cmd_report(d):
    recs = []
    for p in sorted(glob.glob(os.path.join(d, "*.json"))):
        try:
            recs.append(json.load(open(p)))
        except (OSError, ValueError):
            print(f"skipped unreadable {p}", file=sys.stderr)
    if not recs:
        sys.exit(f"no records in {d}")
    prs = sum(len(r.get("prs") or []) for r in recs)
    w = sum(r.get("weighted", 0) for r in recs)
    print(f"{len(recs)} sessions, {prs} merged PRs, weighted tokens {w:,}"
          + (f", {round(w / prs):,} per merged PR" if prs else ""))
    print(f"median context per session (median of medians): {round(statistics.median(r.get('median_context', 0) for r in recs)):,}")
    print(f"first-call context (fixed load per session, median): {round(statistics.median(r.get('first_context', 0) for r in recs)):,}")
    for r in sorted(recs, key=lambda r: -r.get("weighted", 0))[:10]:
        print(f"  {r.get('weighted', 0):>12,}  {r.get('calls', 0):5} calls  PRs {','.join(map(str, r.get('prs') or [])) or '-':10}  {r.get('task') or r.get('branch', '')}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--last", type=int, default=10)
    ap.add_argument("--record", action="store_true")
    ap.add_argument("--pr", type=int, action="append", default=[], help="with --record: a PR this session merged (repeat)")
    ap.add_argument("--task", default="", help="with --record: a few words naming the task")
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--report", nargs="?", const=DEFAULT_OUT, metavar="DIR")
    a = ap.parse_args()
    if a.report:
        cmd_report(a.report)
    elif a.record:
        cmd_record(a.pr, a.task, a.out)
    else:
        cmd_last(a.last)


if __name__ == "__main__":
    main()
