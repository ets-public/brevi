#!/usr/bin/env python3
"""PreToolUse hook: a worker session past its context ceiling hands off and ends instead of growing.

Every call re-sends the whole conversation, so a long-lived session pays for its whole history on every
call. Past ADVISORY tokens the session is told once to finish at its next commit; past CEILING only the
handoff steps run: git and gh, tools/session_tokens.py, writes under the handoff and memory folders, and
MCP tools (posting the handoff, resolving the thread). Everything else is refused with the steps.

Judged: only sessions positively known to be workers, a sub-agent (the hook input has agent_id) or a
project thread session (CLAUDE_CODE_CHILD_SESSION=1, or BREVI_CONTEXT_CEILING=1 to opt in). Anything
else, the coordinator and a person's own session included, is never judged.
It fails open when it cannot read the transcript (a new session's first call has none yet): a cost
guard must never stop work through its own error.

The default thresholds sit on top of the ~95k tokens a cloud thread session carries before its first
step (system prompt, tool list, memory): about 150k and 200k of working context.
Settings: BREVI_CONTEXT_ADVISORY (default 250000), BREVI_CONTEXT_CEILING (default 300000).
Install: tools/agent_hooks/install.py. Tests: python3 tests/agent_hooks_test.py.
"""
import json, os, shlex, sys, tempfile
from pathlib import Path

ADVISORY = int(os.environ.get("BREVI_CONTEXT_ADVISORY", "250000"))
CEILING = int(os.environ.get("BREVI_CONTEXT_CEILING", "300000"))
TAIL_BYTES = 4 << 20
HANDOFF_DIRS = ("/mnt/project-files/handoffs/", "/tmp/claude/memory/")
ALWAYS = {"ToolSearch", "TodoWrite", "TaskUpdate", "TaskList", "TaskGet"}
HANDOFF_STEPS = (
    "Hand off now: (1) commit and push your branch; (2) write a handoff of under 30 lines to "
    "/mnt/project-files/handoffs/<topic>.md (state, next steps, links, head commit); "
    "(3) run `python3 tools/session_tokens.py --record --pr N --task \"...\"`; "
    "(4) reply in the thread with the handoff link and resolve it. The coordinator starts a fresh thread "
    "from the handoff, which costs far less than carrying this context on every call."
)
SEPARATORS = {";", "&&", "||", "\n"}


def is_worker(payload):
    return bool(payload.get("agent_id")) or os.environ.get("CLAUDE_CODE_CHILD_SESSION") == "1" \
        or os.environ.get("BREVI_CONTEXT_CEILING") == "1"


def transcript(payload):
    if payload.get("agent_transcript_path"):
        return Path(payload["agent_transcript_path"])
    session = payload.get("transcript_path")
    if not session:
        return None
    session = Path(session)
    if payload.get("agent_id"):
        return session.parent / session.stem / "subagents" / f"agent-{payload['agent_id']}.jsonl"
    return session


def current_context(path):
    """Prompt size of the session's latest call, read from the end of its transcript."""
    with open(path, "rb") as f:
        f.seek(0, os.SEEK_END)
        f.seek(max(0, f.tell() - TAIL_BYTES))
        lines = f.read().decode("utf-8", errors="ignore").splitlines()
    for line in reversed(lines):
        try:
            rec = json.loads(line)
        except ValueError:
            continue
        msg = rec.get("message")
        u = msg.get("usage") if isinstance(msg, dict) else None
        if rec.get("type") == "assistant" and isinstance(u, dict):
            return sum(int(u.get(k) or 0) for k in ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"))
    return None


def handoff_command(command):
    """True when every statement starts with git, gh or the token recorder, with no redirection,
    pipe or substitution (so nothing else can be run through it). A cost guard, not a sandbox."""
    if any(t in command for t in ("$(", "`", ">", "<", "|&")):
        return False
    lex = shlex.shlex(command.replace("\n", " ; "), posix=True, punctuation_chars=";&|")
    lex.whitespace_split = True
    try:
        words = list(lex)
    except ValueError:
        return False
    i, statements = 0, 0
    while i < len(words):
        w = words[i]
        if w in SEPARATORS:
            i += 1
            continue
        if set(w) <= set(";&|"):
            return False
        if w in ("git", "gh"):
            pass
        elif w in ("python3", "python") and i + 1 < len(words) and words[i + 1].endswith("tools/session_tokens.py"):
            pass
        else:
            return False
        statements += 1
        while i < len(words) and words[i] not in SEPARATORS:
            if set(words[i]) <= set(";&|"):
                return False
            i += 1
    return statements > 0


def allowed_past_ceiling(tool, tin):
    if tool.startswith("mcp__") or tool in ALWAYS:
        return True
    if tool == "Bash":
        return handoff_command(str(tin.get("command", "")))
    if tool in ("Write", "Edit", "MultiEdit"):
        p = os.path.normpath(str(tin.get("file_path", "")))
        return any(p.startswith(d) or p + "/" == d for d in HANDOFF_DIRS)
    return False


def say_once(key):
    marker = Path(tempfile.gettempdir()) / f"brevi-context-advisory-{key}"
    try:
        marker.touch(exist_ok=False)
        return True
    except OSError:
        return False


def decide(payload):
    if not is_worker(payload):
        return None
    path = transcript(payload)
    if path is None or not path.is_file():
        return None
    ctx = current_context(path)
    if ctx is None:
        return None
    if ctx >= CEILING:
        if allowed_past_ceiling(str(payload.get("tool_name", "")), payload.get("tool_input") or {}):
            return None
        return {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny",
                "permissionDecisionReason": f"Context is {ctx:,} tokens, past the {CEILING:,} ceiling. "
                                            f"Only the handoff steps run now. {HANDOFF_STEPS}"}}
    if ctx >= ADVISORY and say_once(payload.get("agent_id") or payload.get("session_id") or path.stem):
        return {"hookSpecificOutput": {"hookEventName": "PreToolUse",
                "additionalContext": f"Context is {ctx:,} tokens. Finish at your next commit, then hand off. "
                                     f"{HANDOFF_STEPS} At {CEILING:,} tokens only the handoff steps will run."}}
    return None


def main():
    try:
        payload = json.load(sys.stdin)
        out = decide(payload) if isinstance(payload, dict) else None
    except Exception as e:  # fail open
        print(f"context_ceiling: {e}", file=sys.stderr)
        return 0
    if out:
        print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
