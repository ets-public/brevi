#!/usr/bin/env python3
"""Register tools/agent_hooks/context_ceiling.py as a PreToolUse hook in ~/.claude/settings.json.

Cloud thread sessions start in the folder above the checkout, so the repo's own .claude/settings.json
would never load there; the user-level file does. The cloud environment's setup script runs this at
container start. Idempotent: an existing entry for this hook is replaced, other settings are kept.

usage: install.py [--settings PATH]
"""
import argparse, json, os, sys

HOOK = os.path.join(os.path.dirname(os.path.abspath(__file__)), "context_ceiling.py")
MARK = "agent_hooks/context_ceiling.py"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--settings", default=os.path.expanduser("~/.claude/settings.json"))
    a = ap.parse_args()
    try:
        cfg = json.load(open(a.settings))
    except FileNotFoundError:
        cfg = {}
    pre = cfg.setdefault("hooks", {}).setdefault("PreToolUse", [])
    pre[:] = [g for g in pre if not any(MARK in str(h.get("command", "")) for h in g.get("hooks", []))]
    pre.append({"hooks": [{"type": "command", "command": f"python3 '{HOOK}'"}]})
    os.makedirs(os.path.dirname(a.settings), exist_ok=True)
    tmp = a.settings + ".tmp"
    with open(tmp, "w") as f:
        json.dump(cfg, f, indent=2)
    os.replace(tmp, a.settings)
    print(f"{a.settings}: PreToolUse -> {HOOK}")


if __name__ == "__main__":
    sys.exit(main())
